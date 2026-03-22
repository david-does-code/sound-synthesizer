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
