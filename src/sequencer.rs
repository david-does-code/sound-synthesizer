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
//! Note tracks consume one voice each from the 8-voice pitched pool. Chord
//! tracks consume a contiguous block of N voices, where N is the largest
//! chord size in that track. **Voice assignments are global across all
//! sections in a song**: the first appearance of a track name (in song order)
//! gets a voice slot, and every later section that uses the same track name
//! reuses the same slot. This means a `bass` line that appears in both verse
//! and chorus shares one voice — its envelope state can carry over cleanly.
//!
//! ## Song playback
//!
//! The sequencer walks the [`Pattern::song`] chain, playing each section the
//! requested number of times in order, then loops the whole song forever.
//! When transitioning from one section to a different one, voices that were
//! used by the previous section but not the new one are released, so notes
//! don't drone forever after their track disappears.
//!
//! ## Timing
//!
//! See the slice 2 commit for the lock-free, sample-accurate scheduling
//! design. The same scheme is used here — the only thing that changed is
//! that `tick` now counts steps across the entire song, not just one bar.

use crate::audio::{Drum, EngineHandle, MAX_VOICES};
use crate::pattern::{Cell, ChordCell, Pattern, TrackKind};
use std::collections::HashMap;
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

/// One pitched / chord track resolved into something the sequencer thread can
/// dispatch directly, without re-walking the pattern.
enum ResolvedTrack {
    Drum {
        drum: Drum,
        hits: Vec<bool>,
    },
    Notes {
        voice_idx: usize,
        cells: Vec<Cell>,
    },
    /// Chord tracks own `slots` consecutive voices starting at `voice_base`.
    Chord {
        voice_base: usize,
        slots: usize,
        cells: Vec<ChordCell>,
    },
}

/// One section resolved against the global voice assignment.
struct ResolvedSection {
    name: String,
    steps: usize,
    tracks: Vec<ResolvedTrack>,
    /// Distinct pitched-voice indices used by this section's note/chord tracks.
    /// Used at section transitions to release voices that drop out.
    voice_set: Vec<usize>,
}

/// A voice allocation for one melodic/chord track name.
#[derive(Clone, Copy)]
struct VoiceAlloc {
    base: usize,
    slots: usize,
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

        // Pre-resolve voice allocations and per-section dispatch tables.
        let resolved_sections = pre_resolve(&pattern, &engine);
        let song = pattern.song.clone();

