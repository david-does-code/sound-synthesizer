//! Offline WAV renderer.
//!
//! Plays a [`Pattern`] through one full pass of its `song` chain (no looping)
//! and writes the result to a WAV file. Uses the same synthesis primitives as
//! the live engine ([`Voice`], [`DrumVoice`]) so what you hear from `--play`
//! is what you get from `--render`.
//!
//! Single-threaded, so we skip the atomic gymnastics: events apply directly to
//! the voice state at the right sample boundary.

use crate::audio::{Drum, DrumVoice, Voice, Waveform, MAX_VOICES, NUM_DRUMS};
use crate::envelope::AdsrParams;
use crate::pattern::{Cell, ChordCell, Pattern, Track, TrackKind};
use crate::reverb::Reverb;
use std::collections::HashMap;
use std::path::Path;

/// Sample rate for offline renders. 44.1 kHz is standard CD quality and
/// keeps file sizes modest for short songs.
const SAMPLE_RATE: f32 = 44_100.0;

/// Extra audio appended after the song ends so release envelopes can finish
/// without an abrupt cutoff.
const TAIL_SECONDS: f32 = 2.0;

/// One pitched / chord / drum track resolved for a single section.
enum ResolvedTrack {
    Drum {
        drum: Drum,
        hits: Vec<f32>,
    },
    Notes {
        voice_idx: usize,
        cells: Vec<Cell>,
        gate_samples: u64,
    },
    Chord {
        voice_base: usize,
        slots: usize,
        cells: Vec<ChordCell>,
        gate_samples: u64,
    },
}

struct ResolvedSection {
    name: String,
    steps: usize,
    samples_per_step: u64,
    swing: f32,
    tracks: Vec<ResolvedTrack>,
    /// Pitched voice indices used by this section.
    voice_set: Vec<usize>,
}

#[derive(Clone, Copy)]
struct VoiceAlloc {
    base: usize,
    slots: usize,
}

