//! Karplus-Strong plucked-string synthesis.
//!
//! Originally published by Karplus & Strong in 1983. Despite being one of the
//! simplest synthesis techniques on paper, it produces remarkably realistic
//! plucked-string sounds — guitar, harp, koto, banjo — for very little cost.
//!
//! The idea: a delay line of length L = sample_rate / frequency is filled with
//! random noise (the "pluck"). On each sample:
//!
//!   1. Read the oldest sample from the buffer (this is the output)
//!   2. Apply a one-zero lowpass to it (the "string damping")
//!   3. Multiply by a slight decay (the "string losses")
//!   4. Write the result back into the buffer at the same position
//!
//! The buffer becomes a self-feeding, slowly-darkening, exponentially-decaying
//! resonator. The initial noise has all frequencies; the lowpass progressively
//! kills the high ones, which is why a real plucked string starts bright and
//! mellows out.
//!
//! Reference: Karplus & Strong, "Digital Synthesis of Plucked-String and Drum
//! Timbres", Computer Music Journal 1983.

/// Maximum delay-line length in samples. At 44.1 kHz this supports notes down
/// to ~21.5 Hz (well below human hearing's lower edge / lowest piano note).
const MAX_LEN: usize = 2048;

pub struct KarplusStrong {
    buffer: [f32; MAX_LEN],
    /// Position to read-then-write next (acts as both read and write head).
    idx: usize,
    /// Active delay line length, set on each trigger from f_s / freq.
    length: usize,
    /// Per-loop multiplier (slightly < 1.0). Lower = shorter sustain.
    decay: f32,
    /// Lowpass mix between current and previous sample. 0.5 = textbook K-S
    /// (max smoothing); higher → brighter / longer sustain on highs;
    /// lower → mellower.
    brightness: f32,
    /// xorshift32 state for the noise burst.
    rng: u32,
}

impl KarplusStrong {
    pub fn new(seed: u32) -> Self {
        Self {
            buffer: [0.0; MAX_LEN],
            idx: 0,
            length: 2,
            decay: 0.996,
            brightness: 0.5,
            rng: seed.max(1),
        }
    }

    /// `decay` is the per-loop amplitude multiplier (typical 0.99–0.999).
    /// Lower values → faster decay, more "muted" string.
    /// `brightness` is 0..1 (0.5 = textbook); higher = ringy/bright.
    pub fn set_params(&mut self, decay: f32, brightness: f32) {
        self.decay = decay.clamp(0.5, 0.9999);
        self.brightness = brightness.clamp(0.0, 1.0);
    }

    /// Pluck the string: fill the delay line with fresh noise scaled by
    /// `velocity`. Length is set from frequency.
    pub fn trigger(&mut self, freq: f32, sample_rate: f32, velocity: f32) {
        let len = (sample_rate / freq).round() as usize;
        self.length = len.clamp(2, MAX_LEN);
        for i in 0..self.length {
            self.buffer[i] = self.next_noise() * velocity;
        }
        self.idx = 0;
    }

    fn next_noise(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn next_sample(&mut self) -> f32 {
        let cur = self.buffer[self.idx];
        let prev_idx = if self.idx == 0 { self.length - 1 } else { self.idx - 1 };
        let prev = self.buffer[prev_idx];
        // Weighted lowpass: brightness=0.5 is the symmetric textbook average,
        // higher values keep more of the current (less filtering / brighter).
        let filtered = self.brightness * cur + (1.0 - self.brightness) * prev;
        self.buffer[self.idx] = filtered * self.decay;
        self.idx = (self.idx + 1) % self.length;
        cur
    }
}