        let handle = thread::spawn(move || {
            // Lookahead: schedule events ~100 ms in the future so they're
            // always queued well before the audio callback would have processed
            // them, even on systems with larger audio buffers.
            let lookahead_samples = (sample_rate / 10.0) as u64;
            let start_sample = engine.current_sample() + lookahead_samples;

            let scheduling_start = Instant::now();
            let mut tick: u64 = 0;
            let mut prev_voice_set: Vec<usize> = Vec::new();

            'outer: loop {
                for entry in &song {
                    let Some(section) =
                        resolved_sections.iter().find(|s| s.name == entry.section)
                    else {
                        // Should never happen — parser already validated.
                        continue;
                    };

                    for _ in 0..entry.repeat {
                        if stop.load(Ordering::Relaxed) {
                            break 'outer;
                        }

                        // Section transition: release any voices used by the
                        // previous section that this one doesn't touch, so a
                        // sustained bass note from the verse doesn't drone
                        // through a kick-only outro.
                        let transition_sample = start_sample + tick * samples_per_step;
                        for v in &prev_voice_set {
                            if !section.voice_set.contains(v) {
                                engine.schedule_note_off(*v, transition_sample);
                            }
                        }

                        for step in 0..section.steps {
                            if stop.load(Ordering::Relaxed) {
                                break 'outer;
                            }

                            let target_sample = start_sample + tick * samples_per_step;

                            for track in &section.tracks {
                                dispatch_step(&engine, track, step, target_sample);
                            }

                            tick += 1;
                            let next_wakeup = scheduling_start
                                + Duration::from_secs_f64(step_secs * tick as f64);
                            let now = Instant::now();
                            if next_wakeup > now {
                                thread::sleep(next_wakeup - now);
                            }
                        }

                        prev_voice_set = section.voice_set.clone();
                    }
                }
            }

            // On stop, release every pitched voice we touched so the song
            // fades out cleanly instead of cutting off mid-note.
            let release_at = engine.current_sample() + 1;
            for v in &prev_voice_set {
                engine.schedule_note_off(*v, release_at);
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

/// Walk every section in declaration order and allocate pitched voices for
/// each unique track name. Drum tracks need no voices. Note tracks claim one
/// voice each. Chord tracks claim N consecutive voices (N = the largest chord
/// in that track across all sections).
///
/// Per-track waveforms are written into the engine here, once at startup.
fn pre_resolve(pattern: &Pattern, engine: &EngineHandle) -> Vec<ResolvedSection> {
    // First pass: build the global allocation table.
    let mut alloc: HashMap<String, VoiceAlloc> = HashMap::new();
    let mut next_voice: usize = 0;

    for section in &pattern.sections {
        for track in &section.tracks {
            match &track.kind {
                TrackKind::Drum(_) => {} // drums don't need pitched voices
                TrackKind::Notes(_) => {
                    if alloc.contains_key(&track.name) {
                        continue;
                    }
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
                    alloc.insert(track.name.clone(), VoiceAlloc { base: next_voice, slots: 1 });
                    next_voice += 1;
                }
                TrackKind::Chord(cells) => {
                    let chord_size = max_chord_size(cells);
                    if chord_size == 0 {
                        continue;
                    }

                    if let Some(existing) = alloc.get(&track.name) {
                        // Track appeared in an earlier section. If a later
                        // section needs more slots than we allocated, we can't
                        // safely expand without shifting everything else;
                        // warn and the extra notes get dropped at dispatch.
                        if existing.slots < chord_size {
                            eprintln!(
                                "warning: chord track {:?} has chords of {} notes in a later section but only {} voices reserved — extra notes will be dropped",
                                track.name, chord_size, existing.slots
                            );
                        }
                        continue;
                    }

                    if next_voice + chord_size > MAX_VOICES {
                        eprintln!(
                            "warning: chord track {:?} dropped — needs {} voices but only {} remain",
                            track.name,
                            chord_size,
                            MAX_VOICES - next_voice
                        );
                        continue;
                    }

                    if let Some(wave) = track.wave {
                        for v in next_voice..(next_voice + chord_size) {
                            engine.set_voice_waveform(v, wave);
                        }
                    }
                    alloc.insert(
                        track.name.clone(),
                        VoiceAlloc { base: next_voice, slots: chord_size },
                    );
                    next_voice += chord_size;
                }
            }
        }
    }

    // Second pass: build per-section dispatch tables that reference the
    // global voice allocations.
    let mut resolved = Vec::with_capacity(pattern.sections.len());
    for section in &pattern.sections {
        let mut tracks = Vec::with_capacity(section.tracks.len());
        let mut voice_set: Vec<usize> = Vec::new();
        for track in &section.tracks {
            match &track.kind {
                TrackKind::Drum(hits) => {
                    if let Some(drum) = resolve_drum(&track.name) {
                        tracks.push(ResolvedTrack::Drum {
                            drum,
                            hits: hits.clone(),
                        });
                    }
                }
                TrackKind::Notes(cells) => {
                    if let Some(va) = alloc.get(&track.name) {
                        tracks.push(ResolvedTrack::Notes {
                            voice_idx: va.base,
                            cells: cells.clone(),
                        });
                        push_unique(&mut voice_set, va.base);
                    }
                }
                TrackKind::Chord(cells) => {
                    if let Some(va) = alloc.get(&track.name) {
                        tracks.push(ResolvedTrack::Chord {
                            voice_base: va.base,
                            slots: va.slots,
                            cells: cells.clone(),
                        });
                        for s in 0..va.slots {
                            push_unique(&mut voice_set, va.base + s);
                        }
                    }
                }
            }
        }
        resolved.push(ResolvedSection {
            name: section.name.clone(),
            steps: section.steps,
            tracks,
            voice_set,
        });
    }
    resolved
}

fn max_chord_size(cells: &[ChordCell]) -> usize {
    cells
        .iter()
        .filter_map(|c| match c {
            ChordCell::Chord(notes) => Some(notes.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

fn push_unique(v: &mut Vec<usize>, x: usize) {
    if !v.contains(&x) {
        v.push(x);
    }
}

fn dispatch_step(
    engine: &EngineHandle,
    track: &ResolvedTrack,
    step: usize,
    target_sample: u64,
) {
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
            Cell::Sustain => {}
        },
        ResolvedTrack::Chord { voice_base, slots, cells } => match &cells[step] {
            ChordCell::Chord(notes) => {
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
            ChordCell::Sustain => {}
        },
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
