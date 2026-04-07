//! Step sequencer — plays a [`Pattern`] in real time with sample-accurate timing.
//!
//! Drum tracks are routed by name to one of the synthesized drum voices:
//!
//! | Track name(s)         | Drum kind          |
//! |-----------------------|--------------------|
//! | `kick`, `bd`          | [`Drum::Kick`]     |
//! | `snare`, `sd`         | [`Drum::Snare`]    |
//! | `hihat`, `hh`, `hat`  | [`Drum::HiHat`]    |
//!
//! Note tracks are assigned a pitched voice each, in the order they appear in
//! the pattern. With 8 pitched voices total, up to 8 simultaneous note tracks
//! are supported. Extra note tracks are silently dropped (TODO: warn).
//!
//! ## Timing
//!
//! Naive `thread::sleep` per step suffers from audio buffer jitter — a "trigger
//! now" flag waits for the next callback boundary, introducing several
//! milliseconds of random offset per hit.
//!
//! Instead, the sequencer thread uses **sample-accurate scheduling**: it
//! pre-computes the absolute audio sample for each step and writes it to the
//! [`EngineHandle`]. The audio callback fires the drum or voice on the exact
//! sample, independent of when the sequencer thread woke up.
//!
//! A ~100 ms lookahead ensures events are queued well before the audio callback
//! would have processed them. Sleep timing only affects when scheduling happens,
//! never when playback happens.

