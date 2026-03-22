mod audio;
mod keyboard;
mod notes;
mod visualizer;

use audio::{AudioEngine, Waveform};
use crossterm::terminal;
use keyboard::{spawn_keyboard_listener, KeyboardEvent};
use notes::keyboard_help;
use std::collections::HashSet;
use std::io::{self, Write};

/// Number of terminal rows used by the waveform display (4 braille rows).
const WAVE_DISPLAY_ROWS: u16 = 4;
/// Total rows in our "live" area: waveform (4) + status line (1) + blank (1)
const LIVE_AREA_ROWS: u16 = WAVE_DISPLAY_ROWS + 2;

/// Octave offset range. The keyboard maps to C4–E5 at offset 0.
/// Offset -3 reaches C1, offset +3 reaches E8 — covering the full piano range.
const MIN_OCTAVE_OFFSET: i8 = -3;
const MAX_OCTAVE_OFFSET: i8 = 3;

fn main() {
    println!("🎹 Sound Synthesizer — Keyboard Piano");
    println!("=======================================");
    println!("{}", keyboard_help());
    println!();
    println!("  Hold keys to play, release to stop. ESC to quit.");
    println!("  [1] Sine  [2] Square  [3] Sawtooth  [4] Triangle");
    println!("  [Z] Octave down  [X] Octave up");
    println!();

    // Enable raw mode BEFORE spawning the keyboard listener.
    terminal::enable_raw_mode().expect("Failed to enable raw mode");

    let engine = AudioEngine::new();
    let events = spawn_keyboard_listener();

    // Octave offset: 0 = default (C4–E5), +1 = C5–E6, -1 = C3–E4, etc.
    let mut octave_offset: i8 = 0;

    // Print the initial live area (no cursor-up on the first draw)
    draw_live_area(engine.waveform(), octave_offset, None);

    // Track base MIDI notes (before offset) that are currently held.
    let mut held_bases: HashSet<u8> = HashSet::new();

    for event in events {
        match event {
            KeyboardEvent::NoteOn { base_midi } => {
                held_bases.insert(base_midi);
                let midi = apply_offset(base_midi, octave_offset);
                let freq = audio::midi_to_freq(midi);
                let name = audio::midi_to_name(midi);
                engine.play_note(midi);
                redraw_live_area(
                    engine.waveform(),
                    octave_offset,
                    Some((&name, freq, midi)),
                );
            }
            KeyboardEvent::NoteOff { base_midi } => {
                held_bases.remove(&base_midi);
                if held_bases.is_empty() {
                    engine.stop();
                    redraw_live_area(engine.waveform(), octave_offset, None);
                } else {
                    // Switch to a remaining held note
                    let &remaining_base = held_bases.iter().next().unwrap();
                    let midi = apply_offset(remaining_base, octave_offset);
                    engine.play_note(midi);
                }
            }
            KeyboardEvent::WaveformChange(waveform) => {
                engine.set_waveform(waveform);
                redraw_live_area(engine.waveform(), octave_offset, None);
            }
            KeyboardEvent::OctaveUp => {
                if octave_offset < MAX_OCTAVE_OFFSET {
                    octave_offset += 1;
                }
                redraw_live_area(engine.waveform(), octave_offset, None);
            }
            KeyboardEvent::OctaveDown => {
                if octave_offset > MIN_OCTAVE_OFFSET {
                    octave_offset -= 1;
                }
                redraw_live_area(engine.waveform(), octave_offset, None);
            }
            KeyboardEvent::Quit => break,
        }
    }

    // Drain buffered stdin before restoring normal mode
    while crossterm::event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
        let _ = crossterm::event::read();
    }

    terminal::disable_raw_mode().expect("Failed to disable raw mode");
    print!("\r\n");
    println!("Bye!");
}

/// Apply the octave offset to a base MIDI note, clamping to valid MIDI range.
fn apply_offset(base_midi: u8, octave_offset: i8) -> u8 {
    let shifted = base_midi as i16 + (octave_offset as i16 * 12);
    shifted.clamp(0, 127) as u8
}

/// Write the live area content (waveform + status line).
/// Used for the initial draw — does NOT move the cursor up first.
fn draw_live_area(waveform: Waveform, octave_offset: i8, note: Option<(&str, f32, u8)>) {
    let mut stdout = io::stdout();

    let lines = visualizer::render_waveform(waveform);
    for line in &lines {
        write!(stdout, "\r  {line}  \r\n").ok();
    }

    write_status_line(&mut stdout, waveform, octave_offset, note);
    write!(stdout, "\r{:60}\r\n", "").ok();

    stdout.flush().ok();
}

/// Move cursor up over the previous live area, then redraw it in place.
fn redraw_live_area(waveform: Waveform, octave_offset: i8, note: Option<(&str, f32, u8)>) {
    let mut stdout = io::stdout();

    // Move cursor up to the start of the live area
    write!(stdout, "\x1b[{LIVE_AREA_ROWS}A").ok();

    let lines = visualizer::render_waveform(waveform);
    for line in &lines {
        write!(stdout, "\r  {line}  \r\n").ok();
    }

    write_status_line(&mut stdout, waveform, octave_offset, note);
    write!(stdout, "\r{:60}\r\n", "").ok();

    stdout.flush().ok();
}

fn write_status_line(
    stdout: &mut io::Stdout,
    waveform: Waveform,
    octave_offset: i8,
    note: Option<(&str, f32, u8)>,
) {
    let octave_indicator = match octave_offset.cmp(&0) {
        std::cmp::Ordering::Greater => format!("+{octave_offset}"),
        std::cmp::Ordering::Less => format!("{octave_offset}"),
        std::cmp::Ordering::Equal => " 0".to_string(),
    };

    match note {
        Some((name, freq, midi)) => {
            write!(
                stdout,
                "\r  ♪ {name:<4} ({freq:>7.1} Hz)  MIDI {midi:<3}  [{wave}]  Oct:{oct}       \r\n",
                wave = waveform.name(),
                oct = octave_indicator,
            ).ok();
        }
        None => {
            write!(
                stdout,
                "\r  [{wave}]  Octave: {oct}                                    \r\n",
                wave = waveform.name(),
                oct = octave_indicator,
            ).ok();
        }
    }
}
