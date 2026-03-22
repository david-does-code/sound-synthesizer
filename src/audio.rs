use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// The available waveform types.
///
/// Each waveform has a different set of harmonics, which is what gives it
/// a distinct timbre (tonal character):
///
/// - **Sine**: No harmonics at all — just the fundamental frequency. Sounds
///   pure and empty, like a tuning fork.
///
/// - **Square**: Contains only odd harmonics (3rd, 5th, 7th...) at amplitudes
///   of 1/n. The missing even harmonics give it a hollow, woody quality.
///   This is the classic chiptune/NES sound.
///
/// - **Sawtooth**: Contains ALL harmonics (2nd, 3rd, 4th...) at 1/n amplitude.
///   The richest waveform — bright and buzzy. Most analog synths use this
///   as a starting point for subtractive synthesis.
///
/// - **Triangle**: Like square, only odd harmonics, but they fall off as 1/n²
///   (much faster). Sounds softer and warmer — partway between sine and square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Waveform {
    Sine = 0,
    Square = 1,
    Sawtooth = 2,
    Triangle = 3,
}

impl Waveform {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Waveform::Sine,
            1 => Waveform::Square,
            2 => Waveform::Sawtooth,
            3 => Waveform::Triangle,
            _ => Waveform::Sine,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Square => "Square",
            Waveform::Sawtooth => "Sawtooth",
            Waveform::Triangle => "Triangle",
        }
    }

    /// Generate a sample for this waveform at the given phase (0.0 to 1.0).
    ///
    /// Phase represents position within one cycle of the wave:
    ///   0.0 = start of cycle
    ///   0.5 = halfway through
    ///   1.0 = end (wraps back to 0.0)
    pub fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => {
                // sin(2π × phase) — a smooth curve
                (phase * TAU).sin()
            }
            Waveform::Square => {
                // +1 for the first half of the cycle, -1 for the second.
                // This abrupt transition is what creates all those odd harmonics.
                if phase < 0.5 { 1.0 } else { -1.0 }
            }
            Waveform::Sawtooth => {
                // Ramps linearly from -1 to +1 over one cycle, then snaps back.
                // The sharp discontinuity at the reset creates rich harmonics.
                2.0 * phase - 1.0
            }
            Waveform::Triangle => {
                // Ramps up for half the cycle, then ramps down.
                // The smooth-ish shape means harmonics fall off quickly (1/n²).
                if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                }
            }
        }
    }
}

/// Converts a MIDI note number to a frequency in Hz.
///
/// MIDI note 69 = A4 = 440 Hz. Each semitone is one MIDI note.
/// The formula: freq = 440 × 2^((note - 69) / 12)
///
/// This is the equal temperament tuning system, where every semitone
/// has the same frequency ratio. It's a compromise — pure intervals
/// like perfect fifths are slightly "off" compared to just intonation,
/// but it lets you play in any key without retuning.
pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert a MIDI note number to a human-readable name like "C4" or "F#5".
///
/// The 12 notes in Western music repeat every octave. MIDI note 0 is C-1,
/// MIDI 60 is C4 (middle C), MIDI 69 is A4 (440 Hz).
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub fn midi_to_name(note: u8) -> String {
    let name = NOTE_NAMES[(note % 12) as usize];
    let octave = (note as i16 / 12) - 1; // MIDI 0 = C-1
    format!("{name}{octave}")
}

/// A real-time audio engine that plays a single note at a time.
///
/// Uses atomics to communicate from the main thread to the audio callback
/// thread — lock-free and safe for real-time audio where blocking (mutexes)
/// can cause glitches.
pub struct AudioEngine {
    _stream: cpal::Stream,
    /// Current frequency to play. 0 = silence.
    /// We use AtomicU32 with f32 bit patterns because there's no AtomicF32.
    frequency: Arc<AtomicU32>,
    /// Current waveform type, as a u8 matching the Waveform repr.
    waveform: Arc<AtomicU8>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No audio output device found");
        let config = device
            .default_output_config()
            .expect("No default output config");

        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let frequency = Arc::new(AtomicU32::new(0u32)); // 0 = silence
        let freq_clone = frequency.clone();

        let waveform = Arc::new(AtomicU8::new(Waveform::Sine as u8));
        let wave_clone = waveform.clone();

        let mut phase: f32 = 0.0;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let freq_bits = freq_clone.load(Ordering::Relaxed);
                    let freq = f32::from_bits(freq_bits);
                    let wave = Waveform::from_u8(wave_clone.load(Ordering::Relaxed));

                    for frame in data.chunks_mut(channels) {
                        if freq <= 0.0 {
                            phase = 0.0;
                            for sample in frame.iter_mut() {
                                *sample = 0.0;
                            }
                            continue;
                        }

                        let value = wave.sample(phase) * 0.4;

                        for sample in frame.iter_mut() {
                            *sample = value;
                        }

                        phase += freq / sample_rate;
                        if phase >= 1.0 {
                            phase -= 1.0;
                        }
                    }
                },
                |err| eprintln!("Audio error: {err}"),
                None,
            )
            .expect("Failed to build audio stream");

        stream.play().expect("Failed to start audio stream");

        AudioEngine {
            _stream: stream,
            frequency,
            waveform,
        }
    }

    /// Start playing a note (by MIDI number). Pass 0 to stop.
    pub fn play_note(&self, midi_note: u8) {
        let freq = if midi_note == 0 {
            0.0
        } else {
            midi_to_freq(midi_note)
        };
        self.frequency
            .store(freq.to_bits(), Ordering::Relaxed);
    }

    /// Stop playing (silence).
    pub fn stop(&self) {
        self.frequency.store(0u32, Ordering::Relaxed);
    }

    /// Switch to a different waveform.
    pub fn set_waveform(&self, waveform: Waveform) {
        self.waveform
            .store(waveform as u8, Ordering::Relaxed);
    }

    /// Get the current waveform.
    pub fn waveform(&self) -> Waveform {
        Waveform::from_u8(self.waveform.load(Ordering::Relaxed))
    }
}
