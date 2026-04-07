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

### Phase 5: Step Sequencer, Patterns & Composition 🚧

Build a step sequencer — drums and pitched voices triggered on a grid — together
with a text-based pattern file format that both humans and Claude can read/write.
This is the foundation for making actual music.

**Done so far:**
- [x] Text-based pattern file format with parser (`src/pattern.rs`)
- [x] Synthesized drum voices (kick / snare / hi-hat) inside the audio engine
- [x] BPM / tempo clock with sample-accurate scheduling (`src/sequencer.rs`)
- [x] CLI pattern player: `cargo run -- --play <file.pat>`
- [x] Melodic note tracks: scientific pitch notation (`C4`, `Eb3`, `F#5`),
      sustain (`.`) and rest (`-`) cells, up to 8 simultaneous note tracks
      sharing the pitched voice pool

**Next slices (in build order):**
- [ ] **Slice 5a — Per-track instruments**: each track can declare its own
      waveform (`wave: square`), so bass and lead can have different timbres.
      Requires adding per-voice waveform state to the audio engine (currently
      a single global `AtomicU8`).
- [ ] **Slice 5b — Chord shorthand**: tokens like `Cm`, `G7`, `Dmaj`, `Fsus4`
      expand to multi-note stacks that play simultaneously. Lets Claude compose
      from chord sheets directly. Format: a `chord` track type that grabs
      multiple voices.
- [ ] **Slice 5c — Longer patterns / song chaining**: escape the 1-bar loop.
      Either allow `steps: 64` for multi-bar patterns, or define multiple
      `[pattern_name]` blocks plus a `song:` chain that names them in order
      (intro → verse → chorus → verse → outro).
- [ ] **Slice 5d — Visual grid in the TUI**: third UI mode (after piano/ADSR)
      that shows the pattern grid with a moving playhead.
- [ ] **Slice 5e — TUI editing**: toggle steps with arrow keys + space, save
      back to `.pat` files. Live-reload while playing.
- [ ] Export a played pattern to WAV (offline render).

**Concepts learned so far**: tempo and BPM, beats and bars, 16th-note subdivisions
in 4/4, drum kit anatomy (kick on the downbeat, snare on the backbeat, hi-hat
driving 8ths), four-on-the-floor as the canonical dance rhythm, drum synthesis
from primitives (pitch-swept sine for kick, noise + body tone for snare), why
hard time-cutoffs cause clicks (discontinuity at non-zero amplitude), and
sample-accurate scheduling vs. wall-clock scheduling — the latter suffers from
audio buffer jitter that the ear can hear as one beat being "off". Scientific
pitch notation (middle C = C4 = MIDI 60), packed atomic event encoding for
lock-free voice scheduling, and the C minor pentatonic / Eb major scale flavor
that drives melodic indie/chiptune music.

**Concepts to learn next**: subtractive timbres (sine vs square vs saw character),
chord construction (triads, 7ths, sus chords), inversions, voicing, song form
(intro / verse / chorus / bridge), and the difference between a riff and a song.

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

### Phase 7: Live Performance & Recording

Now that the engine can play composed music, add the tools to capture it and
play along live.

- [ ] Quantization — snap live-played piano notes to the sequencer grid
- [ ] Live recording — play into the sequencer in real time and have it
      written to a pattern file
- [ ] MIDI input support (connect a real MIDI keyboard)
- [ ] Tap-tempo and metronome
- [ ] Export full compositions to WAV (offline render the whole song)

**Concepts to learn**: quantization tradeoffs (loose vs strict), MIDI protocol
basics, click tracks, the relationship between recording and performance.

### Pattern File Format (current)

Human-readable text files that both the TUI and Claude can read/write. The
format supports drum tracks (single-char cells: `x` = hit, `-`/`.` = rest)
and note tracks (whitespace-separated tokens: note names like `C4`, `Eb3`,
`F#5`, plus `-` rest and `.` sustain).

```
# C minor groove with drums and a walking bass + lead riff.
bpm: 110
steps: 16

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

bass:    C2  .  .  .  Eb2 .  .  .  G2  .  .  .  F2  .  .  .
lead:    Eb4 F4 G4 Bb4 C5  .  Bb4 G4 F4  .  Eb4 .  D4  .  C4  .
```

The parser auto-detects per row whether it's a drum or note track. Track
names map to drum kinds (`kick`/`bd`, `snare`/`sd`, `hihat`/`hh`/`hat`) or
become melodic note tracks. Note tracks consume voices in declaration order
from the shared 8-voice pitched pool. Comments with `#` only at the start
of a line (so `F#4` parses as F-sharp 4, not "F" followed by a comment).

Coming soon (slices 5a-5c): per-track `wave:` declarations, `chord` track
type with shorthand like `Cm`, `G7`, `Dmaj`, and either `steps: 64` or a
`song:` chain across multiple `[pattern_name]` blocks.

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
