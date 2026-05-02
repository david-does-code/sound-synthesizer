use crate::envelope::{AdsrParams, Envelope};
use crate::reverb::Reverb;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

/// Maximum number of simultaneous voices (notes).
pub const MAX_VOICES: usize = 8;

/// Drum kinds the engine can synthesize. One dedicated drum voice per kind —
/// drums are short and percussive, retriggering is fine without polyphony.
#[derive(Copy, Clone, Debug)]
#[repr(usize)]
pub enum Drum {
    Kick = 0,
    Snare = 1,
    HiHat = 2,
}

pub const NUM_DRUMS: usize = 3;

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

/// Voice commands sent from main thread → audio callback (immediate, used by piano mode).
const CMD_IDLE: u32 = 0;
const CMD_PLAY: u32 = 1;
const CMD_RELEASE: u32 = 2;

fn pack_cmd(cmd: u32, midi: u8) -> u32 {
    (cmd << 16) | (midi as u32)
}

fn unpack_cmd(packed: u32) -> (u32, u8) {
    (packed >> 16, (packed & 0xFF) as u8)
}

/// Sample-accurate scheduled voice events used by the sequencer.
///
/// Packed into a single AtomicU64 so reads/writes are atomic and ordering-free:
/// - bits  0..40: target audio sample (u40 — ~7 hours at 44.1 kHz)
/// - bits 40..48: velocity (u8, 0–255 mapped to 0.0–1.0)
/// - bits 48..56: MIDI note (u8)
/// - bits 56..64: event kind (`EVENT_*` below)
///
/// Event kind 0 means "slot empty / no pending event".
const EVENT_NONE: u8 = 0;
const EVENT_PLAY: u8 = 1;
const EVENT_RELEASE: u8 = 2;

fn pack_voice_event(kind: u8, target_sample: u64, midi: u8, velocity: f32) -> u64 {
    debug_assert!(target_sample < (1u64 << 40));
    let vel_u8 = (velocity.clamp(0.0, 1.0) * 255.0) as u64;
    (target_sample & 0xFF_FFFF_FFFF)
        | (vel_u8 << 40)
        | ((midi as u64) << 48)
        | ((kind as u64) << 56)
}

fn unpack_voice_event(packed: u64) -> (u8, u64, u8, f32) {
    let target = packed & 0xFF_FFFF_FFFF;
    let vel = ((packed >> 40) & 0xFF) as f32 / 255.0;
    let midi = ((packed >> 48) & 0xFF) as u8;
    let kind = ((packed >> 56) & 0xFF) as u8;
    (kind, target, midi, vel)
}

/// State for a single drum voice inside the audio callback.
///
/// Each drum is synthesized from primitives:
/// - Kick: pitch-swept sine (150 Hz → 40 Hz) with exponential amplitude decay
/// - Snare: white noise with fast exponential decay
/// - HiHat: white noise with very fast exponential decay
///
/// Phase is accumulated incrementally rather than recomputed from `t` so the
/// pitch sweep stays continuous with no discontinuities.
///
/// Public so the offline WAV renderer can reuse the same synthesis math as
/// the live engine.
pub struct DrumVoice {
    kind: Drum,
    sample_rate: f32,
    /// Sample index since trigger; `u32::MAX` means inactive.
    sample_idx: u32,
    /// Oscillator phase 0..1, used by Kick.
    phase: f32,
    /// xorshift32 state for noise generation.
    rng: u32,
    /// Velocity of the current hit (0.0–1.0).
    velocity: f32,
    /// One-pole lowpass state for filtered noise (used by HiHat to tame the
    /// screechy top end of pure white noise).
    lp_state: f32,
}

impl DrumVoice {
    pub fn new(kind: Drum, sample_rate: f32, seed: u32) -> Self {
        DrumVoice {
            kind,
            sample_rate,
            sample_idx: u32::MAX,
            phase: 0.0,
            rng: seed,
            velocity: 1.0,
            lp_state: 0.0,
        }
    }

    pub fn trigger_with_velocity(&mut self, vel: f32) {
        self.sample_idx = 0;
        self.phase = 0.0;
        self.velocity = vel;
    }

    #[allow(dead_code)]
    pub fn trigger(&mut self) {
        self.trigger_with_velocity(1.0);
    }

