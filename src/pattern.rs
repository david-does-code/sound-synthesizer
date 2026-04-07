//! Pattern file format and parser.
//!
//! A pattern is a step-sequenced piece of music. Each track is a row of "steps"
//! where `x` means trigger and `-` means rest. The simplest format looks like:
//!
//! ```text
//! bpm: 120
//! steps: 16
//!
//! kick:    x---x---x---x---
//! snare:   ----x-------x---
//! hihat:   x-x-x-x-x-x-x-x-
//! ```
//!
//! Lines starting with `#` are comments. Blank lines are ignored. Header keys
//! (`bpm`, `steps`) must appear before any track lines. Each track's step string
//! must contain exactly `steps` step characters (whitespace inside the step
//! string is allowed and ignored, so `x - x - x - x -` works too).

#![allow(dead_code)] // `from_file` and the `Io` variant are wired up by the sequencer in slice 2.

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

/// A single track within a pattern. `hits[i] == true` means trigger at step `i`.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub hits: Vec<bool>,
}

/// Errors that can occur while parsing a pattern file.
#[derive(Debug, PartialEq)]
pub enum PatternParseError {
    /// A required header key was missing (`bpm` or `steps`).
    MissingHeader(&'static str),
    /// A header value couldn't be parsed as a number.
    InvalidHeaderValue {
        line: usize,
        key: String,
        value: String,
    },
    /// A line was malformed — not `key: value`.
    MalformedLine { line: usize, content: String },
    /// A track step string contained an unknown character.
    InvalidStepChar { line: usize, ch: char },
    /// A track step string had the wrong number of steps.
    WrongStepCount {
        line: usize,
        track: String,
        expected: usize,
        got: usize,
    },
    /// I/O error reading the file.
    Io(String),
}

impl fmt::Display for PatternParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader(key) => write!(f, "missing required header `{key}:`"),
            Self::InvalidHeaderValue { line, key, value } => {
                write!(f, "line {line}: invalid value for `{key}`: {value:?}")
            }
            Self::MalformedLine { line, content } => {
                write!(f, "line {line}: malformed (expected `key: value`): {content:?}")
            }
            Self::InvalidStepChar { line, ch } => {
                write!(f, "line {line}: invalid step character {ch:?} (expected `x` or `-`)")
            }
            Self::WrongStepCount { line, track, expected, got } => write!(
                f,
                "line {line}: track {track:?} has {got} steps, expected {expected}"
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
                    // Track line — requires `steps` header to have been set.
                    let expected =
                        steps.ok_or(PatternParseError::MissingHeader("steps"))?;
                    let hits = parse_step_string(line_no, value, expected)?;
                    tracks.push(Track {
                        name: key.to_string(),
                        hits,
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

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
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

fn parse_step_string(
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
            track: String::new(), // filled in by caller if desired
            expected,
            got: hits.len(),
        });
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            pat.tracks[0].hits,
            vec![
                true, false, false, false, true, false, false, false,
                true, false, false, false, true, false, false, false,
            ]
        );
        assert_eq!(pat.tracks[1].name, "snare");
        assert_eq!(pat.tracks[1].hits[4], true);
        assert_eq!(pat.tracks[1].hits[12], true);
        assert_eq!(pat.tracks[2].name, "hihat");
        assert_eq!(pat.tracks[2].hits.iter().filter(|h| **h).count(), 8);
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let text = "\
# a comment at the top
bpm: 90

# another comment
steps: 8
kick: x-x-x-x-  # trailing comment
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.bpm, 90);
        assert_eq!(pat.steps, 8);
        assert_eq!(pat.tracks.len(), 1);
        assert_eq!(pat.tracks[0].hits.len(), 8);
    }

    #[test]
    fn allows_whitespace_inside_step_strings() {
        let text = "\
bpm: 120
steps: 8
hat: x - x - x - x -
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.tracks[0].hits, vec![true, false, true, false, true, false, true, false]);
    }

    #[test]
    fn accepts_dot_as_rest() {
        let text = "\
bpm: 120
steps: 4
t: x.x.
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.tracks[0].hits, vec![true, false, true, false]);
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
    fn errors_on_wrong_step_count() {
        let text = "bpm: 120\nsteps: 8\nkick: x---x\n";
        let err = Pattern::parse(text).unwrap_err();
        match err {
            PatternParseError::WrongStepCount { expected, got, .. } => {
                assert_eq!(expected, 8);
                assert_eq!(got, 5);
            }
            other => panic!("expected WrongStepCount, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_invalid_step_char() {
        let text = "bpm: 120\nsteps: 4\nkick: x?x-\n";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::InvalidStepChar { ch, .. } => assert_eq!(ch, '?'),
            other => panic!("expected InvalidStepChar, got {other:?}"),
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