use crate::audio::{Drum, EngineHandle, MAX_VOICES};
use crate::pattern::{Cell, ChordCell, Pattern, TrackKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct Sequencer {
    pattern: Pattern,
    engine: EngineHandle,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

/// A track resolved into something the sequencer thread can dispatch directly,
/// without re-walking the original [`Pattern`] structure on every tick.
enum ResolvedTrack {
    Drum {
        drum: Drum,
        hits: Vec<bool>,
    },
    Notes {
        voice_idx: usize,
        cells: Vec<Cell>,
    },
    /// A chord track owns N consecutive voices starting at `voice_base`.
    /// `slots` is N — the number of simultaneous notes the track can play
    /// (the largest chord encountered).
    Chord {
        voice_base: usize,
        slots: usize,
        cells: Vec<ChordCell>,
    },
}

impl Sequencer {
    pub fn new(pattern: Pattern, engine: EngineHandle) -> Self {
        Sequencer {
            pattern,
            engine,
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Start the clock thread. Returns immediately; audio plays in the background
    /// until [`stop`](Self::stop) is called.
    pub fn start(&mut self) {
        let pattern = self.pattern.clone();
        let engine = self.engine.clone();
        let stop = self.stop.clone();

        let sample_rate = engine.sample_rate() as f64;
        let step_secs = 60.0 / pattern.bpm as f64 / 4.0;
        let samples_per_step = (step_secs * sample_rate).round() as u64;

        // Resolve every track once, up-front. Note tracks consume voice indices
        // 0..n in the order they appear; chord tracks consume a contiguous block
        // of voices equal to their max chord size. Tracks that don't fit in
        // MAX_VOICES are dropped with a warning.
        let mut resolved: Vec<ResolvedTrack> = Vec::with_capacity(pattern.tracks.len());
        let mut next_voice: usize = 0;
        for track in &pattern.tracks {
            match &track.kind {
                TrackKind::Drum(hits) => {
                    if let Some(drum) = resolve_drum(&track.name) {
                        resolved.push(ResolvedTrack::Drum {
                            drum,
                            hits: hits.clone(),
                        });
                    }
                }
                TrackKind::Notes(cells) => {
                    if next_voice >= MAX_VOICES {
                        eprintln!(
                            "warning: note track {:?} dropped — only {} pitched voices available",
                            track.name, MAX_VOICES
                        );
                        continue;
                    }
                    if let Some(wave) = track.wave {
                        engine.set_voice_waveform(next_voice, wave);
                    }
                    resolved.push(ResolvedTrack::Notes {
                        voice_idx: next_voice,
                        cells: cells.clone(),
                    });
                    next_voice += 1;
                }
                TrackKind::Chord(cells) => {
                    let max_chord = cells
                        .iter()
                        .filter_map(|c| match c {
                            ChordCell::Chord(notes) => Some(notes.len()),
                            _ => None,
                        })
                        .max()
                        .unwrap_or(0);
                    if max_chord == 0 {
                        // Track has no actual chords — skip it.
                        continue;
                    }
                    if next_voice + max_chord > MAX_VOICES {
                        eprintln!(
                            "warning: chord track {:?} dropped — needs {} voices but only {} remain",
                            track.name,
                            max_chord,
                            MAX_VOICES - next_voice
                        );
                        continue;
                    }
                    let voice_base = next_voice;
                    if let Some(wave) = track.wave {
                        for v in voice_base..(voice_base + max_chord) {
                            engine.set_voice_waveform(v, wave);
                        }
                    }
                    resolved.push(ResolvedTrack::Chord {
                        voice_base,
                        slots: max_chord,
                        cells: cells.clone(),
                    });
                    next_voice += max_chord;
                }
            }
        }

        let total_steps = pattern.steps;

        let handle = thread::spawn(move || {
            // Lookahead: schedule events ~100 ms in the future so they're
            // always queued well before the audio callback would have processed
            // them, even on systems with larger audio buffers.
            let lookahead_samples = (sample_rate / 10.0) as u64;
            let start_sample = engine.current_sample() + lookahead_samples;

            let scheduling_start = Instant::now();
            let mut step: usize = 0;
            let mut tick: u64 = 0;

            while !stop.load(Ordering::Relaxed) {
                let target_sample = start_sample + tick * samples_per_step;

                // Dispatch every track's event for this step.
                for track in &resolved {
                    match track {
                        ResolvedTrack::Drum { drum, hits } => {
                            if hits[step] {
                                engine.schedule_at(*drum, target_sample);
                            }
                        }
                        ResolvedTrack::Notes { voice_idx, cells } => match cells[step] {
                            Cell::Note(midi) => {
                                engine.schedule_note_on(*voice_idx, target_sample, midi);
                            }
                            Cell::Rest => {
                                engine.schedule_note_off(*voice_idx, target_sample);
                            }
                            Cell::Sustain => {
                                // Nothing to do — voice keeps playing.
                            }
                        },
                        ResolvedTrack::Chord { voice_base, slots, cells } => match &cells[step] {
                            ChordCell::Chord(notes) => {
                                // Trigger one note per voice slot. Any extra
                                // slots beyond this chord's note count get
                                // released so previous notes don't linger.
                                for s in 0..*slots {
                                    let voice = voice_base + s;
                                    if let Some(midi) = notes.get(s) {
                                        engine.schedule_note_on(voice, target_sample, *midi);
                                    } else {
                                        engine.schedule_note_off(voice, target_sample);
                                    }
                                }
                            }
                            ChordCell::Rest => {
                                for s in 0..*slots {
                                    engine.schedule_note_off(voice_base + s, target_sample);
                                }
                            }
                            ChordCell::Sustain => {
                                // Voices keep playing.
                            }
                        },
                    }
                }

                // Sleep until ~when the next step needs scheduling. Wakeup time
                // is wall-clock based; playback time is sample-accurate.
                tick += 1;
                let next_wakeup = scheduling_start
                    + Duration::from_secs_f64(step_secs * tick as f64);
                let now = Instant::now();
                if next_wakeup > now {
                    thread::sleep(next_wakeup - now);
                }

                step = (step + 1) % total_steps;
            }

            // On stop, release any voices we own so they fade out cleanly.
            let release_at = engine.current_sample() + 1;
            for track in &resolved {
                match track {
                    ResolvedTrack::Notes { voice_idx, .. } => {
                        engine.schedule_note_off(*voice_idx, release_at);
                    }
                    ResolvedTrack::Chord { voice_base, slots, .. } => {
                        for s in 0..*slots {
                            engine.schedule_note_off(voice_base + s, release_at);
                        }
                    }
                    ResolvedTrack::Drum { .. } => {}
                }
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
/// unknown names.
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
