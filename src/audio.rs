use crate::envelope::{AdsrParams, Envelope};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

/// Maximum number of simultaneous voices (notes).
pub const MAX_VOICES: usize = 8;

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

    pub fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => (phase * TAU).sin(),
            Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Sawtooth => 2.0 * phase - 1.0,
            Waveform::Triangle => {
                if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase }
            }
        }
    }
}

pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub fn midi_to_name(note: u8) -> String {
    let name = NOTE_NAMES[(note % 12) as usize];
    let octave = (note as i16 / 12) - 1;
    format!("{name}{octave}")
}

fn pack_adsr(params: &AdsrParams) -> u64 {
    let a = (params.attack * 10000.0) as u16;
    let d = (params.decay * 10000.0) as u16;
    let s = (params.sustain * 65535.0) as u16;
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

/// Voice commands sent from main thread → audio callback.
const CMD_IDLE: u32 = 0;
const CMD_PLAY: u32 = 1;
const CMD_RELEASE: u32 = 2;

fn pack_cmd(cmd: u32, midi: u8) -> u32 {
    (cmd << 16) | (midi as u32)
}

fn unpack_cmd(packed: u32) -> (u32, u8) {
    (packed >> 16, (packed & 0xFF) as u8)
}

/// State for a single voice inside the audio callback.
struct Voice {
    frequency: f32,
    phase: f32,
    envelope: Envelope,
    active: bool,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        Voice {
            frequency: 0.0,
            phase: 0.0,
            envelope: Envelope::new(sample_rate),
            active: false,
        }
    }

    fn next_sample(&mut self, waveform: Waveform) -> f32 {
        if !self.active {
            return 0.0;
        }

        let env_amp = self.envelope.next_sample();

        if env_amp <= 0.0 {
            self.active = false;
            return 0.0;
        }

        let value = waveform.sample(self.phase) * env_amp;

        self.phase += self.frequency / self.envelope.sample_rate();
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        value
    }
}

/// A polyphonic real-time audio engine.
///
/// Supports up to MAX_VOICES simultaneous notes. The main thread
/// allocates voices and the audio callback generates samples.
///
/// Communication is lock-free:
/// - `voice_commands`: AtomicU32 array — main thread writes play/release commands
/// - `voice_active`: AtomicBool array — callback reports which voices are sounding
///   (the main thread reads this to find free voices)
/// - `waveform`: AtomicU8
/// - `adsr_packed`: AtomicU64
pub struct AudioEngine {
    _stream: cpal::Stream,
    voice_commands: Arc<[AtomicU32; MAX_VOICES]>,
    /// Set to true by the callback when a voice starts playing,
    /// set to false when its envelope finishes. The main thread reads
    /// this to find free voices for allocation.
    voice_active: Arc<[AtomicBool; MAX_VOICES]>,
    waveform: Arc<AtomicU8>,
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

        let voice_commands: Arc<[AtomicU32; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU32::new(CMD_IDLE)),
        );
        let cmds_clone = voice_commands.clone();

        let voice_active: Arc<[AtomicBool; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicBool::new(false)),
        );
        let active_clone = voice_active.clone();

        let waveform = Arc::new(AtomicU8::new(Waveform::Sine as u8));
        let wave_clone = waveform.clone();

        let default_params = AdsrParams::default();
        let adsr_packed = Arc::new(AtomicU64::new(pack_adsr(&default_params)));
        let adsr_clone = adsr_packed.clone();

        let mut voices: Vec<Voice> = (0..MAX_VOICES)
            .map(|_| Voice::new(sample_rate))
            .collect();
        let mut prev_adsr_packed = pack_adsr(&default_params);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let wave = Waveform::from_u8(wave_clone.load(Ordering::Relaxed));

                    // Update ADSR params if changed
                    let current_adsr = adsr_clone.load(Ordering::Relaxed);
                    if current_adsr != prev_adsr_packed {
                        let params = unpack_adsr(current_adsr);
                        for voice in voices.iter_mut() {
                            voice.envelope.set_params(params);
                        }
                        prev_adsr_packed = current_adsr;
                    }

                    // Process commands from the main thread
                    for (i, cmd_slot) in cmds_clone.iter().enumerate() {
                        let packed = cmd_slot.load(Ordering::Relaxed);
                        let (cmd, midi) = unpack_cmd(packed);

                        match cmd {
                            CMD_PLAY => {
                                voices[i].frequency = midi_to_freq(midi);
                                voices[i].phase = 0.0;
                                voices[i].active = true;
                                voices[i].envelope.gate_on();
                                active_clone[i].store(true, Ordering::Relaxed);
                                cmd_slot.store(CMD_IDLE, Ordering::Relaxed);
                            }
                            CMD_RELEASE => {
                                voices[i].envelope.gate_off();
                                cmd_slot.store(CMD_IDLE, Ordering::Relaxed);
                            }
                            _ => {}
                        }
                    }

                    // Generate audio: mix all active voices
                    for frame in data.chunks_mut(channels) {
                        let mut mix = 0.0_f32;

                        for (i, voice) in voices.iter_mut().enumerate() {
                            let was_active = voice.active;
                            mix += voice.next_sample(wave);
                            // If voice just became inactive, update the shared flag
                            if was_active && !voice.active {
                                active_clone[i].store(false, Ordering::Relaxed);
                            }
                        }

                        // Scale to prevent clipping: 0.4 base volume, divided by sqrt(max voices)
                        let value = mix * 0.4 / (MAX_VOICES as f32).sqrt();

                        for sample in frame.iter_mut() {
                            *sample = value;
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
            voice_commands,
            voice_active,
            waveform,
            adsr_packed,
        }
    }

    /// Play a note — finds a free voice and assigns it.
    /// Returns the voice index that was assigned.
    pub fn play_note(&self, midi_note: u8) -> usize {
        let idx = self.find_free_voice();
        self.voice_commands[idx]
            .store(pack_cmd(CMD_PLAY, midi_note), Ordering::Relaxed);
        idx
    }

    /// Release a specific voice by index (triggers ADSR release phase).
    pub fn release_voice(&self, voice_idx: usize) {
        if voice_idx < MAX_VOICES {
            self.voice_commands[voice_idx]
                .store(pack_cmd(CMD_RELEASE, 0), Ordering::Relaxed);
        }
    }

    /// Find a free voice. Prefers voices whose envelope has finished (inactive).
    /// If all are active, steals voice 0.
    fn find_free_voice(&self) -> usize {
        for i in 0..MAX_VOICES {
            if !self.voice_active[i].load(Ordering::Relaxed) {
                return i;
            }
        }
        // All voices active — steal voice 0
        0
    }

    pub fn set_waveform(&self, waveform: Waveform) {
        self.waveform.store(waveform as u8, Ordering::Relaxed);
    }

    pub fn waveform(&self) -> Waveform {
        Waveform::from_u8(self.waveform.load(Ordering::Relaxed))
    }

    pub fn set_adsr(&self, params: AdsrParams) {
        self.adsr_packed.store(pack_adsr(&params), Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn adsr(&self) -> AdsrParams {
        unpack_adsr(self.adsr_packed.load(Ordering::Relaxed))
    }
}
