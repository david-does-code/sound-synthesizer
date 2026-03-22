mod audio;
mod keyboard;
mod notes;

use audio::AudioEngine;
use crossterm::terminal;
use keyboard::{spawn_keyboard_listener, KeyboardEvent};
use notes::keyboard_help;
use std::collections::HashSet;

fn main() {
    println!("🎹 Sound Synthesizer — Keyboard Piano");
    println!("=======================================");
    println!("{}", keyboard_help());
    println!();
    println!("  Hold keys to play, release to stop. ESC to quit.");
    println!("  (Reading keyboard directly via /dev/input)");
    println!();
    // Extra blank line so the status line doesn't overwrite the help text
    println!();

    // Enable raw mode BEFORE spawning the keyboard listener.
    // This prevents keypresses from echoing in the terminal
    // (evdev reads from /dev/input but the terminal still receives them too).
    terminal::enable_raw_mode().expect("Failed to enable raw mode");

    let engine = AudioEngine::new();
    let events = spawn_keyboard_listener();

    // Track which MIDI notes are currently held down.
    // This prevents the "blip" when transitioning between keys:
    // if you press B before releasing A, we get NoteOn(B) then NoteOff(A).
    // Without tracking, NoteOff(A) would silence everything.
    // With tracking, we only stop when the set is empty.
    let mut held_notes: HashSet<u8> = HashSet::new();

    for event in events {
        match event {
            KeyboardEvent::NoteOn { midi, name } => {
                held_notes.insert(midi);
                let freq = audio::midi_to_freq(midi);
                print!("\r  ♪ {name:<4} ({freq:>7.1} Hz)  MIDI {midi}    ");
                engine.play_note(midi);
            }
            KeyboardEvent::NoteOff { midi } => {
                held_notes.remove(&midi);
                if held_notes.is_empty() {
                    engine.stop();
                    print!("\r                                        ");
                } else {
                    // Another key is still held — switch to that note
                    let &remaining = held_notes.iter().next().unwrap();
                    engine.play_note(remaining);
                }
            }
            KeyboardEvent::Quit => break,
        }
    }

    // Drain any buffered stdin bytes before restoring normal mode,
    // otherwise the terminal will dump all the keys we pressed.
    while crossterm::event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
        let _ = crossterm::event::read();
    }

    terminal::disable_raw_mode().expect("Failed to disable raw mode");
    println!("\r\nBye!");
}
