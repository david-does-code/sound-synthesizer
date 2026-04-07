//! Pattern file format and parser.
//!
//! A pattern is a step-sequenced piece of music. There are two kinds of tracks:
//!
//! ### Drum tracks
//!
//! Each cell is a single character: `x` (or `X`) means trigger, `-` (or `.`)
//! means rest. Whitespace inside the cell string is ignored, so the row can be
//! written either as `x---x---` or as `x - - - x - - -`.
//!
//! ### Note tracks
//!
//! Each cell is a whitespace-separated **token**:
//!
//! - A note name like `C4`, `Eb3`, `F#5`, `Bb2` — start playing that pitch.
//! - `-` — rest (release any held note, then silence).
//! - `.` — sustain (keep the previously triggered note ringing).
//!
//! Octaves use scientific pitch notation: middle C is `C4` = MIDI 60.
//! Accidentals: `#` for sharp, `b` for flat.
//!
//! ### Auto-detection
//!
//! The parser decides whether a track is a drum or note track from the row
//! contents: if every non-whitespace character is one of `xX-.`, it's parsed
//! as a drum track; otherwise as a note track.
//!
//! ### Example
//!
//! ```text
//! bpm: 110
//! steps: 16
//!
//! kick:    x---x---x---x---
//! snare:   ----x-------x---
//! hihat:   x-x-x-x-x-x-x-x-
//! bass:    C2 . . . Eb2 . . . G2 . . . Bb2 . . .
//! ```
//!
//! Lines starting with `#` are comments. Blank lines are ignored. Header keys
//! (`bpm`, `steps`) must appear before any track lines.

#![allow(dead_code)]

use std::fmt;
use std::fs;
use std::path::Path;

/// A parsed pattern: tempo, length, and a set of tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub bpm: u32,
    pub steps: usize,
    pub tracks: Vec<Track>,
}

/// A single track within a pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
}

/// Either a percussive trigger track or a melodic note track.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackKind {
    /// Drum-style track. `hits[i] == true` means trigger the drum at step `i`.
    Drum(Vec<bool>),
    /// Melodic note track. One [`Cell`] per step.
    Notes(Vec<Cell>),
}

/// One cell of a note track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Release any held note and stay silent.
    Rest,
    /// Continue holding whatever note is currently playing.
    Sustain,
    /// Trigger the given MIDI note (releasing any previous one on this track).
    Note(u8),
}

/// Errors that can occur while parsing a pattern file.
#[derive(Debug, PartialEq)]
pub enum PatternParseError {
    MissingHeader(&'static str),
    InvalidHeaderValue {
        line: usize,
        key: String,
        value: String,
    },
    MalformedLine {
        line: usize,
        content: String,
    },
    InvalidStepChar {
        line: usize,
        ch: char,
    },
    WrongStepCount {
        line: usize,
        track: String,
        expected: usize,
        got: usize,
    },
    InvalidNoteToken {
        line: usize,
        token: String,
    },
    Io(String),
}

impl fmt::Display for PatternParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader(key) => write!(f, "missing required header `{key}:`"),
            Self::InvalidHeaderValue { line, key, value } => {
                write!(f, "line {line}: invalid value for `{key}`: {value:?}")
            }
            Self::MalformedLine { line, content } => write!(
                f,
                "line {line}: malformed (expected `key: value`): {content:?}"
            ),
            Self::InvalidStepChar { line, ch } => write!(
                f,
                "line {line}: invalid step character {ch:?} (expected `x` or `-`)"
            ),
            Self::WrongStepCount { line, track, expected, got } => write!(
                f,
                "line {line}: track {track:?} has {got} cells, expected {expected}"
            ),
            Self::InvalidNoteToken { line, token } => write!(
                f,
                "line {line}: invalid note token {token:?} (expected a note name like C4, Eb3, F#5, or `-`/`.`)"
            ),
            Self::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl std::error::Error for PatternParseError {}

impl Pattern {
    /// Parse a pattern from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PatternParseError> {
        let text = fs::read_to_string(path).map_err(|e| PatternParseError::Io(e.to_string()))?;
        Self::parse(&text)
    }

