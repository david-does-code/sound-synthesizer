# Sound Synthesizer

A terminal-based sound synthesizer written in Rust. Uses your keyboard as a piano — hold keys to play notes, release to stop.

Inspired by [Sebastian Lague's video on synthesizing musical instruments in code](https://www.youtube.com/watch?v=rRnOtKlg4jA).

## How It Works

The synthesizer generates audio in real time by computing waveform samples in an audio callback. It reads keyboard input directly from Linux's evdev input layer (`/dev/input/`), which gives true key press and release events — something terminal emulators can't provide.

```
  ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
  │ W  │ E  │    │ T  │ Y  │ U  │    │ O  │ P  │    │  Black keys
  │ C#4│ D#4│    │ F#4│ G#4│ A#4│    │ C#5│ D#5│    │  (sharps)
  ├────┼────┼────┼────┼────┼────┼────┼────┼────┼────┤
  │ A  │ S  │ D  │ F  │ G  │ H  │ J  │ K  │ L  │ ;  │  White keys
  │ C4 │ D4 │ E4 │ F4 │ G4 │ A4 │ B4 │ C5 │ D5 │ E5 │
  └────┴────┴────┴────┴────┴────┴────┴────┴────┴────┘
```

### Controls

- **A-L / W-P** — Play notes (piano layout)
- **1-4** — Switch waveform: Sine, Square, Sawtooth, Triangle
- **Z / X** — Octave down / up (range: -3 to +3)
- **ESC** — Quit

### Waveforms

Each waveform has a different harmonic profile, giving it a distinct character:

| Key | Waveform | Character |
|-----|----------|-----------|
| 1 | Sine | Pure, clean — no harmonics |
| 2 | Square | Hollow, retro — odd harmonics (1/n) |
| 3 | Sawtooth | Bright, buzzy — all harmonics (1/n) |
| 4 | Triangle | Soft, warm — odd harmonics (1/n²) |

A live braille-character waveform visualization updates in the terminal as you switch between waveforms.

## Requirements

- **Linux** (uses evdev for keyboard input)
- **Rust** (stable, 1.85+)
- Your user must be in the `input` group to read `/dev/input/` devices:
  ```bash
  sudo usermod -aG input $USER
  # Log out and back in for the group change to take effect
  ```

## Building & Running

```bash
cargo build
cargo run
```

## Project Structure

```
src/
├── main.rs        — Terminal UI and event loop
├── audio.rs       — Real-time audio engine (oscillator, waveforms, MIDI-to-frequency)
├── keyboard.rs    — Evdev keyboard listener (press/release detection)
├── notes.rs       — Keyboard layout diagram
└── visualizer.rs  — Braille-character waveform renderer
```

## Architecture

- **Audio thread** (cpal callback): Generates samples at 44.1kHz. Reads the target frequency and waveform type from atomic variables — lock-free, no mutex, no risk of audio glitches from blocking.
- **Keyboard thread** (evdev): Reads raw input events from `/dev/input/` and sends `NoteOn`/`NoteOff`/`WaveformChange`/`OctaveUp`/`OctaveDown` messages over an MPSC channel.
- **Main thread**: Connects keyboard events to the audio engine. Tracks held keys for smooth transitions, manages octave offset, redraws the waveform visualization in place using ANSI cursor control.

## Roadmap

See [PLAN.md](PLAN.md) for the full learning roadmap, covering waveforms, ADSR envelopes, polyphony, filters, effects, and instrument synthesis.

## License

MIT
