//! Schroeder reverberator — 4 parallel feedback comb filters into 2 series
//! allpass filters. The classic 1962 design that sounds far better than its
//! ~80 lines of code suggest.
//!
//! # How it works
//!
//! A real room reverb is a dense cloud of thousands of decaying echoes (the
//! sound bouncing off walls, furniture, ceiling). To synthesize that:
//!
//! - A **feedback comb filter** is a delay line that feeds back into itself
//!   with attenuation: `y[n] = x[n] + g · y[n − D]`. One comb gives a regular
//!   periodic echo. Stack 4 of them in parallel with mutually-prime delay
//!   lengths and the echoes interleave, building density.
//! - An **allpass filter** has flat magnitude response but smears the phase.
//!   Running the comb output through 2 allpass filters in series scatters the
//!   echoes further without changing the spectral balance, producing the
//!   smooth "wash" that makes reverb sound like a room rather than a flutter.
//!
//! Both are really just delay lines with a tiny bit of math — circular buffers
//! holding the last `D` samples, plus one multiply-and-add per sample.
//!
//! # Tuning
//!
//! Default delay times (in samples at 44.1 kHz) follow the Freeverb-ish
//! tradition: small mutually-prime numbers around 25–30 ms for combs and
//! 10–12 ms for allpass. Comb feedback ~0.84 gives roughly a 1.5 s tail.

const SAMPLE_RATE: f32 = 44_100.0;

/// Comb-filter delay lengths in samples. Mutually prime numbers (no shared
/// factors) so the echoes don't coincide and create flutter.
const COMB_DELAYS: [usize; 4] = [1116, 1188, 1277, 1356];
/// Allpass-filter delay lengths in samples.
const ALLPASS_DELAYS: [usize; 2] = [556, 441];
/// Comb feedback coefficient — controls reverb tail length. Higher = longer.
const COMB_FEEDBACK: f32 = 0.84;
/// Allpass coefficient — controls echo density / smoothness.
const ALLPASS_GAIN: f32 = 0.5;
/// One-pole lowpass damping inside each comb's feedback path. 0.0 = no
/// damping (bright, metallic ring); 1.0 = full damping (no feedback at all).
/// Real rooms absorb high frequencies faster than lows, so each echo bounce
/// gets duller. Without this, a Schroeder reverb has the characteristic
/// "hollow / metallic" sound. ~0.2 sounds like a small carpeted room.
const COMB_DAMP: f32 = 0.2;

/// One feedback comb filter with one-pole lowpass damping in the loop.
///
/// Without damping (`damp = 0`) this is the textbook Schroeder comb:
/// `y[n] = x[n] + g · y[n − D]`. With damping, the fed-back delayed sample
/// is first lowpassed by a one-pole IIR (`f[n] = (1 − d) · y[n − D] + d ·
/// f[n − 1]`), so each successive echo is duller than the last — exactly
/// what real rooms do, and what removes the metallic ring.
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damp: f32,
    /// One-pole lowpass state: previous filtered sample.
    lp_state: f32,
}

impl Comb {
    fn new(delay: usize, feedback: f32, damp: f32) -> Self {
        Comb {
            buffer: vec![0.0; delay],
            index: 0,
            feedback,
            damp,
            lp_state: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.index];
        // One-pole lowpass on the feedback path: lp_state slides toward
        // `delayed` more sluggishly when damp is high.
        self.lp_state = delayed * (1.0 - self.damp) + self.lp_state * self.damp;
        let output = input + self.lp_state * self.feedback;
        self.buffer[self.index] = output;
        self.index = (self.index + 1) % self.buffer.len();
        delayed
    }
}

/// One allpass filter — flat magnitude, scattered phase.
///
/// `y[n] = -g · x[n] + d[n − D] + g · y[n − D]`, implemented in canonical
/// form using one delay line and a feedback term. The transfer function has
/// |H| = 1 at every frequency (hence "allpass"); only the timing changes.
struct Allpass {
    buffer: Vec<f32>,
    index: usize,
    gain: f32,
}

impl Allpass {
    fn new(delay: usize, gain: f32) -> Self {
        Allpass {
            buffer: vec![0.0; delay],
            index: 0,
            gain,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.index];
        let output = -input + delayed;
        // Feedback the input + g·delayed back into the buffer for the next round.
        self.buffer[self.index] = input + delayed * self.gain;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// A simple Schroeder reverb. Process one sample at a time via [`process`].
///
/// `mix` is the wet/dry blend, 0.0 = fully dry, 1.0 = fully wet. Anything
/// above ~0.4 starts to sound like the listener is underwater; for music
/// 0.15–0.3 is the typical range.
pub struct Reverb {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
    mix: f32,
}

impl Reverb {
    /// Build a reverb with the default Schroeder/Freeverb-ish tuning. The
    /// delay constants are sized for 44.1 kHz; if the engine ever runs at a
    /// different rate, the room size will scale slightly but the result is
    /// still musical.
    pub fn new(mix: f32) -> Self {
        let combs = COMB_DELAYS
            .iter()
            .map(|&d| Comb::new(d, COMB_FEEDBACK, COMB_DAMP))
            .collect();
        let allpasses = ALLPASS_DELAYS
            .iter()
            .map(|&d| Allpass::new(d, ALLPASS_GAIN))
            .collect();
        Reverb {
            combs,
            allpasses,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn mix(&self) -> f32 {
        self.mix
    }

    /// Process one sample. Returns the wet/dry mix.
    pub fn process(&mut self, input: f32) -> f32 {
        if self.mix <= 0.0 {
            return input;
        }
        // Sum the four comb outputs in parallel — this builds echo density.
        let mut wet: f32 = 0.0;
        for c in &mut self.combs {
            wet += c.process(input);
        }
        wet *= 0.25; // Average the four combs back to roughly unity gain.

        // Run through the allpasses in series to smooth out the comb pattern.
        for a in &mut self.allpasses {
            wet = a.process(wet);
        }

        // Wet/dry blend.
        input * (1.0 - self.mix) + wet * self.mix
    }
}

/// Sample rate the reverb was tuned at. Other modules can reference this if
/// they want to scale-correct delay times for non-44.1 kHz output.
#[allow(dead_code)]
pub fn tuned_sample_rate() -> f32 {
    SAMPLE_RATE
}