    fn noise(&mut self) -> f32 {
        // xorshift32 — fast, deterministic, good enough for percussion noise.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn next_sample(&mut self) -> f32 {
        if self.sample_idx == u32::MAX {
            return 0.0;
        }
        let t = self.sample_idx as f32 / self.sample_rate;

        // Each drum's recipe computes (amplitude, raw_signal). The voice is
        // deactivated only once the amplitude has decayed below a silence
        // threshold — never with a hard time cutoff — so there's no click at
        // the tail and adjacent hits don't collide audibly.
        const SILENCE: f32 = 0.001;

        let (amp, signal) = match self.kind {
            Drum::Kick => {
                // Pitch sweeps from ~150 Hz down to ~40 Hz exponentially.
                // Amplitude decay is fast enough to be nearly silent by 300ms.
                let freq = 40.0 + 110.0 * (-t * 30.0).exp();
                let amp = (-t * 14.0).exp();
                self.phase += freq / self.sample_rate;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                (amp, (self.phase * TAU).sin())
            }
            Drum::Snare => {
                // Mix a body tone with noise for a more drum-like character.
                let body_freq = 180.0;
                self.phase += body_freq / self.sample_rate;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                let body = (self.phase * TAU).sin();
                let signal = self.noise() * 0.7 + body * 0.3;
                let amp = (-t * 22.0).exp();
                (amp, signal)
            }
            Drum::HiHat => {
                // White noise is flat to 22 kHz; real closed hi-hats peak
                // around 8-10 kHz and roll off sharply above that. A one-pole
                // lowpass at ~9 kHz tames the screechy ultra-highs and leaves
                // the metallic "tss" band intact. Coefficient α tuned by ear:
                // α=0.55 gives roughly a 9 kHz corner at 44.1 kHz sample rate.
                let raw = self.noise();
                self.lp_state = 0.55 * raw + 0.45 * self.lp_state;
                // Slightly slower decay (50 vs 70) so it has a bit more body.
                let amp = (-t * 50.0).exp() * 0.6;
                (amp, self.lp_state)
            }
        };

        if amp < SILENCE {
            self.sample_idx = u32::MAX;
            return 0.0;
        }

        self.sample_idx = self.sample_idx.saturating_add(1);
        signal * amp * self.velocity
    }
}

/// A clonable, Send + Sync control surface for the audio engine, designed for
/// sample-accurate scheduling from another thread (the sequencer).
///
/// It exposes:
/// - **Drums** — `schedule_at` writes a drum's target sample into a per-drum slot
///   that the audio callback checks every frame.
/// - **Pitched voices** — `schedule_note_on` / `schedule_note_off` write a packed
///   event (kind, sample, midi) into a per-voice slot, also checked every frame.
/// - **Clock readout** — `current_sample()` lets the sequencer read the audio
///   callback's current position so it can compute future sample numbers.
///
/// Single-slot scheduling means the contract is: **don't schedule a new event for
/// the same drum/voice until the previous one has fired**. With ~100 ms of
/// scheduler lookahead and step intervals well above the audio buffer size,
/// this holds for any reasonable tempo.
#[derive(Clone)]
pub struct EngineHandle {
    drum_schedule: Arc<[AtomicU64; NUM_DRUMS]>,
    voice_events: Arc<[AtomicU64; MAX_VOICES]>,
    voice_waveforms: Arc<[AtomicU8; MAX_VOICES]>,
    voice_adsr: Arc<[AtomicU64; MAX_VOICES]>,
    voice_gains: Arc<[AtomicU32; MAX_VOICES]>,
    /// Per-voice gate duration in samples. 0 = legato (no auto-release).
    /// Read by the callback on each note-on to set a countdown timer.
    voice_gate: Arc<[AtomicU64; MAX_VOICES]>,
    /// Per-voice hammer-click amount in semitones (f32 bits as u32).
    /// 0.0 = no transient. Read once on each note-on.
    voice_click: Arc<[AtomicU32; MAX_VOICES]>,
    /// Per-voice sub-octave sine layer amplitude (f32 bits). 0.0 = off.
    voice_sub: Arc<[AtomicU32; MAX_VOICES]>,
    drum_gains: Arc<[AtomicU32; NUM_DRUMS]>,
    sample_clock: Arc<AtomicU64>,
    /// Master reverb wet/dry mix as f32 bits. Read once per frame in the
    /// audio callback and pushed into the Reverb instance.
    reverb_mix: Arc<AtomicU32>,
    sample_rate: f32,
}

