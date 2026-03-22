# Sound Synthesizer — Learning Plan

A project to learn Rust and music theory by building a sound synthesizer from scratch,
inspired by [Sebastian Lague's video on synthesizing musical instruments](https://www.youtube.com/watch?v=rRnOtKlg4jA).

## Learning Goals

- **Rust**: Systems programming, real-time audio, concurrency, and the ownership model
- **Music theory**: How sound works physically, scales, harmony, and what makes instruments sound different
- **DSP fundamentals**: Digital signal processing concepts that underpin all audio software

## Roadmap

### Phase 1: Foundations ✅

- [x] Generate a pure sine wave and output to speakers
- [x] Save audio to WAV files
- [x] Map keyboard keys to musical notes (MIDI → frequency)
- [x] Build an interactive terminal piano with real-time playback
- [x] Read keyboard press/release events via Linux evdev

**Concepts learned**: sample rate, Nyquist theorem, phase accumulation, sine oscillation,
MIDI note numbers, equal temperament tuning, semitones, the piano key layout,
real-time audio callbacks, lock-free concurrency with atomics.

### Phase 2: Waveforms & Timbre ✅

- [x] Add selectable waveforms: square, sawtooth, triangle
- [x] Understand why different waveforms sound different (harmonic content)
- [x] Visualize waveforms in the terminal (braille-character renderer)
- [x] Implement octave shifting (Z/X keys, range -3 to +3)

**Concepts learned**: harmonics and overtones, Fourier series (any waveform is a sum of
sine waves), timbre — why a piano and a flute playing the same note sound different.
Each waveform's harmonic recipe: sine (none), square (odd at 1/n), saw (all at 1/n),
triangle (odd at 1/n²). Octave relationships (each octave doubles frequency, ±12 MIDI notes).
Evdev scancodes are physical keys, not characters — important for keyboard-layout independence.

### Phase 3: Envelopes (ADSR) ✅

- [x] Implement Attack-Decay-Sustain-Release envelope state machine
- [x] Make notes fade in and out naturally instead of abrupt on/off
- [x] Add adjustable envelope parameters
- [x] Interactive ADSR editor with braille envelope visualization
- [x] Live preview — play notes while adjusting parameters

**Concepts learned**: ADSR model (attack ramp, decay slope, sustain level, release fade),
how real instruments have characteristic amplitude shapes, gate signal for triggering
envelope stages, per-sample state machine in the audio callback, lock-free parameter
transfer via packed AtomicU64.

### Phase 4: Polyphony ✅

- [x] Play multiple notes simultaneously (chords) — 8 voices
- [x] Mix multiple oscillators without clipping (gain scaling by √MAX_VOICES)
- [x] Implement voice allocation and stealing (oldest voice stolen when all busy)

**Concepts learned**: additive mixing (summing voice samples), gain staging (scaling to
prevent clipping), voice allocation (finding free slots via shared AtomicBool flags),
voice stealing (reusing the oldest voice when all are busy), the distinction between
"no pending command" and "voice is free" in lock-free designs.

### Phase 5: Filters & Subtractive Synthesis

- [ ] Implement low-pass, high-pass, and band-pass filters
- [ ] Build a subtractive synthesizer (start with rich waveform, sculpt with filters)
- [ ] Add filter cutoff and resonance controls

**Concepts to learn**: frequency spectrum, filter types and their effect on timbre,
subtractive synthesis (the approach used by classic analog synths like the Moog).

### Phase 6: Effects

- [ ] Delay / echo
- [ ] Reverb (simulating room acoustics)
- [ ] Chorus / detune

**Concepts to learn**: delay lines, convolution, feedback loops, how physical spaces
shape sound.

### Phase 7: Instrument Synthesis

- [ ] Use additive synthesis to approximate real instruments
- [ ] Combine oscillators, envelopes, and filters to create presets
- [ ] Record and export compositions to WAV

**Concepts to learn**: spectral analysis of real instruments, formants, how to
decompose and reconstruct the character of an instrument from its harmonics.

## Stretch Goals

- [ ] Full TUI with ratatui (waveform display, knobs, keyboard visualization)
- [ ] MIDI input support (connect a real MIDI keyboard)
- [ ] Sequencer / pattern editor
- [ ] Wavetable synthesis
- [ ] FM synthesis
