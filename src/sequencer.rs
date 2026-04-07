//! Step sequencer — plays a [`Pattern`] in real time with sample-accurate timing.
//!
//! Track names map to drum kinds:
//!
//! | Track name(s)         | Drum kind     |
//! |-----------------------|---------------|
//! | `kick`, `bd`          | [`Drum::Kick`] |
//! | `snare`, `sd`         | [`Drum::Snare`] |
//! | `hihat`, `hh`, `hat`  | [`Drum::HiHat`] |
//!
//! Unrecognized track names are ignored for now (melodic tracks come in slice 4).
//!
//! ## Timing
//!
//! Naive `thread::sleep` per step suffers from audio buffer jitter — each
//! "trigger now" flag waits for the next callback boundary, introducing
//! several milliseconds of random offset per hit.
//!
//! Instead, we use **sample-accurate scheduling**: the sequencer thread
//! computes the absolute audio sample number for each hit and writes it to
//! [`DrumHandle`]. The audio callback fires the drum on the exact sample,
//! independent of when the sequencer thread happened to wake up.
//!
//! The sequencer thread uses a small lookahead (~50 ms) so that triggers are
//! always scheduled before the audio callback would have processed them.
//! Sleep timing then only affects how often the sequencer thread wakes — never
//! the audible playback.

use crate::audio::{Drum, DrumHandle};
use crate::pattern::Pattern;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct Sequencer {
    pattern: Pattern,
    drums: DrumHandle,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Sequencer {
    pub fn new(pattern: Pattern, drums: DrumHandle) -> Self {
        Sequencer {
            pattern,
            drums,
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Start the clock thread. Returns immediately; audio plays in the background
    /// until [`stop`](Self::stop) is called.
    pub fn start(&mut self) {
        let pattern = self.pattern.clone();
        let drums = self.drums.clone();
        let stop = self.stop.clone();

        let sample_rate = drums.sample_rate() as f64;
        // 16th note duration in seconds and in samples.
        let step_secs = 60.0 / pattern.bpm as f64 / 4.0;
        let samples_per_step = (step_secs * sample_rate).round() as u64;

        // Pre-resolve track name → drum kind so we don't string-match on every tick.
        let resolved: Vec<(Option<Drum>, Vec<bool>)> = pattern
            .tracks
            .iter()
            .map(|t| (resolve_drum(&t.name), t.hits.clone()))
            .collect();
        let total_steps = pattern.steps;

        let handle = thread::spawn(move || {
            // Lookahead: schedule events ~100 ms in the future so they're
            // always queued well before the audio callback would have processed
            // them, even on systems with larger audio buffers.
            let lookahead_samples = (sample_rate / 10.0) as u64;
            let start_sample = drums.current_sample() + lookahead_samples;

            let scheduling_start = Instant::now();
            let mut step: usize = 0;
            let mut tick: u64 = 0;

            while !stop.load(Ordering::Relaxed) {
                // Compute the exact audio sample for this step.
                let target_sample = start_sample + tick * samples_per_step;

                // Schedule every drum hit on this step at that exact sample.
                for (drum_opt, hits) in &resolved {
                    if let Some(drum) = drum_opt {
                        if hits[step] {
                            drums.schedule_at(*drum, target_sample);
                        }
                    }
                }

                // Sleep until just before the next step's scheduling time.
                // The sleep target is wall-clock based, but the actual playback
                // timing is sample-accurate regardless of when we wake up.
                tick += 1;
                let next_wakeup = scheduling_start
                    + Duration::from_secs_f64(step_secs * tick as f64);
                let now = Instant::now();
                if next_wakeup > now {
                    thread::sleep(next_wakeup - now);
                }

                step = (step + 1) % total_steps;
            }
        });
        self.handle = Some(handle);
    }

    /// Signal the clock thread to stop and wait for it to exit.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Sequencer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Map a track name to a drum kind. Case-insensitive. Returns `None` for
/// unknown names (e.g. future melodic tracks).
fn resolve_drum(name: &str) -> Option<Drum> {
    match name.to_ascii_lowercase().as_str() {
        "kick" | "bd" | "bassdrum" => Some(Drum::Kick),
        "snare" | "sd" => Some(Drum::Snare),
        "hihat" | "hh" | "hat" | "closedhat" => Some(Drum::HiHat),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_drum_names() {
        assert!(matches!(resolve_drum("kick"), Some(Drum::Kick)));
        assert!(matches!(resolve_drum("BD"), Some(Drum::Kick)));
        assert!(matches!(resolve_drum("snare"), Some(Drum::Snare)));
        assert!(matches!(resolve_drum("HiHat"), Some(Drum::HiHat)));
        assert!(matches!(resolve_drum("hat"), Some(Drum::HiHat)));
        assert!(resolve_drum("bass").is_none());
    }
}