/// Render a pattern to a 16-bit mono WAV file at 44.1 kHz.
pub fn render_to_wav(
    pattern: &Pattern,
    output: &Path,
) -> Result<RenderStats, Box<dyn std::error::Error>> {
    // ─── Resolve voices and per-voice settings ───────────────────────
    let mut alloc: HashMap<String, VoiceAlloc> = HashMap::new();
    let mut next_voice: usize = 0;

    // Per-voice settings (parallel arrays indexed by voice index).
    let mut voice_wave: [Waveform; MAX_VOICES] = [Waveform::Sine; MAX_VOICES];
    let mut voice_adsr: [AdsrParams; MAX_VOICES] =
        [AdsrParams::default(); MAX_VOICES];
    let mut voice_gain: [f32; MAX_VOICES] = [1.0; MAX_VOICES];
    let mut drum_gain: [f32; NUM_DRUMS] = [1.0; NUM_DRUMS];

    // Pass 1: walk every section in declaration order, allocate pitched
    // voices for unique track names, and capture per-track instrument
    // settings (waveform, ADSR, gain). Same logic as sequencer::pre_resolve
    // but storing into local arrays instead of engine atomics.
    for section in &pattern.sections {
        for track in &section.tracks {
            match &track.kind {
                TrackKind::Drum(_) => {
                    if let (Some(g), Some(d)) = (track.gain, resolve_drum(&track.name)) {
                        drum_gain[d as usize] = g;
                    }
                }
                TrackKind::Notes(_) => {
                    if alloc.contains_key(&track.name) {
                        continue;
                    }
                    if next_voice >= MAX_VOICES {
                        eprintln!(
                            "warning: note track {:?} dropped — out of voices",
                            track.name
                        );
                        continue;
                    }
                    apply_track_settings(
                        track,
                        next_voice..next_voice + 1,
                        &mut voice_wave,
                        &mut voice_adsr,
                        &mut voice_gain,
                    );
                    alloc.insert(
                        track.name.clone(),
                        VoiceAlloc { base: next_voice, slots: 1 },
                    );
                    next_voice += 1;
                }
                TrackKind::Chord(cells) => {
                    let chord_size = max_chord_size(cells);
                    if chord_size == 0 || alloc.contains_key(&track.name) {
                        continue;
                    }
                    if next_voice + chord_size > MAX_VOICES {
                        eprintln!(
                            "warning: chord track {:?} dropped — needs {} voices, {} remain",
                            track.name,
                            chord_size,
                            MAX_VOICES - next_voice
                        );
                        continue;
                    }
                    apply_track_settings(
                        track,
                        next_voice..next_voice + chord_size,
                        &mut voice_wave,
                        &mut voice_adsr,
                        &mut voice_gain,
                    );
                    alloc.insert(
                        track.name.clone(),
                        VoiceAlloc { base: next_voice, slots: chord_size },
                    );
                    next_voice += chord_size;
                }
            }
        }
    }

    // Pass 2: build per-section dispatch tables.
    let mut sections: Vec<ResolvedSection> = Vec::with_capacity(pattern.sections.len());
    for section in &pattern.sections {
        let section_bpm = section.bpm.unwrap_or(pattern.bpm);
        let step_secs = 60.0 / section_bpm as f64 / 4.0;
        let samples_per_step = (step_secs * SAMPLE_RATE as f64).round() as u64;
        let swing = section.swing.unwrap_or(pattern.swing);

        let mut tracks: Vec<ResolvedTrack> = Vec::with_capacity(section.tracks.len());
        let mut voice_set: Vec<usize> = Vec::new();

        for track in &section.tracks {
            let gate_samples = track
                .gate
                .map(|g| (g as f64 * samples_per_step as f64) as u64)
                .unwrap_or(0);
            match &track.kind {
                TrackKind::Drum(hits) => {
                    if let Some(drum) = resolve_drum(&track.name) {
                        tracks.push(ResolvedTrack::Drum { drum, hits: hits.clone() });
                    }
                }
                TrackKind::Notes(cells) => {
                    if let Some(va) = alloc.get(&track.name) {
                        tracks.push(ResolvedTrack::Notes {
                            voice_idx: va.base,
                            cells: cells.clone(),
                            gate_samples,
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
                            gate_samples,
                        });
                        for s in 0..va.slots {
                            push_unique(&mut voice_set, va.base + s);
                        }
                    }
                }
            }
        }

        sections.push(ResolvedSection {
            name: section.name.clone(),
            steps: section.steps,
            samples_per_step,
            swing,
            tracks,
            voice_set,
        });
    }

    // ─── Build voices and drum voices ────────────────────────────────
    let mut voices: Vec<Voice> = (0..MAX_VOICES).map(|_| Voice::new(SAMPLE_RATE)).collect();
    for (i, params) in voice_adsr.iter().enumerate() {
        voices[i].envelope.set_params(*params);
    }
    for (idx, semitones) in collect_clicks(pattern, &alloc) {
        voices[idx].set_click(semitones);
    }
    let mut drums: [DrumVoice; NUM_DRUMS] = [
        DrumVoice::new(Drum::Kick, SAMPLE_RATE, 0x1234_5678),
        DrumVoice::new(Drum::Snare, SAMPLE_RATE, 0x9E37_79B9),
        DrumVoice::new(Drum::HiHat, SAMPLE_RATE, 0xBADC_0FFE),
    ];

    // ─── Walk the song chain and synthesize ──────────────────────────
    // Output buffer. Pre-allocate based on song length estimate.
    let mut samples: Vec<f32> = Vec::new();
    let mut prev_voice_set: Vec<usize> = Vec::new();
    let mut sections_played: u32 = 0;

    for entry in &pattern.song {
        let Some(section) = sections.iter().find(|s| s.name == entry.section) else {
            continue;
        };
        let sps = section.samples_per_step;
        for _ in 0..entry.repeat {
            // Section transition: release voices that drop out.
            for v in &prev_voice_set {
                if !section.voice_set.contains(v) {
                    voices[*v].release();
                }
            }

            for step in 0..section.steps {
                // Swing: nudge odd-numbered steps later by a fraction of step duration.
                let swing_offset = if step % 2 == 1 {
                    (section.swing as f64 * sps as f64) as u64
                } else {
                    0
                };

                // Apply step events first (we synthesize for this step's
                // duration including any swing offset, but the step *event*
                // happens at offset 0 of the step, plus swing).
                // Generate `swing_offset` samples of "before-event" audio,
                // then trigger the step's events, then generate the rest.
                generate_samples(
                    swing_offset,
                    &mut samples,
                    &mut voices,
                    &voice_wave,
                    &voice_gain,
                    &mut drums,
                    &drum_gain,
                );

                for track in &section.tracks {
                    match track {
                        ResolvedTrack::Drum { drum, hits } => {
                            let vel = hits[step];
                            if vel > 0.0 {
                                drums[*drum as usize].trigger_with_velocity(vel);
                            }
                        }
                        _ => dispatch_step(track, step, &mut voices),
                    }
                }

                let remaining = sps.saturating_sub(swing_offset);
                generate_samples(
                    remaining,
                    &mut samples,
                    &mut voices,
                    &voice_wave,
                    &voice_gain,
                    &mut drums,
                    &drum_gain,
                );
            }

            prev_voice_set = section.voice_set.clone();
            sections_played += 1;
        }
    }

    // Release any voices still held at song end, then render the tail so
    // their releases finish naturally.
    for v in &prev_voice_set {
        voices[*v].release();
    }
    let tail_samples = (TAIL_SECONDS * SAMPLE_RATE) as u64;
    generate_samples(
        tail_samples,
        &mut samples,
        &mut voices,
        &voice_wave,
        &voice_gain,
        &mut drums,
        &drum_gain,
    );

    // ─── Master reverb ───────────────────────────────────────────────
    // Single send-style reverb on the full master mix. A second pass over
    // the dry buffer feeds each sample through the reverb in order, so
    // echoes of earlier samples land naturally on later ones (including
    // during the release tail).
    if pattern.reverb > 0.0 {
        let mut reverb = Reverb::new(pattern.reverb);
        for s in samples.iter_mut() {
            *s = reverb.process(*s);
        }
    }

    // ─── Write WAV ───────────────────────────────────────────────────
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output, spec)?;

    // Track peak so we can warn if we clipped.
    let mut peak: f32 = 0.0;
    for s in &samples {
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
    }

    for s in &samples {
        // Clamp to [-1, 1] then convert to i16. The live engine's mix scaling
        // (0.4/√8 for voices, 0.5 for drums) already keeps things well below 1.0
        // for normal patterns, but cumulative ADSR + multiple loud chord stacks
        // can occasionally exceed 1.0.
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(v)?;
    }
    writer.finalize()?;

    Ok(RenderStats {
        sample_count: samples.len(),
        duration_secs: samples.len() as f32 / SAMPLE_RATE,
        peak_amplitude: peak,
        sections_played,
    })
}

