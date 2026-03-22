use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

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

/// A real-time audio engine that plays a single note at a time.
///
/// Uses an atomic to communicate the desired frequency from the main
/// thread to the audio callback thread — lock-free and safe for
/// real-time audio where blocking (mutexes) can cause glitches.
pub struct AudioEngine {
    _stream: cpal::Stream,
    /// Current frequency to play. 0 = silence.
    /// We use AtomicU32 with f32 bit patterns because there's no AtomicF32.
    frequency: Arc<AtomicU32>,
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

        let mut phase: f32 = 0.0;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let freq_bits = freq_clone.load(Ordering::Relaxed);
                    let freq = f32::from_bits(freq_bits);

                    for frame in data.chunks_mut(channels) {
                        if freq <= 0.0 {
                            // Silence — also reset phase so next note starts clean
                            phase = 0.0;
                            for sample in frame.iter_mut() {
                                *sample = 0.0;
                            }
                            continue;
                        }

                        let value = (phase * TAU).sin() * 0.4;

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
}