impl EngineHandle {
    /// The current audio sample position (frames generated since stream start).
    pub fn current_sample(&self) -> u64 {
        self.sample_clock.load(Ordering::Relaxed)
    }

    /// Audio output sample rate (samples per second).
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Schedule a drum to fire at the given absolute audio sample number.
    /// Velocity defaults to 1.0.
    #[allow(dead_code)]
    pub fn schedule_at(&self, drum: Drum, target_sample: u64) {
        self.schedule_drum_at(drum, target_sample, 1.0);
    }

    /// Schedule a drum with a specific velocity (0.0–1.0).
    pub fn schedule_drum_at(&self, drum: Drum, target_sample: u64, velocity: f32) {
        // Pack: bits 0..48 = target sample, bits 48..64 = velocity as u16.
        let vel_u16 = (velocity.clamp(0.0, 1.0) * 65535.0) as u64;
        let packed = (target_sample.max(1) & 0xFFFF_FFFF_FFFF) | (vel_u16 << 48);
        self.drum_schedule[drum as usize].store(packed, Ordering::Relaxed);
    }

    /// Trigger a drum as soon as possible (used for live, non-sequenced playback).
    #[allow(dead_code)]
    pub fn trigger(&self, drum: Drum) {
        self.schedule_at(drum, self.current_sample() + 1);
    }

    /// Schedule a note-on (gate-on with new pitch) for `voice_idx` at the given
    /// absolute audio sample. Velocity defaults to 1.0.
    #[allow(dead_code)]
    pub fn schedule_note_on(&self, voice_idx: usize, target_sample: u64, midi: u8) {
        self.schedule_note_on_vel(voice_idx, target_sample, midi, 1.0);
    }

    /// Schedule a note-on with explicit velocity (0.0–1.0).
    pub fn schedule_note_on_vel(&self, voice_idx: usize, target_sample: u64, midi: u8, velocity: f32) {
        if voice_idx < MAX_VOICES {
            let packed = pack_voice_event(EVENT_PLAY, target_sample, midi, velocity);
            self.voice_events[voice_idx].store(packed, Ordering::Relaxed);
        }
    }

    /// Schedule a note-off (gate-off, ADSR enters Release) for `voice_idx`
    /// at the given absolute audio sample.
    pub fn schedule_note_off(&self, voice_idx: usize, target_sample: u64) {
        if voice_idx < MAX_VOICES {
            let packed = pack_voice_event(EVENT_RELEASE, target_sample, 0, 0.0);
            self.voice_events[voice_idx].store(packed, Ordering::Relaxed);
        }
    }

    /// Set the waveform used by a single voice. Used by the sequencer to give
    /// each track its own timbre. Takes effect on the next sample boundary.
    pub fn set_voice_waveform(&self, voice_idx: usize, waveform: Waveform) {
        if voice_idx < MAX_VOICES {
            self.voice_waveforms[voice_idx].store(waveform as u8, Ordering::Relaxed);
        }
    }

    /// Set per-voice ADSR parameters. Used by the sequencer so each track can
    /// have its own envelope shape.
    pub fn set_voice_adsr(&self, voice_idx: usize, params: AdsrParams) {
        if voice_idx < MAX_VOICES {
            self.voice_adsr[voice_idx].store(pack_adsr(&params), Ordering::Relaxed);
        }
    }

    /// Set per-voice gain (0.0–∞, typically 0.0–2.0). Default is 1.0.
    pub fn set_voice_gain(&self, voice_idx: usize, gain: f32) {
        if voice_idx < MAX_VOICES {
            self.voice_gains[voice_idx].store(gain.to_bits(), Ordering::Relaxed);
        }
    }

    /// Set per-drum gain (0.0–∞, typically 0.0–2.0). Default is 1.0.
    pub fn set_drum_gain(&self, drum: Drum, gain: f32) {
        self.drum_gains[drum as usize].store(gain.to_bits(), Ordering::Relaxed);
    }

    /// Set per-voice gate duration in samples. The audio callback will
    /// auto-release the voice this many samples after each note-on.
    /// 0 = legato (no auto-release, hold until explicit note-off).
    pub fn set_voice_gate(&self, voice_idx: usize, samples: u64) {
        if voice_idx < MAX_VOICES {
            self.voice_gate[voice_idx].store(samples, Ordering::Relaxed);
        }
    }

