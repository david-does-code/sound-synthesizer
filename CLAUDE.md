# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Polyphonic audio engine with 8 voices. Each voice has its own oscillator
  and ADSR envelope inside the callback closure. Communication is lock-free via:
  `voice_commands` (`[AtomicU32; 8]` for play/release), `voice_active` (`[AtomicBool; 8]`
  for the callback to report which voices are sounding), waveform (`AtomicU8`), and ADSR
  params (`AtomicU64`, packed). Contains the `Waveform` enum.
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

Check PLAN.md — Phases 1-4 are complete. Next phase is filters & subtractive synthesis.
