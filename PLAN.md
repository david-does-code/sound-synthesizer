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

### Phase 2: Waveforms & Timbre

- [ ] Add selectable waveforms: square, sawtooth, triangle
- [ ] Understand why different waveforms sound different (harmonic content)
- [ ] Visualize waveforms in the terminal
- [ ] Implement octave shifting (extend playable range)

**Concepts to learn**: harmonics and overtones, Fourier series (any waveform is a sum of
sine waves), timbre — why a piano and a flute playing the same note sound different.

### Phase 3: Envelopes (ADSR)

- [ ] Implement Attack-Decay-Sustain-Release envelopes
- [ ] Make notes fade in and out naturally instead of abrupt on/off
- [ ] Add adjustable envelope parameters

**Concepts to learn**: ADSR model, how real instruments have characteristic amplitude
shapes (a plucked guitar vs a bowed violin vs a struck piano).

### Phase 4: Polyphony

- [ ] Play multiple notes simultaneously (chords)
- [ ] Mix multiple oscillators without clipping
- [ ] Implement voice allocation and stealing

**Concepts to learn**: additive mixing, gain staging, voice management, basic chord theory
(major, minor, seventh chords).

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
