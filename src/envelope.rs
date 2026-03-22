/// ADSR Envelope Generator.
///
/// An envelope shapes a note's volume over time through four stages:
///
///   Volume
///     │
///   1 │   /\
///     │  /  \
///   S │ /    \_______________
///     │/     D       S       \
///   0 │                       \
///     └────────────────────────── Time
///      A                      R
///
/// - **Attack**: Time to ramp from 0 to full volume (note pressed)
/// - **Decay**: Time to fall from full volume to the sustain level
/// - **Sustain**: Volume level held while key is held (0.0–1.0)
/// - **Release**: Time to fade from sustain to silence (note released)
///
/// The envelope is a state machine that advances one step per audio sample.
/// This means all timing is sample-accurate — no timer jitter.

/// The four ADSR parameters, stored as simple values.
/// Attack, Decay, Release are in seconds. Sustain is a level (0.0–1.0).
#[derive(Clone, Copy)]
pub struct AdsrParams {
    pub attack: f32,  // seconds
    pub decay: f32,   // seconds
    pub sustain: f32, // level 0.0–1.0
    pub release: f32, // seconds
}

impl Default for AdsrParams {
    fn default() -> Self {
        AdsrParams {
            attack: 0.05,  // 50ms — snappy but not instant
            decay: 0.12,   // 120ms
            sustain: 0.7,  // 70% — natural level for held notes
            release: 0.2,  // 200ms — smooth fade-out
        }
    }
}

/// Which stage the envelope is currently in.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Idle,    // Silent, waiting for a note
    Attack,  // Ramping up to full volume
    Decay,   // Falling to sustain level
    Sustain, // Holding at sustain level
    Release, // Fading to silence after note release
}

/// The envelope state machine. Lives inside the audio callback.
///
/// Call `gate_on()` when a note starts, `gate_off()` when it's released.
/// Call `next_sample()` each audio sample to get the current amplitude (0.0–1.0).
pub struct Envelope {
    stage: Stage,
    /// Current amplitude (0.0–1.0)
    amplitude: f32,
    /// How much amplitude changes per sample in the current stage
    rate: f32,
    /// Cached params and sample rate for recomputing rates
    params: AdsrParams,
    sample_rate: f32,
}

impl Envelope {
    pub fn new(sample_rate: f32) -> Self {
        Envelope {
            stage: Stage::Idle,
            amplitude: 0.0,
            rate: 0.0,
            params: AdsrParams::default(),
            sample_rate,
        }
    }

    /// Update the ADSR parameters. Takes effect on the next gate_on/gate_off.
    pub fn set_params(&mut self, params: AdsrParams) {
        self.params = params;
    }

    /// Note pressed — start the attack phase.
    /// If we're already in the middle of a note, restart from the current amplitude
    /// (this prevents a click from jumping to 0).
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
        // Rate = how much amplitude increases per sample to reach 1.0 during attack time.
        // If attack is 0, we jump instantly to full volume.
        let attack_samples = self.params.attack * self.sample_rate;
        if attack_samples > 0.0 {
            self.rate = (1.0 - self.amplitude) / attack_samples;
        } else {
            self.amplitude = 1.0;
            self.start_decay();
        }
    }

    /// Note released — start the release phase from whatever amplitude we're at.
    pub fn gate_off(&mut self) {
        if self.stage == Stage::Idle {
            return;
        }
        self.stage = Stage::Release;
        let release_samples = self.params.release * self.sample_rate;
        if release_samples > 0.0 {
            self.rate = -self.amplitude / release_samples;
        } else {
            self.amplitude = 0.0;
            self.stage = Stage::Idle;
        }
    }

    /// Advance the envelope by one sample and return the current amplitude.
    /// Called once per sample in the audio callback — must be fast.
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => 0.0,
            Stage::Attack => {
                self.amplitude += self.rate;
                if self.amplitude >= 1.0 {
                    self.amplitude = 1.0;
                    self.start_decay();
                }
                self.amplitude
            }
            Stage::Decay => {
                self.amplitude += self.rate;
                if self.amplitude <= self.params.sustain {
                    self.amplitude = self.params.sustain;
                    self.stage = Stage::Sustain;
                }
                self.amplitude
            }
            Stage::Sustain => {
                // Just hold at sustain level until gate_off
                self.params.sustain
            }
            Stage::Release => {
                self.amplitude += self.rate; // rate is negative
                if self.amplitude <= 0.0 {
                    self.amplitude = 0.0;
                    self.stage = Stage::Idle;
                }
                self.amplitude
            }
        }
    }

    /// Is the envelope completely silent?
    #[allow(dead_code)]
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    fn start_decay(&mut self) {
        self.stage = Stage::Decay;
        let decay_samples = self.params.decay * self.sample_rate;
        if decay_samples > 0.0 {
            self.rate = (self.params.sustain - 1.0) / decay_samples;
        } else {
            self.amplitude = self.params.sustain;
            self.stage = Stage::Sustain;
        }
    }
}
