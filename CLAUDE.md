# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Audio engine. Runs a cpal output stream with a callback that generates
  samples. Frequency (`AtomicU32`) and waveform (`AtomicU8`) are communicated lock-free.
  Contains the `Waveform` enum with sample generation for sine, square, saw, and triangle.
- `src/keyboard.rs` — Reads raw keyboard events from Linux evdev (`/dev/input/`).
  Runs in a background thread, sends `NoteOn`/`NoteOff`/`WaveformChange` over an MPSC channel.
- `src/main.rs` — Event loop connecting keyboard input to audio output. Tracks held keys
  in a `HashSet` for smooth note transitions. Manages a "live area" in the terminal
  (waveform + status line) using ANSI cursor-up to redraw in place.
- `src/visualizer.rs` — Renders waveforms using Unicode braille characters (2×4 dot grid
  per character) for sub-character resolution curves.
- `src/notes.rs` — Keyboard layout diagram.

## Key Design Decisions

- **evdev over terminal input**: Terminals don't send key release events. We read
  `/dev/input/` directly for true press/release, which requires the `input` group.
- **Atomic frequency (not mutex)**: The audio callback is real-time — it must never block.
  We use `AtomicU32` with f32 bit patterns instead of a `Mutex<f32>`.
- **Phase accumulation**: Track oscillator phase as 0.0–1.0 and increment by `freq/sample_rate`
  each sample. Avoids floating-point drift that occurs with `sin(2π × freq × t)` over time.
- **Monophonic with key tracking**: Currently plays one note at a time but tracks all held
  keys so releasing one key while holding another doesn't cause silence.
- **Live area redraw**: The waveform display and status line occupy a fixed region at the
  bottom. First draw prints normally; subsequent redraws use `\x1b[{n}A` (cursor up) to
  overwrite in place without scrolling.

## Development Notes

- Linux-only (evdev dependency). No plans for cross-platform yet.
- `cargo run` to test — requires a real terminal (not a backgrounded process).
- Number keys 1-4 switch waveforms; Z/X shift octave down/up. All handled in `keyboard.rs`.
- Control keys must be physical scancodes (evdev), not characters — matters for non-US layouts
  (e.g., `[`/`]` are AltGr combos on Nordic keyboards, so `KEY_LEFTBRACE` won't fire).
- Octave offset is tracked in main.rs and applied to base MIDI notes. Note names are computed
  dynamically via `midi_to_name()` rather than hardcoded.
- Generated WAV files are gitignored.

## What's Next

Check PLAN.md — Phase 2 is complete. Next phase is ADSR envelopes.
