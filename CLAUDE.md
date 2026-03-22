# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Audio engine. Runs a cpal output stream with a callback that generates
  samples. Frequency is communicated via `AtomicU32` (lock-free, real-time safe).
- `src/keyboard.rs` — Reads raw keyboard events from Linux evdev (`/dev/input/`).
  Runs in a background thread, sends `NoteOn`/`NoteOff` over an MPSC channel.
- `src/main.rs` — Event loop connecting keyboard input to audio output. Tracks held keys
  in a `HashSet` for smooth note transitions.
- `src/notes.rs` — QWERTY-to-MIDI mapping and keyboard layout diagram.

## Key Design Decisions

- **evdev over terminal input**: Terminals don't send key release events. We read
  `/dev/input/` directly for true press/release, which requires the `input` group.
- **Atomic frequency (not mutex)**: The audio callback is real-time — it must never block.
  We use `AtomicU32` with f32 bit patterns instead of a `Mutex<f32>`.
- **Phase accumulation**: Track oscillator phase as 0.0–1.0 and increment by `freq/sample_rate`
  each sample. Avoids floating-point drift that occurs with `sin(2π × freq × t)` over time.
- **Monophonic with key tracking**: Currently plays one note at a time but tracks all held
  keys so releasing one key while holding another doesn't cause silence.

## Development Notes

- Linux-only (evdev dependency). No plans for cross-platform yet.
- `cargo run` to test — requires a real terminal (not a backgrounded process).
- The `notes.rs` file also has a crossterm-based key mapping that is currently unused
  (leftover from before the evdev switch). Can be removed or kept for potential
  cross-platform fallback.
- Generated WAV files are gitignored.

## What's Next

Check PLAN.md — the next phase is adding waveform types (square, saw, triangle)
and octave shifting.