    /// Parse a pattern from a string.
    pub fn parse(text: &str) -> Result<Self, PatternParseError> {
        let mut bpm: Option<u32> = None;
        let mut steps: Option<usize> = None;
        let mut tracks: Vec<Track> = Vec::new();

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            let (key, value) = match line.split_once(':') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => {
                    return Err(PatternParseError::MalformedLine {
                        line: line_no,
                        content: line.to_string(),
                    });
                }
            };

            match key {
                "bpm" => {
                    bpm = Some(parse_header_num(line_no, key, value)?);
                }
                "steps" => {
                    steps = Some(parse_header_num(line_no, key, value)?);
                }
                _ => {
                    let expected =
                        steps.ok_or(PatternParseError::MissingHeader("steps"))?;
                    let kind = if looks_like_drum_track(value) {
                        TrackKind::Drum(parse_drum_row(line_no, value, expected)?)
                    } else {
                        TrackKind::Notes(parse_note_row(line_no, value, expected)?)
                    };
                    tracks.push(Track {
                        name: key.to_string(),
                        kind,
                    });
                }
            }
        }

        Ok(Pattern {
            bpm: bpm.ok_or(PatternParseError::MissingHeader("bpm"))?,
            steps: steps.ok_or(PatternParseError::MissingHeader("steps"))?,
            tracks,
        })
    }
}

/// Comments start with `#` only at the beginning of a line (after optional
/// whitespace). `#` elsewhere is a literal character — important for note
/// names like `F#4` and `C#3`.
fn strip_comment(line: &str) -> &str {
    if line.trim_start().starts_with('#') {
        ""
    } else {
        line
    }
}

fn parse_header_num<T: std::str::FromStr>(
    line_no: usize,
    key: &str,
    value: &str,
) -> Result<T, PatternParseError> {
    value
        .parse::<T>()
        .map_err(|_| PatternParseError::InvalidHeaderValue {
            line: line_no,
            key: key.to_string(),
            value: value.to_string(),
        })
}

/// A track value is a "drum track" if every non-whitespace character is one of
/// `xX-.`. Anything else (digits, letters other than x/X) means it's a note row.
fn looks_like_drum_track(value: &str) -> bool {
    value
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|c| matches!(c, 'x' | 'X' | '-' | '.'))
}

fn parse_drum_row(
    line_no: usize,
    value: &str,
    expected: usize,
) -> Result<Vec<bool>, PatternParseError> {
    let mut hits = Vec::with_capacity(expected);
    for ch in value.chars() {
        match ch {
            'x' | 'X' => hits.push(true),
            '-' | '.' => hits.push(false),
            c if c.is_whitespace() => continue,
            other => {
                return Err(PatternParseError::InvalidStepChar {
                    line: line_no,
                    ch: other,
                });
            }
        }
    }
    if hits.len() != expected {
        return Err(PatternParseError::WrongStepCount {
            line: line_no,
            track: String::new(),
            expected,
            got: hits.len(),
        });
    }
    Ok(hits)
}

fn parse_note_row(
    line_no: usize,
    value: &str,
    expected: usize,
) -> Result<Vec<Cell>, PatternParseError> {
    let mut cells: Vec<Cell> = Vec::with_capacity(expected);
    for token in value.split_whitespace() {
        let cell = match token {
            "-" => Cell::Rest,
            "." => Cell::Sustain,
            other => match parse_note_name(other) {
                Some(midi) => Cell::Note(midi),
                None => {
                    return Err(PatternParseError::InvalidNoteToken {
                        line: line_no,
                        token: other.to_string(),
                    });
                }
            },
        };
        cells.push(cell);
    }
    if cells.len() != expected {
        return Err(PatternParseError::WrongStepCount {
            line: line_no,
            track: String::new(),
            expected,
            got: cells.len(),
        });
    }
    Ok(cells)
}

