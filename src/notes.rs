/// Mapping from keyboard keys to MIDI note numbers.
///
/// Layout mirrors a piano keyboard using two rows of your QWERTY keyboard:
///
///  ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
///  │ W │ E │   │ T │ Y │ U │   │ O │ P │   │  ← Black keys (sharps/flats)
///  │C#4│D#4│   │F#4│G#4│A#4│   │C#5│D#5│   │
///  ├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┤
///  │ A │ S │ D │ F │ G │ H │ J │ K │ L │ ; │  ← White keys
///  │ C4│ D4│ E4│ F4│ G4│ A4│ B4│ C5│ D5│ E5│
///  └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
///
/// This matches how a real piano is laid out: the black keys sit between
/// the white keys, offset upward — just like W sits between A and S.
///
/// Music theory: C4 is "middle C" (MIDI 60). The notes are:
///   C  C# D  D# E  F  F# G  G# A  A# B  C
///   60 61 62 63 64 65 66 67 68 69 70 71 72
///
/// The # symbol means "sharp" — one semitone higher. C# is the black key
/// between C and D. There's no black key between E-F or B-C because those
/// pairs are already one semitone apart (this is why the piano keyboard
/// has its irregular pattern of black keys).

use crossterm::event::KeyCode;

/// Returns (MIDI note number, note name) for a given key, or None.
pub fn key_to_note(key: KeyCode) -> Option<(u8, &'static str)> {
    match key {
        // White keys (bottom row) — C major scale
        KeyCode::Char('a') => Some((60, "C4")),
        KeyCode::Char('s') => Some((62, "D4")),
        KeyCode::Char('d') => Some((64, "E4")),
        KeyCode::Char('f') => Some((65, "F4")),
        KeyCode::Char('g') => Some((67, "G4")),
        KeyCode::Char('h') => Some((69, "A4")),
        KeyCode::Char('j') => Some((71, "B4")),
        KeyCode::Char('k') => Some((72, "C5")),
        KeyCode::Char('l') => Some((74, "D5")),
        KeyCode::Char(';') => Some((76, "E5")),

        // Black keys (top row) — sharps
        KeyCode::Char('w') => Some((61, "C#4")),
        KeyCode::Char('e') => Some((63, "D#4")),
        KeyCode::Char('t') => Some((66, "F#4")),
        KeyCode::Char('y') => Some((68, "G#4")),
        KeyCode::Char('u') => Some((70, "A#4")),
        KeyCode::Char('o') => Some((73, "C#5")),
        KeyCode::Char('p') => Some((75, "D#5")),

        _ => None,
    }
}

/// Returns a display string showing the keyboard layout.
pub fn keyboard_help() -> &'static str {
    r#"
  ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
  │ W  │ E  │    │ T  │ Y  │ U  │    │ O  │ P  │    │  Black keys
  │ C#4│ D#4│    │ F#4│ G#4│ A#4│    │ C#5│ D#5│    │  (sharps)
  ├────┼────┼────┼────┼────┼────┼────┼────┼────┼────┤
  │ A  │ S  │ D  │ F  │ G  │ H  │ J  │ K  │ L  │ ;  │  White keys
  │ C4 │ D4 │ E4 │ F4 │ G4 │ A4 │ B4 │ C5 │ D5 │ E5 │
  └────┴────┴────┴────┴────┴────┴────┴────┴────┴────┘"#
}
