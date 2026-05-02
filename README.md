# Sound Synthesizer

A polyphonic terminal-based sound synthesizer written in Rust. Uses your keyboard as a piano — hold multiple keys to play chords, release to stop.

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

**Piano mode:**
- **A-L / W-P** — Play notes (piano layout)
- **1-4** — Switch waveform: Sine, Square, Sawtooth, Triangle
- **Z / X** — Octave down / up (range: -3 to +3)
- **Tab** — Switch to ADSR envelope editor
- **ESC** — Quit

**ADSR editor mode:**
- **Left / Right** — Select parameter (Attack, Decay, Sustain, Release)
- **Up / Down** — Adjust selected value
- **A-L / W-P** — Play notes to preview the envelope
- **Tab** — Return to piano mode

### Waveforms

Each waveform has a different harmonic profile, giving it a distinct character:

| Key | Waveform | Character |
|-----|----------|-----------|
| 1 | Sine | Pure, clean — no harmonics |
| 2 | Square | Hollow, retro — odd harmonics (1/n) |
| 3 | Sawtooth | Bright, buzzy — all harmonics (1/n) |
| 4 | Triangle | Soft, warm — odd harmonics (1/n²) |

Live braille-character visualizations update in the terminal for both the waveform shape and ADSR envelope curve.

### ADSR Envelope

Notes are shaped by an Attack-Decay-Sustain-Release envelope instead of playing at constant volume. Press **Tab** to open the interactive ADSR editor where you can adjust each parameter with arrow keys and see the envelope shape update in real time. Hold note keys to preview how the envelope sounds.

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
cargo run                                     # interactive piano (default)
cargo run -- --play patterns/cm_groove.pat    # play a pattern in a loop
cargo run -- --render patterns/foo.pat out.wav  # render one pass to a WAV
```

To listen to a rendered WAV, use any system audio player. `mpv` works well:

```bash
mpv /tmp/clocks.wav
```

`aplay` (ALSA, usually preinstalled on Linux) and `paplay` (PulseAudio) also work.

## Project Structure

```
src/
├── main.rs        — Terminal UI, event loop, two-mode interface (piano + ADSR editor)
├── audio.rs       — Polyphonic audio engine (8 voices, waveforms, MIDI-to-frequency)
├── envelope.rs    — ADSR envelope generator (per-sample state machine)
├── keyboard.rs    — Evdev keyboard listener (press/release detection)
├── notes.rs       — Keyboard layout diagram
└── visualizer.rs  — Braille-character renderer (waveforms + envelope curves)
```

## Architecture

- **Audio thread** (cpal callback): Generates samples at 44.1kHz. Runs 8 independent voices, each with its own oscillator and ADSR envelope. Voice commands and active state are communicated via atomic arrays — fully lock-free. Voices are mixed and scaled by √8 to prevent clipping.
- **Keyboard thread** (evdev): Reads raw input events from `/dev/input/` and sends note, waveform, octave, mode, and arrow key events over an MPSC channel.
- **Main thread**: Two-mode UI (piano + ADSR editor). Maps held MIDI notes to voice indices for correct polyphonic release. Manages octave offset and redraws visualizations using ANSI cursor control.

## Roadmap

See [PLAN.md](PLAN.md) for the full learning roadmap, covering waveforms, ADSR envelopes, polyphony, filters, effects, and instrument synthesis.

## License

MIT
