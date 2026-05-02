//! Resonant lowpass filter (2-pole, 12 dB/oct) using Andrew Simper's TPT
//! state-variable topology.
//!
//! Stable up to Nyquist (unlike the textbook Chamberlin SVF which blows up
//! near the high end), zero-delay feedback handled analytically, and all
//! three classic outputs (LP/BP/HP) fall out of the same structure. We only
//! expose the lowpass for now since that's the workhorse for subtractive
//! synthesis; HP and BP are essentially free to add later.
//!
//! Reference: https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf

use std::f32::consts::PI;

pub struct SvfLowpass {
    ic1eq: f32,
    ic2eq: f32,
}

impl SvfLowpass {
    pub fn new() -> Self {
        Self { ic1eq: 0.0, ic2eq: 0.0 }
    }

    /// Process one sample.
    ///
    /// `cutoff_hz` is clamped to a safe range (>= 10 Hz, <= Nyquist - 100).
    /// `resonance` is clamped to `[0.0, 0.97]` — at 1.0 the filter
    /// self-oscillates and can blow up.
    pub fn process(
        &mut self,
        input: f32,
        cutoff_hz: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> f32 {
        let nyquist = sample_rate * 0.5;
        let cutoff = cutoff_hz.clamp(10.0, nyquist - 100.0);
        let res = resonance.clamp(0.0, 0.97);

        // Bilinear-prewarped cutoff coefficient.
        let g = (PI * cutoff / sample_rate).tan();
        // k = 1/Q, mapped from resonance: 0 → k=2 (Q=0.5, no resonance),
        // 0.97 → k=0.06 (very resonant, just shy of self-oscillation).
        let k = 2.0 - 2.0 * res;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        // Lowpass output (v2 = ic2eq state). Bandpass would be v1, highpass
        // would be input - k*v1 - v2 — left out for now.
        v2
    }
}