/// Generate `n` samples and append them to `out`. Each sample is the mix of
/// all active voices (with per-voice gain and the same √MAX_VOICES scaling
/// as the live engine) plus the drums (also with per-drum gain).
fn generate_samples(
    n: u64,
    out: &mut Vec<f32>,
    voices: &mut [Voice],
    voice_wave: &[Waveform; MAX_VOICES],
    voice_gain: &[f32; MAX_VOICES],
    drums: &mut [DrumVoice; NUM_DRUMS],
    drum_gain: &[f32; NUM_DRUMS],
) {
    let voice_scale = 0.4 / (MAX_VOICES as f32).sqrt();
    let drum_scale = 0.5;

    for _ in 0..n {
        let mut mix = 0.0_f32;
        for (i, v) in voices.iter_mut().enumerate() {
            // Gate auto-release countdown.
            if v.gate_remaining > 0 {
                v.gate_remaining -= 1;
                if v.gate_remaining == 0 {
                    v.envelope.gate_off();
                }
            }
            mix += v.next_sample(voice_wave[i]) * voice_gain[i];
        }
        let mut value = mix * voice_scale;

        let mut drum_mix = 0.0_f32;
        for (i, d) in drums.iter_mut().enumerate() {
            drum_mix += d.next_sample() * drum_gain[i];
        }
        value += drum_mix * drum_scale;

        out.push(value);
    }
}

