use crate::envelope::{AdsrParams, Envelope};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
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
    pub fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => (phase * TAU).sin(),
            Waveform::Square => {
                if phase < 0.5 { 1.0 } else { -1.0 }
            }
            Waveform::Sawtooth => 2.0 * phase - 1.0,
            Waveform::Triangle => {
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
pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert a MIDI note number to a human-readable name like "C4" or "F#5".
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub fn midi_to_name(note: u8) -> String {
    let name = NOTE_NAMES[(note % 12) as usize];
    let octave = (note as i16 / 12) - 1;
    format!("{name}{octave}")
}

/// Pack four f32 ADSR params into a single u64 for atomic transfer.
/// Each param is quantized to a u16 (0–65535) which gives us more than
/// enough precision for audio parameters.
fn pack_adsr(params: &AdsrParams) -> u64 {
    let a = (params.attack * 10000.0) as u16;  // 0–6.5535s range, 0.1ms precision
    let d = (params.decay * 10000.0) as u16;
    let s = (params.sustain * 65535.0) as u16;  // 0.0–1.0 range, full u16 precision
    let r = (params.release * 10000.0) as u16;
    ((a as u64) << 48) | ((d as u64) << 32) | ((s as u64) << 16) | (r as u64)
}

fn unpack_adsr(packed: u64) -> AdsrParams {
    let a = ((packed >> 48) & 0xFFFF) as u16;
    let d = ((packed >> 32) & 0xFFFF) as u16;
    let s = ((packed >> 16) & 0xFFFF) as u16;
    let r = (packed & 0xFFFF) as u16;
    AdsrParams {
        attack: a as f32 / 10000.0,
        decay: d as f32 / 10000.0,
        sustain: s as f32 / 65535.0,
        release: r as f32 / 10000.0,
    }
}

/// A real-time audio engine that plays a single note at a time with ADSR envelope.
///
/// Communication between the main thread and the audio callback is entirely
/// lock-free using atomics:
/// - `frequency`: AtomicU32 (f32 bit pattern) for the note pitch
/// - `waveform`: AtomicU8 for the waveform type
/// - `gate`: AtomicBool for note on/off (triggers envelope)
/// - `adsr_packed`: AtomicU64 for ADSR parameters
pub struct AudioEngine {
    _stream: cpal::Stream,
    frequency: Arc<AtomicU32>,
    waveform: Arc<AtomicU8>,
    gate: Arc<AtomicBool>,
    adsr_packed: Arc<AtomicU64>,
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

        let frequency = Arc::new(AtomicU32::new(0u32));
        let freq_clone = frequency.clone();

        let waveform = Arc::new(AtomicU8::new(Waveform::Sine as u8));
        let wave_clone = waveform.clone();

        let gate = Arc::new(AtomicBool::new(false));
        let gate_clone = gate.clone();

        let default_params = AdsrParams::default();
        let adsr_packed = Arc::new(AtomicU64::new(pack_adsr(&default_params)));
        let adsr_clone = adsr_packed.clone();

        let mut phase: f32 = 0.0;
        let mut envelope = Envelope::new(sample_rate);
        let mut prev_gate = false;
        let mut prev_adsr_packed = pack_adsr(&default_params);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let freq_bits = freq_clone.load(Ordering::Relaxed);
                    let freq = f32::from_bits(freq_bits);
                    let wave = Waveform::from_u8(wave_clone.load(Ordering::Relaxed));
                    let gate_on = gate_clone.load(Ordering::Relaxed);

                    // Check if ADSR params changed
                    let current_adsr = adsr_clone.load(Ordering::Relaxed);
                    if current_adsr != prev_adsr_packed {
                        envelope.set_params(unpack_adsr(current_adsr));
                        prev_adsr_packed = current_adsr;
                    }

                    // Detect gate transitions
                    if gate_on && !prev_gate {
                        envelope.gate_on();
                    } else if !gate_on && prev_gate {
                        envelope.gate_off();
                    }
                    prev_gate = gate_on;

                    for frame in data.chunks_mut(channels) {
                        let env_amp = envelope.next_sample();

                        if env_amp <= 0.0 || freq <= 0.0 {
                            if freq <= 0.0 {
                                phase = 0.0;
                            }
                            for sample in frame.iter_mut() {
                                *sample = 0.0;
                            }
                            continue;
                        }

                        let value = wave.sample(phase) * env_amp * 0.4;

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
            gate,
            adsr_packed,
        }
    }

    /// Start playing a note — sets frequency and opens the gate (triggers attack).
    pub fn play_note(&self, midi_note: u8) {
        let freq = midi_to_freq(midi_note);
        self.frequency.store(freq.to_bits(), Ordering::Relaxed);
        self.gate.store(true, Ordering::Relaxed);
    }

    /// Release the note — closes the gate (triggers release phase).
    /// The sound will fade out according to the release time, not stop instantly.
    pub fn stop(&self) {
        self.gate.store(false, Ordering::Relaxed);
    }

    pub fn set_waveform(&self, waveform: Waveform) {
        self.waveform.store(waveform as u8, Ordering::Relaxed);
    }

    pub fn waveform(&self) -> Waveform {
        Waveform::from_u8(self.waveform.load(Ordering::Relaxed))
    }

    /// Update ADSR envelope parameters (takes effect on next note).
    pub fn set_adsr(&self, params: AdsrParams) {
        self.adsr_packed.store(pack_adsr(&params), Ordering::Relaxed);
    }

    /// Get current ADSR parameters.
    #[allow(dead_code)]
    pub fn adsr(&self) -> AdsrParams {
        unpack_adsr(self.adsr_packed.load(Ordering::Relaxed))
    }
}