/// Parse a note name like `C4`, `C#3`, `Eb2`, `F#5` into a MIDI note number.
/// Uses scientific pitch notation: middle C = `C4` = MIDI 60.
pub fn parse_note_name(s: &str) -> Option<u8> {
    let mut chars = s.chars();
    let letter = chars.next()?;
    let pitch_class: i32 = match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };

    let mut next = chars.next()?;
    let accidental: i32 = match next {
        '#' => {
            next = chars.next()?;
            1
        }
        'b' => {
            next = chars.next()?;
            -1
        }
        _ => 0,
    };

    // Remaining characters form the octave (allow leading '-' for negative octaves).
    let mut octave_str = String::new();
    octave_str.push(next);
    for c in chars {
        octave_str.push(c);
    }
    let octave: i32 = octave_str.parse().ok()?;

    let midi = (octave + 1) * 12 + pitch_class + accidental;
    if !(0..=127).contains(&midi) {
        return None;
    }
    Some(midi as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drum_hits(track: &Track) -> &Vec<bool> {
        match &track.kind {
            TrackKind::Drum(h) => h,
            _ => panic!("expected drum track, got {:?}", track.kind),
        }
    }

    fn notes(track: &Track) -> &Vec<Cell> {
        match &track.kind {
            TrackKind::Notes(c) => c,
            _ => panic!("expected note track, got {:?}", track.kind),
        }
    }

    #[test]
    fn parses_basic_drum_pattern() {
        let text = "\
bpm: 120
steps: 16

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-
";
        let pat = Pattern::parse(text).expect("should parse");
        assert_eq!(pat.bpm, 120);
        assert_eq!(pat.steps, 16);
        assert_eq!(pat.tracks.len(), 3);

        assert_eq!(pat.tracks[0].name, "kick");
        let kick = drum_hits(&pat.tracks[0]);
        assert_eq!(
            kick,
            &vec![
                true, false, false, false, true, false, false, false, true, false, false, false,
                true, false, false, false,
            ]
        );

        let snare = drum_hits(&pat.tracks[1]);
        assert!(snare[4]);
        assert!(snare[12]);

        let hihat = drum_hits(&pat.tracks[2]);
        assert_eq!(hihat.iter().filter(|h| **h).count(), 8);
    }

    #[test]
    fn parses_note_track_with_basic_pitches() {
        let text = "\
bpm: 120
steps: 8
bass: C2 . . . Eb2 . . .
";
        let pat = Pattern::parse(text).unwrap();
        let cells = notes(&pat.tracks[0]);
        assert_eq!(cells.len(), 8);
        assert_eq!(cells[0], Cell::Note(36)); // C2
        assert_eq!(cells[1], Cell::Sustain);
        assert_eq!(cells[2], Cell::Sustain);
        assert_eq!(cells[3], Cell::Sustain);
        assert_eq!(cells[4], Cell::Note(39)); // Eb2
        assert_eq!(cells[5], Cell::Sustain);
    }

    #[test]
    fn parses_note_track_with_rests_and_accidentals() {
        let text = "\
bpm: 120
steps: 8
lead: C4 D4 Eb4 F#4 - . G4 -
";
        let pat = Pattern::parse(text).unwrap();
        let cells = notes(&pat.tracks[0]);
        assert_eq!(cells[0], Cell::Note(60));
        assert_eq!(cells[1], Cell::Note(62));
        assert_eq!(cells[2], Cell::Note(63));
        assert_eq!(cells[3], Cell::Note(66));
        assert_eq!(cells[4], Cell::Rest);
        assert_eq!(cells[5], Cell::Sustain);
        assert_eq!(cells[6], Cell::Note(67));
        assert_eq!(cells[7], Cell::Rest);
    }

    #[test]
    fn mixes_drum_and_note_tracks() {
        let text = "\
bpm: 120
steps: 4
kick: x-x-
bass: C2 . G2 .
";
        let pat = Pattern::parse(text).unwrap();
        assert!(matches!(pat.tracks[0].kind, TrackKind::Drum(_)));
        assert!(matches!(pat.tracks[1].kind, TrackKind::Notes(_)));
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let text = "\
# a comment at the top
bpm: 90

# another comment
   # indented comments work too
steps: 8
kick: x-x-x-x-
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.bpm, 90);
        assert_eq!(pat.steps, 8);
        let kick = drum_hits(&pat.tracks[0]);
        assert_eq!(kick.len(), 8);
    }

    #[test]
    fn hash_inside_a_track_value_is_a_sharp_not_a_comment() {
        let text = "\
bpm: 120
steps: 4
lead: C#4 D#4 F#4 G#4
";
        let pat = Pattern::parse(text).unwrap();
        let cells = notes(&pat.tracks[0]);
        assert_eq!(cells, &vec![
            Cell::Note(61),
            Cell::Note(63),
            Cell::Note(66),
            Cell::Note(68),
        ]);
    }

    #[test]
    fn allows_whitespace_inside_drum_rows() {
        let text = "\
bpm: 120
steps: 8
hat: x - x - x - x -
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(
            drum_hits(&pat.tracks[0]),
            &vec![true, false, true, false, true, false, true, false]
        );
    }

    #[test]
    fn note_name_parser_handles_all_pitches() {
        // Reference values from MIDI spec.
        assert_eq!(parse_note_name("C4"), Some(60));
        assert_eq!(parse_note_name("C#4"), Some(61));
        assert_eq!(parse_note_name("Db4"), Some(61));
        assert_eq!(parse_note_name("A4"), Some(69));
        assert_eq!(parse_note_name("A0"), Some(21));
        assert_eq!(parse_note_name("C0"), Some(12));
        assert_eq!(parse_note_name("G9"), Some(127));
        // Lowercase letter ok.
        assert_eq!(parse_note_name("c4"), Some(60));
        // Bb (B flat) — disambiguates against B.
        assert_eq!(parse_note_name("Bb4"), Some(70));
        assert_eq!(parse_note_name("B4"), Some(71));
    }

    #[test]
    fn note_name_parser_rejects_garbage() {
        assert_eq!(parse_note_name("H4"), None);
        assert_eq!(parse_note_name("C"), None);
        assert_eq!(parse_note_name("4"), None);
        assert_eq!(parse_note_name("C99"), None); // out of MIDI range
        assert_eq!(parse_note_name(""), None);
    }

    #[test]
    fn errors_on_missing_bpm() {
        let text = "steps: 4\nkick: x---\n";
        assert_eq!(
            Pattern::parse(text).unwrap_err(),
            PatternParseError::MissingHeader("bpm")
        );
    }

    #[test]
    fn errors_on_missing_steps() {
        let text = "bpm: 120\nkick: x---\n";
        assert!(matches!(
            Pattern::parse(text).unwrap_err(),
            PatternParseError::MissingHeader("steps")
        ));
    }

    #[test]
    fn errors_on_wrong_drum_step_count() {
        let text = "bpm: 120\nsteps: 8\nkick: x---x\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::WrongStepCount { expected, got, .. } => {
                assert_eq!(expected, 8);
                assert_eq!(got, 5);
            }
            other => panic!("expected WrongStepCount, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_wrong_note_cell_count() {
        let text = "bpm: 120\nsteps: 8\nlead: C4 D4 E4\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::WrongStepCount { expected, got, .. } => {
                assert_eq!(expected, 8);
                assert_eq!(got, 3);
            }
            other => panic!("expected WrongStepCount, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_invalid_note_token() {
        let text = "bpm: 120\nsteps: 4\nlead: C4 H4 D4 E4\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::InvalidNoteToken { token, .. } => assert_eq!(token, "H4"),
            other => panic!("expected InvalidNoteToken, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_malformed_line() {
        let text = "bpm 120\nsteps: 4\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::MalformedLine { .. } => {}
            other => panic!("expected MalformedLine, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_invalid_bpm_value() {
        let text = "bpm: fast\nsteps: 4\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::InvalidHeaderValue { key, .. } => assert_eq!(key, "bpm"),
            other => panic!("expected InvalidHeaderValue, got {other:?}"),
        }
    }
}