    /// Set per-voice hammer-click amount (semitones of pitch transient on
    /// each note-on). 0.0 = no transient.
    pub fn set_voice_click(&self, voice_idx: usize, semitones: f32) {
        if voice_idx < MAX_VOICES {
            self.voice_click[voice_idx].store(semitones.to_bits(), Ordering::Relaxed);
        }
    }

    /// Set per-voice sub-octave sine layer amplitude (0.0 = off).
    pub fn set_voice_sub(&self, voice_idx: usize, amount: f32) {
        if voice_idx < MAX_VOICES {
            self.voice_sub[voice_idx].store(amount.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        }
    }

    /// Set master reverb wet/dry mix (0.0 = dry, ~0.2–0.3 = roomy).
    pub fn set_reverb_mix(&self, mix: f32) {
        self.reverb_mix.store(mix.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

/// State for a single voice inside the audio callback.
///
/// Public so the offline WAV renderer can reuse the same synthesis math as
/// the live engine. The live engine drives this via lock-free atomics; the
/// offline renderer drives it directly with method calls.
pub struct Voice {
    frequency: f32,
    phase: f32,
    pub envelope: Envelope,
    active: bool,
    /// Per-note velocity (0.0–1.0), set on note-on.
    velocity: f32,
    /// Cached packed ADSR to detect changes without unpacking every buffer.
    last_adsr: u64,
    /// Gate auto-release countdown. When > 0, decrements each sample.
    /// When it reaches 0, gate_off is triggered. Set from voice_gate on note-on.
    pub gate_remaining: u64,
    /// "Hammer click" config: how many semitones above target pitch each note
    /// starts when triggered. 0.0 = no transient. Multiplied per sample by
    /// `click_decay` so the offset fades to ~0 in a few ms.
    click_initial: f32,
    /// Current click offset in semitones. Decays each sample toward 0.
    click_current: f32,
    /// Per-sample multiplier for `click_current`. Computed once from the
    /// configured decay time when `set_click` is called.
    click_decay: f32,
    /// Sub-octave sine layer amplitude (0.0 = off). When >0, the voice mixes
    /// in a sine wave one octave below the main note's frequency at this
    /// amplitude — adds body / warmth.
    sub_amount: f32,
    /// Independent phase accumulator for the sub-octave sine. Reset on trigger.
    sub_phase: f32,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        // Default click decay: 8ms time constant. Multiplier per sample is
        // exp(-1 / (decay_secs * sample_rate)). Recomputed by set_click if
        // the user sets a non-zero click amount.
        let decay = (-1.0_f32 / (0.008 * sample_rate)).exp();
        Voice {
            frequency: 0.0,
            phase: 0.0,
            envelope: Envelope::new(sample_rate),
            active: false,
            velocity: 1.0,
            last_adsr: 0,
            gate_remaining: 0,
            click_initial: 0.0,
            click_current: 0.0,
            click_decay: decay,
            sub_amount: 0.0,
            sub_phase: 0.0,
        }
    }

    /// Configure the per-note pitch transient ("hammer click"). On every
    /// note-on, the voice's pitch starts `semitones` above the target and
    /// exponentially decays back to the target over ~8ms.
    pub fn set_click(&mut self, semitones: f32) {
        self.click_initial = semitones;
    }

    /// Configure the sub-octave sine layer. `amount` is the relative
    /// amplitude (0.0 = off, 1.0 = same level as the main waveform).
    pub fn set_sub(&mut self, amount: f32) {
        self.sub_amount = amount.clamp(0.0, 1.0);
    }

    /// Trigger a note-on directly (for offline rendering).
    pub fn trigger(&mut self, midi: u8, velocity: f32) {
        self.frequency = midi_to_freq(midi);
        self.phase = 0.0;
        self.sub_phase = 0.0;
        self.velocity = velocity;
        self.active = true;
        self.envelope.gate_on();
        self.click_current = self.click_initial;
    }

    /// Release the note (gate-off), entering the release phase.
    pub fn release(&mut self) {
        self.envelope.gate_off();
        self.gate_remaining = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn next_sample(&mut self, waveform: Waveform) -> f32 {
        if !self.active {
            return 0.0;
        }

        let env_amp = self.envelope.next_sample();

        if env_amp <= 0.0 {
            self.active = false;
            return 0.0;
        }

        let mut value = waveform.sample(self.phase);

        // Sub-octave sine layer (skip the math when off).
        if self.sub_amount > 0.0 {
            value += (self.sub_phase * TAU).sin() * self.sub_amount;
            // Sub frequency is one octave (=half) below the main pitch.
            self.sub_phase += self.frequency * 0.5 / self.envelope.sample_rate();
            if self.sub_phase >= 1.0 {
                self.sub_phase -= 1.0;
            }
        }

        value *= env_amp * self.velocity;

        // Apply hammer-click pitch transient if active. Skip the math when
        // the offset is essentially zero (the common case).
        let effective_freq = if self.click_current.abs() > 0.001 {
            // 2^(semitones/12), but exp2 is faster than powf.
            let mult = (self.click_current / 12.0).exp2();
            self.click_current *= self.click_decay;
            self.frequency * mult
        } else {
            self.frequency
        };

        self.phase += effective_freq / self.envelope.sample_rate();
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
    /// Engine-wide default waveform (used by piano mode and as the seed for
    /// new voices). When the user changes the global waveform, all per-voice
    /// slots are updated to match.
    waveform: Arc<AtomicU8>,
    /// Per-voice waveform — lets the sequencer give each track its own timbre.
    /// The audio callback reads from here, not from the global `waveform`.
    voice_waveforms: Arc<[AtomicU8; MAX_VOICES]>,
    adsr_packed: Arc<AtomicU64>,
    /// Per-voice ADSR params — lets the sequencer give each track its own
    /// envelope shape. 0 = use global ADSR.
    voice_adsr: Arc<[AtomicU64; MAX_VOICES]>,
    /// Per-voice gain multiplier (f32 bits stored as u32). Default 1.0.
    voice_gains: Arc<[AtomicU32; MAX_VOICES]>,
    /// Per-voice gate duration in samples (0 = legato). The callback reads
    /// this on note-on and sets a countdown to auto-release the voice.
    voice_gate: Arc<[AtomicU64; MAX_VOICES]>,
    /// Per-voice hammer-click pitch transient in semitones (f32 bits).
    voice_click: Arc<[AtomicU32; MAX_VOICES]>,
    /// Per-voice sub-octave sine amplitude (f32 bits).
    voice_sub: Arc<[AtomicU32; MAX_VOICES]>,
    /// Per-drum gain multiplier (f32 bits stored as u32). Default 1.0.
    drum_gains: Arc<[AtomicU32; NUM_DRUMS]>,
    /// Per-drum scheduled trigger time (absolute audio sample), 0 = none.
    drum_schedule: Arc<[AtomicU64; NUM_DRUMS]>,
    /// Per-voice scheduled events (packed: kind | midi | sample), 0 = none.
    /// Used by the sequencer for sample-accurate note triggering.
    voice_events: Arc<[AtomicU64; MAX_VOICES]>,
    /// Audio callback's current sample position. Used by the sequencer for
    /// sample-accurate scheduling.
    sample_clock: Arc<AtomicU64>,
    /// Master reverb wet/dry mix as f32 bits. Lives outside the callback so
    /// the sequencer / main thread can change it lock-free.
    reverb_mix: Arc<AtomicU32>,
    sample_rate: f32,
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

        let voice_waveforms: Arc<[AtomicU8; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU8::new(Waveform::Sine as u8)),
        );
        let voice_waveforms_clone = voice_waveforms.clone();

        let default_params = AdsrParams::default();
        let adsr_packed = Arc::new(AtomicU64::new(pack_adsr(&default_params)));
        let adsr_clone = adsr_packed.clone();

        // Per-voice ADSR: 0 means "use global ADSR".
        let voice_adsr: Arc<[AtomicU64; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU64::new(0)),
        );
        let voice_adsr_clone = voice_adsr.clone();

        // Per-voice gain (f32 bits as u32). Default 1.0.
        let voice_gains: Arc<[AtomicU32; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU32::new(1.0_f32.to_bits())),
        );
        let voice_gains_clone = voice_gains.clone();

        // Per-voice gate (samples until auto-release, 0 = legato).
        let voice_gate: Arc<[AtomicU64; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU64::new(0)),
        );
        let voice_gate_clone = voice_gate.clone();

        // Per-voice hammer click (semitones, f32 bits, default 0.0).
        let voice_click: Arc<[AtomicU32; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
        );
        let voice_click_clone = voice_click.clone();

        // Per-voice sub-octave amplitude (f32 bits, default 0.0).
        let voice_sub: Arc<[AtomicU32; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
        );
        let voice_sub_clone = voice_sub.clone();

        // Per-drum gain (f32 bits as u32). Default 1.0.
        let drum_gains: Arc<[AtomicU32; NUM_DRUMS]> = Arc::new(
            std::array::from_fn(|_| AtomicU32::new(1.0_f32.to_bits())),
        );
        let drum_gains_clone = drum_gains.clone();

        let mut voices: Vec<Voice> = (0..MAX_VOICES)
            .map(|_| Voice::new(sample_rate))
            .collect();
        let mut prev_adsr_packed = pack_adsr(&default_params);

        let drum_schedule: Arc<[AtomicU64; NUM_DRUMS]> = Arc::new(
            std::array::from_fn(|_| AtomicU64::new(0)),
        );
        let drum_schedule_clone = drum_schedule.clone();

        let voice_events: Arc<[AtomicU64; MAX_VOICES]> = Arc::new(
            std::array::from_fn(|_| AtomicU64::new(0)),
        );
        let voice_events_clone = voice_events.clone();

        let sample_clock = Arc::new(AtomicU64::new(0));
        let sample_clock_clone = sample_clock.clone();

        let reverb_mix = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let reverb_mix_clone = reverb_mix.clone();
        let mut reverb = Reverb::new(0.0);
        // Distinct seeds so each noise drum has its own RNG stream.
        let mut drum_voices: [DrumVoice; NUM_DRUMS] = [
            DrumVoice::new(Drum::Kick, sample_rate, 0x1234_5678),
            DrumVoice::new(Drum::Snare, sample_rate, 0x9E37_79B9),
            DrumVoice::new(Drum::HiHat, sample_rate, 0xBADC_0FFE),
        ];

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Snapshot per-voice waveforms once per buffer (cheap, and
                    // avoids an atomic load per voice per frame).
                    let voice_waves: [Waveform; MAX_VOICES] = std::array::from_fn(|i| {
                        Waveform::from_u8(voice_waveforms_clone[i].load(Ordering::Relaxed))
                    });

                    // Snapshot per-voice gains once per buffer.
                    let vgains: [f32; MAX_VOICES] = std::array::from_fn(|i| {
                        f32::from_bits(voice_gains_clone[i].load(Ordering::Relaxed))
                    });
                    let dgains: [f32; NUM_DRUMS] = std::array::from_fn(|i| {
                        f32::from_bits(drum_gains_clone[i].load(Ordering::Relaxed))
                    });

                    // Update ADSR params if changed — global ADSR applies to
                    // voices that don't have a per-voice override (slot == 0).
                    let current_adsr = adsr_clone.load(Ordering::Relaxed);
                    let global_changed = current_adsr != prev_adsr_packed;
                    if global_changed {
                        prev_adsr_packed = current_adsr;
                    }
                    // Per-voice ADSR: snapshot and apply. A per-voice value of
                    // 0 means "use global".
                    for (i, voice) in voices.iter_mut().enumerate() {
                        let per_voice = voice_adsr_clone[i].load(Ordering::Relaxed);
                        let effective = if per_voice != 0 { per_voice } else { current_adsr };
                        if voice.last_adsr != effective {
                            voice.envelope.set_params(unpack_adsr(effective));
                            voice.last_adsr = effective;
                        }
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

                    // Cache the sample clock locally so we don't pay an atomic
                    // load on every frame.
                    let mut local_clock = sample_clock_clone.load(Ordering::Relaxed);

                    // Pick up any reverb mix change from outside.
                    let new_mix = f32::from_bits(reverb_mix_clone.load(Ordering::Relaxed));
                    if (new_mix - reverb.mix()).abs() > f32::EPSILON {
                        reverb.set_mix(new_mix);
                    }

                    // Generate audio: mix all active voices
                    for frame in data.chunks_mut(channels) {
                        // Sample-accurate drum scheduling: fire any drum whose
                        // scheduled sample has arrived. Cleared after firing.
                        // Packed: bits 0..48 = target sample, bits 48..64 = velocity u16.
                        for (i, slot) in drum_schedule_clone.iter().enumerate() {
                            let packed = slot.load(Ordering::Relaxed);
                            if packed == 0 {
                                continue;
                            }
                            let target = packed & 0xFFFF_FFFF_FFFF;
                            if local_clock >= target {
                                let vel = ((packed >> 48) & 0xFFFF) as f32 / 65535.0;
                                drum_voices[i].trigger_with_velocity(vel);
                                slot.store(0, Ordering::Relaxed);
                            }
                        }

                        // Sample-accurate voice (note) scheduling. Decoded
                        // each frame; very cheap on x86 and never waits.
                        for (i, slot) in voice_events_clone.iter().enumerate() {
                            let packed = slot.load(Ordering::Relaxed);
                            if packed == 0 {
                                continue;
                            }
                            let (kind, target, midi, vel) = unpack_voice_event(packed);
                            if kind == EVENT_NONE || local_clock < target {
                                continue;
                            }
                            match kind {
                                EVENT_PLAY => {
                                    // Pull current click + sub amounts, then
                                    // trigger (which resets per-note state).
                                    let click = f32::from_bits(
                                        voice_click_clone[i].load(Ordering::Relaxed),
                                    );
                                    let sub = f32::from_bits(
                                        voice_sub_clone[i].load(Ordering::Relaxed),
                                    );
                                    voices[i].set_click(click);
                                    voices[i].set_sub(sub);
                                    voices[i].trigger(midi, vel);
                                    voices[i].gate_remaining =
                                        voice_gate_clone[i].load(Ordering::Relaxed);
                                    active_clone[i].store(true, Ordering::Relaxed);
                                }
                                EVENT_RELEASE => {
                                    voices[i].envelope.gate_off();
                                    voices[i].gate_remaining = 0;
                                }
                                _ => {}
                            }
                            slot.store(0, Ordering::Relaxed);
                        }

                        let mut mix = 0.0_f32;

                        for (i, voice) in voices.iter_mut().enumerate() {
                            // Gate auto-release: countdown per sample.
                            if voice.gate_remaining > 0 {
                                voice.gate_remaining -= 1;
                                if voice.gate_remaining == 0 {
                                    voice.envelope.gate_off();
                                }
                            }

                            let was_active = voice.active;
                            mix += voice.next_sample(voice_waves[i]) * vgains[i];
                            // If voice just became inactive, update the shared flag
                            if was_active && !voice.active {
                                active_clone[i].store(false, Ordering::Relaxed);
                            }
                        }

                        // Pitched voices: scale to prevent clipping.
                        let mut value = mix * 0.4 / (MAX_VOICES as f32).sqrt();

                        // Mix in drums with per-drum gain.
                        let mut drum_mix = 0.0_f32;
                        for (i, dv) in drum_voices.iter_mut().enumerate() {
                            drum_mix += dv.next_sample() * dgains[i];
                        }
                        value += drum_mix * 0.5;

                        // Master reverb (no-op when mix is 0.0).
                        value = reverb.process(value);

                        for sample in frame.iter_mut() {
                            *sample = value;
                        }

                        local_clock += 1;
                    }

                    // Publish the advanced clock so the sequencer can read it.
                    sample_clock_clone.store(local_clock, Ordering::Relaxed);
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
            voice_waveforms,
            adsr_packed,
            voice_adsr,
            voice_gains,
            voice_gate,
            voice_click,
            voice_sub,
            drum_gains,
            drum_schedule,
            voice_events,
            sample_clock,
            reverb_mix,
            sample_rate,
        }
    }

    /// Get a clonable, Send + Sync handle for sample-accurate scheduling of
    /// drums and pitched voices.
    pub fn engine_handle(&self) -> EngineHandle {
        EngineHandle {
            drum_schedule: self.drum_schedule.clone(),
            voice_events: self.voice_events.clone(),
            voice_waveforms: self.voice_waveforms.clone(),
            voice_adsr: self.voice_adsr.clone(),
            voice_gains: self.voice_gains.clone(),
            voice_gate: self.voice_gate.clone(),
            voice_click: self.voice_click.clone(),
            voice_sub: self.voice_sub.clone(),
            drum_gains: self.drum_gains.clone(),
            sample_clock: self.sample_clock.clone(),
            reverb_mix: self.reverb_mix.clone(),
            sample_rate: self.sample_rate,
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
        // Broadcast to every voice so the piano (and any future "global"
        // user-facing controls) affect all voices uniformly.
        for slot in self.voice_waveforms.iter() {
            slot.store(waveform as u8, Ordering::Relaxed);
        }
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