fn dispatch_step(track: &ResolvedTrack, step: usize, voices: &mut [Voice]) {
    match track {
        ResolvedTrack::Drum { .. } => {
            // Drums handled separately so we can keep the &mut DrumVoice
            // pattern out of this function. See `dispatch_drum_step`.
        }
        ResolvedTrack::Notes { voice_idx, cells, gate_samples } => match cells[step] {
            Cell::Note(midi, vel) => {
                voices[*voice_idx].trigger(midi, vel);
                voices[*voice_idx].gate_remaining = *gate_samples;
            }
            Cell::Rest => {
                voices[*voice_idx].release();
            }
            Cell::Sustain => {}
        },
        ResolvedTrack::Chord { voice_base, slots, cells, gate_samples } => {
            match &cells[step] {
                ChordCell::Chord(notes) => {
                    for s in 0..*slots {
                        let voice = voice_base + s;
                        if let Some((midi, vel)) = notes.get(s) {
                            voices[voice].trigger(*midi, *vel);
                            voices[voice].gate_remaining = *gate_samples;
                        } else {
                            voices[voice].release();
                        }
                    }
                }
                ChordCell::Rest => {
                    for s in 0..*slots {
                        voices[voice_base + s].release();
                    }
                }
                ChordCell::Sustain => {}
            }
        }
    }
}

// Drums need separate dispatch because the &mut on drums must not coexist
// with &mut on voices in `generate_samples`. Called inline in the song loop.
fn apply_track_settings(
    track: &Track,
    range: std::ops::Range<usize>,
    voice_wave: &mut [Waveform; MAX_VOICES],
    voice_adsr: &mut [AdsrParams; MAX_VOICES],
    voice_gain: &mut [f32; MAX_VOICES],
) {
    let def = AdsrParams::default();
    let params = AdsrParams {
        attack: track.attack.unwrap_or(def.attack),
        decay: track.decay.unwrap_or(def.decay),
        sustain: track.sustain.unwrap_or(def.sustain),
        release: track.release.unwrap_or(def.release),
    };
    for v in range {
        if let Some(w) = track.wave {
            voice_wave[v] = w;
        }
        voice_adsr[v] = params;
        if let Some(g) = track.gain {
            voice_gain[v] = g;
        }
    }
}

/// Per-track click amounts collected during pre-resolve, applied to voices
/// before rendering. (`apply_track_settings` only handles the simple parallel
/// arrays — click is applied directly on Voice via `set_click`.)
fn collect_clicks(
    pattern: &Pattern,
    alloc: &HashMap<String, VoiceAlloc>,
) -> Vec<(usize, f32)> {
    let mut out: Vec<(usize, f32)> = Vec::new();
    for section in &pattern.sections {
        for track in &section.tracks {
            let Some(c) = track.click else { continue };
            let Some(va) = alloc.get(&track.name) else { continue };
            for s in 0..va.slots {
                if !out.iter().any(|(v, _)| *v == va.base + s) {
                    out.push((va.base + s, c));
                }
            }
        }
    }
    out
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

fn resolve_drum(name: &str) -> Option<Drum> {
    match name.to_ascii_lowercase().as_str() {
        "kick" | "bd" | "bassdrum" => Some(Drum::Kick),
        "snare" | "sd" => Some(Drum::Snare),
        "hihat" | "hh" | "hat" | "closedhat" => Some(Drum::HiHat),
        _ => None,
    }
}

pub struct RenderStats {
    pub sample_count: usize,
    pub duration_secs: f32,
    pub peak_amplitude: f32,
    pub sections_played: u32,
}
