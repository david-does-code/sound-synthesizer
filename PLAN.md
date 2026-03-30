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

### Phase 5: Step Sequencer & Rhythm

Build a step sequencer — a grid where each row is a sound and each column is a beat.
Classic drum machine style (TR-808). This is the foundation for making actual music.

- [ ] BPM / tempo clock that ticks at a steady rate
- [ ] 16-step pattern grid — toggle steps on/off per sound
- [ ] Visual grid in the TUI — see the playhead move across beats
- [ ] Play/pause/stop controls
- [ ] Text-based pattern file format (load/save)

**Concepts to learn**: tempo and BPM, beats and bars, time signatures (4/4, 3/4),
subdivisions (8th notes, 16th notes), swing/shuffle, the rhythmic grid that
underpins all western music.

### Phase 6: Sampling & Drum Kits

Load WAV files as sound sources instead of only generating waveforms. Combine with
the sequencer to make drum patterns with real sounds.

- [ ] Load short WAV samples (kick, snare, hi-hat, clap, etc.)
- [ ] Pitch-shift samples to play them at different notes
- [ ] Mix synth voices + samples together in the sequencer
- [ ] Bundle a basic drum kit (freely licensed samples)

**Concepts to learn**: PCM audio and sample playback, pitch-frequency relationship,
drum kit anatomy (kick grounds the beat, snare marks the backbeat, hi-hat drives
rhythm), layering sounds.

### Phase 7: Melody, Chords & Composition

Layer melodic and harmonic patterns over drums. Chain patterns into songs.
Support text-based composition so both humans and Claude can write music.

- [ ] Melodic tracks using note names (C4, Eb3, F#5)
- [ ] Chord shorthand that expands to notes (Cm, G7, Dmaj)
- [ ] Multiple pattern tracks — drums + bass + lead
- [ ] Pattern chaining — arrange patterns into a song (intro → verse → chorus)
- [ ] Quantization — snap live-played notes to the grid
- [ ] Export full compositions to WAV

**Concepts to learn**: scales and keys (major/minor), intervals, chord construction
(triads, 7ths), chord progressions (I-IV-V-I, ii-V-I, 12-bar blues), song structure
(verse, chorus, bridge), the role of bass vs lead, call-and-response.

### Pattern File Format

Human-readable text files that both the TUI and Claude can read/write:

```
bpm: 120
time: 4/4
key: C minor

[drums]
kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

[bass]
notes:   C2--G2--Ab2-Eb2-

[lead]
notes:   .---Eb4G4Ab4-G4--
```

This enables Claude to generate patterns from chord sheets, lead sheets, or
descriptions like "write me a 12-bar blues in E".

### Phase 8: Filters & Effects

Once there's actual music to process, add filters and effects as creative tools.

- [ ] Low-pass / high-pass filters with cutoff and resonance
- [ ] Delay / echo
- [ ] Reverb
- [ ] Per-track filter and effect controls

**Concepts to learn**: frequency spectrum, filter types, subtractive synthesis,
delay lines, feedback loops, how physical spaces shape sound.

## Stretch Goals

- [ ] Full TUI with ratatui (waveform display, knobs, keyboard visualization)
- [ ] MIDI input support (connect a real MIDI keyboard)
- [ ] Wavetable synthesis
- [ ] FM synthesis
- [ ] Import chord sheets / lead sheets from text
- [ ] Live recording — play into the sequencer in real time
