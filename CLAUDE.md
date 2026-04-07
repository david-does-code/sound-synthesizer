# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Polyphonic audio engine with 8 pitched voices plus 3 dedicated drum
  voices (kick, snare, hi-hat) synthesized inside the callback. Lock-free communication:
  `voice_commands` (`[AtomicU32; 8]` for play/release), `voice_active` (`[AtomicBool; 8]`),
  waveform (`AtomicU8`), ADSR params (packed `AtomicU64`), `drum_schedule` (`[AtomicU64; 3]`
  holding absolute audio sample numbers for sample-accurate drum triggers), and `sample_clock`
  (`AtomicU64`, advanced per-frame by the callback). Contains `Waveform`, `Drum`, `DrumVoice`,
  and `DrumHandle` (a clonable Send + Sync handle for the sequencer).
- `src/envelope.rs` — ADSR envelope generator. Per-sample state machine (Idle → Attack →
  Decay → Sustain → Release → Idle). Lives inside the audio callback closure.
- `src/keyboard.rs` — Reads raw keyboard events from Linux evdev (`/dev/input/`).
  Sends note, waveform, octave, mode toggle, and arrow key events over an MPSC channel.
- `src/pattern.rs` — Pattern file format and parser. Defines `Pattern`, `Track`, and
  `PatternParseError`. Format is line-based: `bpm:`/`steps:` headers, then `name: x---x---`
  rows. Comments with `#`, blank lines ignored, `x`/`X` = hit, `-`/`.` = rest. Parser
  errors carry line numbers.
- `src/sequencer.rs` — Step sequencer that plays a `Pattern` via a background thread.
  Uses sample-accurate scheduling: pre-computes the absolute audio sample for each step
  and writes it to `DrumHandle::schedule_at`, so playback timing is independent of the
  scheduler thread's wall-clock jitter. Track names map to drum kinds (kick/bd, snare/sd,
  hihat/hh/hat). Lookahead is ~100 ms.
- `src/main.rs` — Two interactive modes (piano + ADSR editor, Tab toggles) plus a CLI
  pattern player: `cargo run -- --play <file.pat>` loads a pattern and plays it in a
  loop until Enter is pressed. `--help` lists usage.
- `src/visualizer.rs` — Renders waveforms and ADSR envelopes using Unicode braille characters
  (2×4 dot grid per character). Shared `render_braille` function for both.
- `src/notes.rs` — Keyboard layout diagram.
- `patterns/` — Example `.pat` files (`four_on_the_floor.pat` and diagnostic patterns).

## Key Design Decisions

- **evdev over terminal input**: Terminals don't send key release events. We read
  `/dev/input/` directly for true press/release, which requires the `input` group.
- **Lock-free polyphony**: 8 voices, each with its own phase/freq/envelope inside the
  callback. The main thread sends play/release commands via `[AtomicU32; 8]`. The callback
  reports voice liveness via `[AtomicBool; 8]` — critical distinction: "no pending command"
  (`CMD_IDLE`) ≠ "voice is free" (`voice_active == false`). Getting this wrong caused all
  notes to steal voice 0.
- **Phase accumulation**: Track oscillator phase as 0.0–1.0 and increment by `freq/sample_rate`
  each sample. Avoids floating-point drift that occurs with `sin(2π × freq × t)` over time.
- **Voice allocation**: Main thread maps MIDI notes → voice indices (`HashMap<u8, usize>`).
  NoteOn finds a free voice (via `voice_active`), NoteOff releases the specific voice.
  When all 8 voices are busy, voice 0 is stolen.
- **ADSR via packed AtomicU64**: Four f32 params are quantized to u16 and packed into a
  single u64 for atomic transfer. Each voice has its own envelope state machine.
- **Gain staging**: Voices are summed and scaled by `0.4 / √8` to prevent clipping.
  Drums are mixed in separately at 0.5 gain on top of the pitched voices.
- **Drum synthesis recipes**: Each drum is a tiny formula run per sample. Kick = pitch-swept
  sine (150 Hz → 40 Hz, exp decay). Snare = white noise + 180 Hz body tone. HiHat = fast-
  decaying noise. Each voice deactivates only once amplitude falls below 0.001 (no hard
  time cutoff — that caused tail clicks at exactly the inter-hit interval).
- **Sample-accurate sequencer scheduling**: The sequencer writes absolute sample numbers
  to per-drum atomic slots; the audio callback compares against `sample_clock` each frame
  and triggers when the time arrives. The wall-clock sleep in the sequencer thread only
  controls *when* events get scheduled, not *when* they play. ~100 ms of lookahead is
  needed to absorb audio buffer batching (smaller lookahead caused occasional late hits
  on Linux desktop audio).
- **Live area redraw**: Each mode has a fixed-height region that redraws in place via
  cursor-up. Mode switches clear the entire screen (`\x1b[2J\x1b[H`).
- **Raw mode newlines**: Headers printed before raw mode use `println!`. Headers printed
  during raw mode (mode switches) use `raw_println` which converts `\n` to `\r\n`.

## Development Notes

- Linux-only (evdev dependency). No plans for cross-platform yet.
- `cargo run` to test — requires a real terminal (not a backgrounded process).
- Number keys 1-4 switch waveforms; Z/X shift octave; Tab toggles ADSR editor; arrows
  navigate/adjust in ADSR mode. All handled in `keyboard.rs`.
- Control keys must be physical scancodes (evdev), not characters — matters for non-US layouts
  (e.g., `[`/`]` are AltGr combos on Nordic keyboards, so `KEY_LEFTBRACE` won't fire).
- Arrow keys allow repeat events (value == 2) for continuous adjustment when held.
- Octave offset is tracked in main.rs and applied to base MIDI notes. Note names are computed
  dynamically via `midi_to_name()` rather than hardcoded.
- Generated WAV files are gitignored.

## What's Next

Check PLAN.md — Phases 1-4 are complete. Phase 5 is in progress: step sequencer with
text-based pattern files. Slices 1-2 done (parser + sequencer engine + drum synthesis +
`--play` CLI). Next slice: melodic note tracks so the format can express bass/lead
lines, then chord shorthand, then a TUI grid view.
