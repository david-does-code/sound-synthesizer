# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Audio engine. Runs a cpal output stream with a callback that generates
  samples. Frequency (`AtomicU32`), waveform (`AtomicU8`), gate (`AtomicBool`), and ADSR
  params (`AtomicU64`, packed) are all communicated lock-free. Contains the `Waveform` enum.
- `src/envelope.rs` — ADSR envelope generator. Per-sample state machine (Idle → Attack →
  Decay → Sustain → Release → Idle). Lives inside the audio callback closure.
- `src/keyboard.rs` — Reads raw keyboard events from Linux evdev (`/dev/input/`).
  Sends note, waveform, octave, mode toggle, and arrow key events over an MPSC channel.
- `src/main.rs` — Two-mode UI: piano mode and ADSR editor. Tab toggles between them.
  Mode switch clears screen (`\x1b[2J\x1b[H`) and redraws. Each mode has its own
  "live area" that redraws in place with cursor-up.
- `src/visualizer.rs` — Renders waveforms and ADSR envelopes using Unicode braille characters
  (2×4 dot grid per character). Shared `render_braille` function for both.
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
- **ADSR via packed AtomicU64**: Four f32 params are quantized to u16 and packed into a
  single u64 for atomic transfer to the audio callback. The envelope state machine runs
  per-sample inside the callback; the main thread only sends gate on/off and param updates.
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

Check PLAN.md — Phases 1-3 are complete. Next phase is polyphony (playing chords).
