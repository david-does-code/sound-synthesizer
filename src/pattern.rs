//! Pattern file format and parser.
//!
//! A pattern is a step-sequenced piece of music. Three kinds of tracks:
//!
//! ### Drum tracks
//!
//! Each cell is a single character: `x` (or `X`) means trigger, `-` (or `.`)
//! means rest. Whitespace inside the row is ignored, so it can be written as
//! either `x---x---` or `x - - - x - - -`.
//!
//! ### Note tracks (monophonic)
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
//! ### Chord tracks (polyphonic)
//!
//! Each cell is either `-`, `.`, or a **chord shorthand** like `Cm`, `G7`,
//! `Fmaj7`, `Dsus4`. The parser expands the shorthand to the component notes,
//! which are played simultaneously across multiple voices. Supported chord
//! types: major (`C`), minor (`Cm` / `Cmin`), dominant 7 (`C7`), major 7
//! (`Cmaj7`), minor 7 (`Cm7`), diminished (`Cdim`), diminished 7 (`Cdim7`),
//! augmented (`Caug`), suspended 2 (`Csus2`), suspended 4 (`Csus4`).
//!
//! Default octave for chord roots is 3, configurable via `name.octave: N`.
//!
//! ### Per-track properties
//!
//! Lines like `name.wave: square` set a property on a track. Currently:
//!
//! - `name.wave: <sine|square|saw|triangle>` — waveform for the track's voices
//! - `name.octave: <int>` — root octave for chord tracks (default 3)
//!
//! Property lines can appear before or after the track row.
//!
//! ### Auto-detection
//!
//! The parser decides each track's kind from the row contents:
//! 1. Only `xX-.` characters → drum track
//! 2. Any token that's a chord shorthand (e.g. `Cm`, `Gmaj7`) → chord track
//! 3. Otherwise → note track
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
//!
//! bass.wave: sine
//! bass:    C2 . . . Eb2 . . . G2 . . . Bb2 . . .
//!
//! lead.wave: square
//! lead:    Eb4 F4 G4 Bb4 C5 . Bb4 G4 F4 . Eb4 . D4 . C4 .
//!
//! pad.wave: triangle
//! pad.octave: 4
//! pad:     Cm . . . Fm . . . Gm . . . Cm . . .
//! ```
//!
//! Lines starting with `#` (at the beginning of the line) are comments. Blank
//! lines are ignored. Header keys (`bpm`, `steps`) must appear before any
//! track lines.

#![allow(dead_code)]

use crate::audio::Waveform;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

/// Default octave for chord roots when no `track.octave: N` property is set.
const DEFAULT_CHORD_OCTAVE: i32 = 3;

/// A parsed pattern file: tempo, one or more named sections, and a song chain
/// that says which order to play them in. A file with no `[section]` headers
/// is parsed into a single section called `"main"` with an implicit song
/// `[main x1]`, so existing single-pattern files keep working.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub bpm: u32,
    /// Global swing amount (0.0–1.0). 0.0 = straight, 0.5 = full triplet feel.
    pub swing: f32,
    /// Reverb wet/dry mix (0.0–1.0). 0.0 = dry (default), ~0.2–0.3 = roomy.
    /// Applied to the master mix as a single send-style reverb.
    pub reverb: f32,
    /// How many steps fit in one beat (a quarter note). Default 4 = 16th-note
    /// grid (the standard step-sequencer default). Set to 3 for eighth-note
    /// triplets, 6 for 16th-note triplets, 8 for 32nd notes, etc.
    pub steps_per_beat: u32,
    pub sections: Vec<Section>,
    pub song: Vec<SongEntry>,
}

/// A named section of music — equivalent to "one pattern" in the old format.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub name: String,
    pub steps: usize,
    /// Per-section BPM override. `None` = inherit from global `Pattern.bpm`.
    pub bpm: Option<u32>,
    /// Per-section swing override. `None` = inherit from global `Pattern.swing`.
    pub swing: Option<f32>,
    /// Per-section steps-per-beat override. Lets one section be triplets and
    /// another be 16ths within the same song.
    pub steps_per_beat: Option<u32>,
    pub tracks: Vec<Track>,
}

/// One entry in a song chain: which section to play and how many times in a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongEntry {
    pub section: String,
    pub repeat: u32,
}

impl Pattern {
    /// Look up a section by name.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }
}

/// A single track within a pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    /// Optional per-track waveform override (`name.wave: square`).
    pub wave: Option<Waveform>,
    /// Per-track ADSR overrides (`name.attack: 0.2`, etc.). `None` = use default.
    pub attack: Option<f32>,
    pub decay: Option<f32>,
    pub sustain: Option<f32>,
    pub release: Option<f32>,
    /// Per-track gain/volume (`name.gain: 0.8`). `None` = use default (1.0).
    pub gain: Option<f32>,
    /// Per-track gate length as fraction of step (`name.gate: 0.5`). `None` = legato.
    pub gate: Option<f32>,
    /// Per-track "hammer click" — pitch transient on note-on, in semitones.
    /// Each note starts pitched up by this many semitones and decays to its
    /// target pitch over a few milliseconds, simulating a piano hammer's
    /// percussive attack. 0 / None = no transient. (`name.click: 12`)
    pub click: Option<f32>,
    /// Per-track sub-octave layer amplitude. When set, the voice mixes in a
    /// sine wave one octave below each note at this amplitude (0.0–1.0).
    /// Adds body / warmth to thin synth leads. (`name.sub: 0.3`)
    pub sub: Option<f32>,
}

/// What this track plays.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackKind {
    /// Drum-style track. Each step is a velocity (0.0 = rest, >0 = hit).
    /// `X` = accent (1.0), `x` = normal (0.7), `o` = ghost (0.35).
    Drum(Vec<f32>),
    /// Monophonic note track. One [`Cell`] per step.
    Notes(Vec<Cell>),
    /// Polyphonic chord track. One [`ChordCell`] per step. Each chord plays
    /// all of its notes simultaneously across multiple voices.
    Chord(Vec<ChordCell>),
}

/// Default velocity for normal hits.
pub const VEL_NORMAL: f32 = 0.7;
/// Velocity for accented hits.
pub const VEL_ACCENT: f32 = 1.0;
/// Velocity for ghost notes.
pub const VEL_GHOST: f32 = 0.35;

/// One cell of a monophonic note track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    /// Release any held note and stay silent.
    Rest,
    /// Continue holding whatever note is currently playing.
    Sustain,
    /// Trigger the given MIDI note with velocity (0.0–1.0).
    Note(u8, f32),
}

/// One cell of a polyphonic chord track.
#[derive(Debug, Clone, PartialEq)]
pub enum ChordCell {
    /// Release any held notes and stay silent.
    Rest,
    /// Continue holding whatever chord is currently playing.
    Sustain,
    /// Trigger the given set of (MIDI note, velocity) pairs simultaneously.
    Chord(Vec<(u8, f32)>),
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
    UnknownProperty {
        line: usize,
        track: String,
        prop: String,
    },
    InvalidPropertyValue {
        line: usize,
        track: String,
        prop: String,
        value: String,
    },
    /// `[section]` header was malformed — e.g. missing closing bracket.
    MalformedSectionHeader { line: usize, content: String },
    /// A `song:` entry referenced a section that doesn't exist.
    UnknownSection { line: usize, name: String },
    /// A `song:` repeat count couldn't be parsed (e.g. `verse xfoo`).
    InvalidSongRepeat { line: usize, token: String },
    /// A track row appeared before any section header in a multi-section file
    /// (only allowed in single-section files where the section is implicit).
    OrphanTrackRow { line: usize, track: String },
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
                "line {line}: invalid note token {token:?} (expected a note name like C4, Eb3, F#5, a chord like Cm or G7, or `-`/`.`)"
            ),
            Self::UnknownProperty { line, track, prop } => write!(
                f,
                "line {line}: unknown property {prop:?} on track {track:?} (supported: wave, octave, attack, decay, sustain, release, gain, gate)"
            ),
            Self::InvalidPropertyValue { line, track, prop, value } => write!(
                f,
                "line {line}: invalid value {value:?} for {track:?}.{prop} property"
            ),
            Self::MalformedSectionHeader { line, content } => write!(
                f,
                "line {line}: malformed section header {content:?} (expected `[name]`)"
            ),
            Self::UnknownSection { line, name } => write!(
                f,
                "line {line}: song references unknown section {name:?}"
            ),
            Self::InvalidSongRepeat { line, token } => write!(
                f,
                "line {line}: invalid song repeat {token:?} (expected like `x4`)"
            ),
            Self::OrphanTrackRow { line, track } => write!(
                f,
                "line {line}: track {track:?} appears outside any section header"
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
    ///
    /// Two passes: the first collects per-track properties (so they can be
    /// declared either before or after the track row), the second builds the
    /// header values, sections, and song chain.
    pub fn parse(text: &str) -> Result<Self, PatternParseError> {
        // ── pass 1: collect per-track properties ───────────────────────────
        // Properties are global across all sections, so they can appear
        // anywhere in the file.
        let mut props: HashMap<String, TrackProps> = HashMap::new();
        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() || is_section_header(line) {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue; // pass 2 will report the malformed line
            };
            let key = key.trim();
            let value = value.trim();
            if let Some((track_name, prop_name)) = key.split_once('.') {
                let entry = props.entry(track_name.trim().to_string()).or_default();
                apply_property(line_no, track_name.trim(), prop_name.trim(), value, entry)?;
            }
        }

        // ── pass 2: parse headers, sections, and the song chain ────────────
        let mut bpm: Option<u32> = None;
        let mut global_steps: Option<usize> = None;
        let mut global_swing: f32 = 0.0;
        let mut global_reverb: f32 = 0.0;
        let mut global_steps_per_beat: u32 = 4;
        let mut sections: Vec<Section> = Vec::new();
        let mut song: Vec<SongEntry> = Vec::new();

        // The "current section" tracks where new track rows go. It starts as
        // an implicit anonymous section so that single-pattern files (no
        // `[section]` headers) keep working unchanged.
        let mut current: Option<Section> = None;
        let mut implicit_section = true;

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            // ── section header: `[name]` ───────────────────────────────────
            if let Some(name) = parse_section_header(line) {
                // Flush the current section if there is one.
                if let Some(sec) = current.take() {
                    sections.push(sec);
                }
                current = Some(Section {
                    name: name.to_string(),
                    steps: global_steps.unwrap_or(0),
                    bpm: None,
                    swing: None,
                    steps_per_beat: None,
                    tracks: Vec::new(),
                });
                implicit_section = false;
                continue;
            }

            // Reject obvious half-headers that didn't match parse_section_header.
            if line.starts_with('[') {
                return Err(PatternParseError::MalformedSectionHeader {
                    line: line_no,
                    content: line.to_string(),
                });
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

            // Skip property lines (already processed in pass 1).
            if key.contains('.') {
                continue;
            }

            match key {
                "bpm" => {
                    let val: u32 = parse_header_num(line_no, key, value)?;
                    if bpm.is_none() || implicit_section {
                        bpm = Some(val);
                    }
                    // If we're inside a named section, also set per-section BPM.
                    if !implicit_section {
                        if let Some(ref mut sec) = current {
                            sec.bpm = Some(val);
                        }
                    }
                }
                "steps" => {
                    let n: usize = parse_header_num(line_no, key, value)?;
                    if let Some(ref mut sec) = current {
                        // Per-section steps. If we're still in the implicit
                        // section, also remember it as the global default.
                        sec.steps = n;
                    }
                    if implicit_section || global_steps.is_none() {
                        global_steps = Some(n);
                    }
                }
                "swing" => {
                    let s: f32 = value.parse().map_err(|_| {
                        PatternParseError::InvalidHeaderValue {
                            line: line_no,
                            key: key.to_string(),
                            value: value.to_string(),
                        }
                    })?;
                    let clamped = s.clamp(0.0, 1.0);
                    if implicit_section || !current.is_some() {
                        global_swing = clamped;
                    }
                    if let Some(ref mut sec) = current {
                        if !implicit_section {
                            sec.swing = Some(clamped);
                        }
                    }
                }
                "song" => {
                    song = parse_song_chain(line_no, value)?;
                }
                "reverb" => {
                    let r: f32 = value.parse().map_err(|_| {
                        PatternParseError::InvalidHeaderValue {
                            line: line_no,
                            key: key.to_string(),
                            value: value.to_string(),
                        }
                    })?;
                    global_reverb = r.clamp(0.0, 1.0);
                }
                "steps_per_beat" => {
                    let n: u32 = parse_header_num(line_no, key, value)?;
                    if n == 0 {
                        return Err(PatternParseError::InvalidHeaderValue {
                            line: line_no,
                            key: key.to_string(),
                            value: value.to_string(),
                        });
                    }
                    if implicit_section || current.is_none() {
                        global_steps_per_beat = n;
                    }
                    if let Some(ref mut sec) = current {
                        if !implicit_section {
                            sec.steps_per_beat = Some(n);
                        }
                    }
                }
                _ => {
                    // It's a track row. Make sure we have a section to put it in.
                    if current.is_none() && implicit_section {
                        current = Some(Section {
                            name: "main".to_string(),
                            steps: global_steps.unwrap_or(0),
                            bpm: None,
                            swing: None,
                            steps_per_beat: None,
                            tracks: Vec::new(),
                        });
                    }
                    let sec = current.as_mut().ok_or_else(|| {
                        PatternParseError::OrphanTrackRow {
                            line: line_no,
                            track: key.to_string(),
                        }
                    })?;
                    if sec.steps == 0 {
                        return Err(PatternParseError::MissingHeader("steps"));
                    }
                    let expected = sec.steps;
                    let track_props = props.get(key).cloned().unwrap_or_default();
                    let kind = if looks_like_drum_track(value) {
                        TrackKind::Drum(parse_drum_row(line_no, value, expected)?)
                    } else if row_is_chord(value) {
                        let octave = track_props.octave.unwrap_or(DEFAULT_CHORD_OCTAVE);
                        TrackKind::Chord(parse_chord_row(line_no, value, expected, octave)?)
                    } else {
                        TrackKind::Notes(parse_note_row(line_no, value, expected)?)
                    };
                    sec.tracks.push(Track {
                        name: key.to_string(),
                        kind,
                        wave: track_props.wave,
                        attack: track_props.attack,
                        decay: track_props.decay,
                        sustain: track_props.sustain,
                        release: track_props.release,
                        gain: track_props.gain,
                        gate: track_props.gate,
                        click: track_props.click,
                        sub: track_props.sub,
                    });
                }
            }
        }

        // Flush the final section.
        if let Some(sec) = current.take() {
            sections.push(sec);
        }

        // Default song chain: every section in declaration order, played once each.
        // For single-section files this just plays the only section forever.
        if song.is_empty() {
            song = sections
                .iter()
                .map(|s| SongEntry {
                    section: s.name.clone(),
                    repeat: 1,
                })
                .collect();
        }

        // Validate that every song entry references a real section.
        for entry in &song {
            if !sections.iter().any(|s| s.name == entry.section) {
                return Err(PatternParseError::UnknownSection {
                    line: 0,
                    name: entry.section.clone(),
                });
            }
        }

        Ok(Pattern {
            bpm: bpm.ok_or(PatternParseError::MissingHeader("bpm"))?,
            swing: global_swing,
            reverb: global_reverb,
            steps_per_beat: global_steps_per_beat,
            sections,
            song,
        })
    }
}

fn is_section_header(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']') && line.len() >= 3
}

/// Parse a `[name]` line into the section name. Returns `None` if not a section header.
fn parse_section_header(line: &str) -> Option<&str> {
    if is_section_header(line) {
        Some(line[1..line.len() - 1].trim())
    } else {
        None
    }
}

/// Parse a song chain line like `intro verse x2 chorus outro` into a list of
/// (section, repeat) entries.
fn parse_song_chain(line_no: usize, value: &str) -> Result<Vec<SongEntry>, PatternParseError> {
    let mut entries: Vec<SongEntry> = Vec::new();
    for token in value.split_whitespace() {
        if let Some(rest) = token.strip_prefix('x') {
            // Repeat marker — applies to the previous entry.
            let n: u32 = rest
                .parse()
                .map_err(|_| PatternParseError::InvalidSongRepeat {
                    line: line_no,
                    token: token.to_string(),
                })?;
            if let Some(last) = entries.last_mut() {
                last.repeat = n;
            } else {
                return Err(PatternParseError::InvalidSongRepeat {
                    line: line_no,
                    token: token.to_string(),
                });
            }
        } else {
            entries.push(SongEntry {
                section: token.to_string(),
                repeat: 1,
            });
        }
    }
    Ok(entries)
}

/// Per-track properties accumulated in pass 1 of parsing.
#[derive(Default, Debug, Clone)]
struct TrackProps {
    wave: Option<Waveform>,
    octave: Option<i32>,
    attack: Option<f32>,
    decay: Option<f32>,
    sustain: Option<f32>,
    release: Option<f32>,
    gain: Option<f32>,
    gate: Option<f32>,
    click: Option<f32>,
    sub: Option<f32>,
}

fn apply_property(
    line_no: usize,
    track: &str,
    prop: &str,
    value: &str,
    out: &mut TrackProps,
) -> Result<(), PatternParseError> {
    match prop {
        "wave" => {
            let wave = parse_waveform(value).ok_or_else(|| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.wave = Some(wave);
        }
        "octave" => {
            let oct: i32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.octave = Some(oct);
        }
        "attack" | "decay" | "release" => {
            let secs = parse_time_value(value).ok_or_else(|| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            match prop {
                "attack" => out.attack = Some(secs),
                "decay" => out.decay = Some(secs),
                _ => out.release = Some(secs),
            }
        }
        "sustain" => {
            let level: f32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.sustain = Some(level.clamp(0.0, 1.0));
        }
        "gain" => {
            let g: f32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.gain = Some(g.max(0.0));
        }
        "gate" => {
            let g: f32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.gate = Some(g.clamp(0.0, 1.0));
        }
        "click" => {
            let c: f32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.click = Some(c);
        }
        "sub" => {
            let s: f32 = value.parse().map_err(|_| {
                PatternParseError::InvalidPropertyValue {
                    line: line_no,
                    track: track.to_string(),
                    prop: prop.to_string(),
                    value: value.to_string(),
                }
            })?;
            out.sub = Some(s.clamp(0.0, 1.0));
        }
        _ => {
            return Err(PatternParseError::UnknownProperty {
                line: line_no,
                track: track.to_string(),
                prop: prop.to_string(),
            });
        }
    }
    Ok(())
}

/// Parse a time value: bare number is seconds (`0.2`), suffix `ms` is
/// milliseconds (`200ms`). Returns seconds.
fn parse_time_value(s: &str) -> Option<f32> {
    if let Some(ms_str) = s.strip_suffix("ms") {
        let ms: f32 = ms_str.trim().parse().ok()?;
        Some(ms / 1000.0)
    } else {
        s.parse::<f32>().ok()
    }
}

/// Parse a waveform name (case-insensitive). Accepts `sine`, `square`,
/// `saw`/`sawtooth`, `triangle`/`tri`.
fn parse_waveform(s: &str) -> Option<Waveform> {
    match s.to_ascii_lowercase().as_str() {
        "sine" | "sin" => Some(Waveform::Sine),
        "square" | "sq" => Some(Waveform::Square),
        "saw" | "sawtooth" => Some(Waveform::Sawtooth),
        "triangle" | "tri" => Some(Waveform::Triangle),
        _ => None,
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
/// `xXo-.`. Anything else (digits, letters other than x/X/o) means it's a note row.
fn looks_like_drum_track(value: &str) -> bool {
    value
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|c| matches!(c, 'x' | 'X' | 'o' | '-' | '.'))
}

fn parse_drum_row(
    line_no: usize,
    value: &str,
    expected: usize,
) -> Result<Vec<f32>, PatternParseError> {
    let mut hits = Vec::with_capacity(expected);
    for ch in value.chars() {
        match ch {
            'X' => hits.push(VEL_ACCENT),
            'x' => hits.push(VEL_NORMAL),
            'o' => hits.push(VEL_GHOST),
            '-' | '.' => hits.push(0.0),
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
            other => {
                // Strip velocity suffix: `!` = accent, `?` = ghost.
                let (note_str, vel) = if let Some(s) = other.strip_suffix('!') {
                    (s, VEL_ACCENT)
                } else if let Some(s) = other.strip_suffix('?') {
                    (s, VEL_GHOST)
                } else {
                    (other, VEL_NORMAL)
                };
                match parse_note_name(note_str) {
                    Some(midi) => Cell::Note(midi, vel),
                    None => {
                        return Err(PatternParseError::InvalidNoteToken {
                            line: line_no,
                            token: other.to_string(),
                        });
                    }
                }
            }
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

/// True if `value` looks like a chord row — i.e., contains at least one
/// token that is unambiguously a chord shorthand (a token that can't be parsed
/// as a single note). Tokens like `C7` are ambiguous (could be either a note
/// in octave 7 or C dominant 7) and don't on their own trigger chord mode;
/// the row needs at least one unambiguous chord token like `Cm`, `Gmaj7`,
/// `Fdim`, `Asus4`, etc.
fn row_is_chord(value: &str) -> bool {
    for token in value.split_whitespace() {
        if matches!(token, "-" | ".") {
            continue;
        }
        // If it parses as a plain note, it doesn't qualify on its own.
        if parse_note_name(token).is_some() {
            continue;
        }
        // If it parses as a chord, this row is a chord row.
        if parse_chord_shorthand(token, DEFAULT_CHORD_OCTAVE).is_some() {
            return true;
        }
    }
    false
}

fn parse_chord_row(
    line_no: usize,
    value: &str,
    expected: usize,
    octave: i32,
) -> Result<Vec<ChordCell>, PatternParseError> {
    let mut cells = Vec::with_capacity(expected);
    for token in value.split_whitespace() {
        let cell = match token {
            "-" => ChordCell::Rest,
            "." => ChordCell::Sustain,
            other => {
                let (chord_str, vel) = if let Some(s) = other.strip_suffix('!') {
                    (s, VEL_ACCENT)
                } else if let Some(s) = other.strip_suffix('?') {
                    (s, VEL_GHOST)
                } else {
                    (other, VEL_NORMAL)
                };
                match parse_chord_shorthand(chord_str, octave) {
                    Some(notes) => {
                        ChordCell::Chord(notes.into_iter().map(|m| (m, vel)).collect())
                    }
                    None => {
                        return Err(PatternParseError::InvalidNoteToken {
                            line: line_no,
                            token: other.to_string(),
                        });
                    }
                }
            }
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

/// Parse a chord shorthand like `Cm`, `G7`, `Fmaj7`, `Dsus4` into a vector of
/// MIDI notes. The root pitch class is taken from the first 1-2 characters
/// (note letter + optional accidental); the rest is the chord type.
///
/// Supported chord types and their semitone intervals (from the root):
///
/// | Suffix              | Type             | Intervals       |
/// |---------------------|------------------|------------------|
/// | (empty)             | major triad      | 0, 4, 7          |
/// | `m` / `min`         | minor triad      | 0, 3, 7          |
/// | `7`                 | dominant 7       | 0, 4, 7, 10      |
/// | `maj7` / `M7`       | major 7          | 0, 4, 7, 11      |
/// | `m7` / `min7`       | minor 7          | 0, 3, 7, 10      |
/// | `dim`               | diminished       | 0, 3, 6          |
/// | `dim7`              | diminished 7     | 0, 3, 6, 9       |
/// | `aug` / `+`         | augmented        | 0, 4, 8          |
/// | `sus2`              | suspended 2nd    | 0, 2, 7          |
/// | `sus4`              | suspended 4th    | 0, 5, 7          |
pub fn parse_chord_shorthand(s: &str, default_octave: i32) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    // Note letter
    let pitch_class: i32 = match bytes[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };

    // Optional accidental
    let (accidental, suffix_start): (i32, usize) = match bytes.get(1) {
        Some(b'#') => (1, 2),
        Some(b'b') => (-1, 2),
        _ => (0, 1),
    };

    let suffix = &s[suffix_start..];
    let intervals: &[i32] = match suffix {
        "" => &[0, 4, 7],
        "m" | "min" => &[0, 3, 7],
        "7" => &[0, 4, 7, 10],
        "maj7" | "M7" => &[0, 4, 7, 11],
        "m7" | "min7" => &[0, 3, 7, 10],
        "dim" => &[0, 3, 6],
        "dim7" => &[0, 3, 6, 9],
        "aug" | "+" => &[0, 4, 8],
        "sus2" => &[0, 2, 7],
        "sus4" => &[0, 5, 7],
        _ => return None,
    };

    let root_midi = (default_octave + 1) * 12 + pitch_class + accidental;
    let mut notes = Vec::with_capacity(intervals.len());
    for interval in intervals {
        let midi = root_midi + interval;
        if !(0..=127).contains(&midi) {
            return None;
        }
        notes.push(midi as u8);
    }
    Some(notes)
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

    fn drum_hits(track: &Track) -> Vec<bool> {
        match &track.kind {
            TrackKind::Drum(h) => h.iter().map(|v| *v > 0.0).collect(),
            _ => panic!("expected drum track, got {:?}", track.kind),
        }
    }

    fn notes(track: &Track) -> &Vec<Cell> {
        match &track.kind {
            TrackKind::Notes(c) => c,
            _ => panic!("expected note track, got {:?}", track.kind),
        }
    }

    /// Shorthand for Cell::Note with default velocity.
    fn note(midi: u8) -> Cell {
        Cell::Note(midi, VEL_NORMAL)
    }

    /// Shorthand for ChordCell::Chord with default velocity.
    fn chord(midis: &[u8]) -> ChordCell {
        ChordCell::Chord(midis.iter().map(|m| (*m, VEL_NORMAL)).collect())
    }

    fn chords(track: &Track) -> &Vec<ChordCell> {
        match &track.kind {
            TrackKind::Chord(c) => c,
            _ => panic!("expected chord track, got {:?}", track.kind),
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
        assert_eq!(pat.sections[0].steps, 16);
        assert_eq!(pat.sections[0].tracks.len(), 3);

        assert_eq!(pat.sections[0].tracks[0].name, "kick");
        let kick = drum_hits(&pat.sections[0].tracks[0]);
        assert_eq!(
            kick,
            vec![
                true, false, false, false, true, false, false, false, true, false, false, false,
                true, false, false, false,
            ]
        );

        let snare = drum_hits(&pat.sections[0].tracks[1]);
        assert!(snare[4]);
        assert!(snare[12]);

        let hihat = drum_hits(&pat.sections[0].tracks[2]);
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
        let cells = notes(&pat.sections[0].tracks[0]);
        assert_eq!(cells.len(), 8);
        assert_eq!(cells[0], note(36)); // C2
        assert_eq!(cells[1], Cell::Sustain);
        assert_eq!(cells[2], Cell::Sustain);
        assert_eq!(cells[3], Cell::Sustain);
        assert_eq!(cells[4], note(39)); // Eb2
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
        let cells = notes(&pat.sections[0].tracks[0]);
        assert_eq!(cells[0], note(60));
        assert_eq!(cells[1], note(62));
        assert_eq!(cells[2], note(63));
        assert_eq!(cells[3], note(66));
        assert_eq!(cells[4], Cell::Rest);
        assert_eq!(cells[5], Cell::Sustain);
        assert_eq!(cells[6], note(67));
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
        assert!(matches!(pat.sections[0].tracks[0].kind, TrackKind::Drum(_)));
        assert!(matches!(pat.sections[0].tracks[1].kind, TrackKind::Notes(_)));
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
        assert_eq!(pat.sections[0].steps, 8);
        let kick = drum_hits(&pat.sections[0].tracks[0]);
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
        let cells = notes(&pat.sections[0].tracks[0]);
        assert_eq!(cells, &vec![
            note(61),
            note(63),
            note(66),
            note(68),
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
            drum_hits(&pat.sections[0].tracks[0]),
            vec![true, false, true, false, true, false, true, false]
        );
    }

    #[test]
    fn drum_velocity_x_vs_big_x_vs_o() {
        let text = "\
bpm: 120
steps: 4
kick: Xx-o
";
        let pat = Pattern::parse(text).unwrap();
        if let TrackKind::Drum(vels) = &pat.sections[0].tracks[0].kind {
            assert!((vels[0] - VEL_ACCENT).abs() < 0.01);
            assert!((vels[1] - VEL_NORMAL).abs() < 0.01);
            assert_eq!(vels[2], 0.0);
            assert!((vels[3] - VEL_GHOST).abs() < 0.01);
        } else {
            panic!("expected drum track");
        }
    }

    #[test]
    fn global_swing_parsing() {
        let text = "\
bpm: 120
steps: 4
swing: 0.15
kick: x-x-
";
        let pat = Pattern::parse(text).unwrap();
        assert!((pat.swing - 0.15).abs() < 0.001);
    }

    #[test]
    fn per_section_bpm_and_swing() {
        let text = "\
bpm: 120
steps: 4

[verse]
bpm: 100
swing: 0.2
kick: x-x-

[chorus]
bpm: 140
kick: xxxx

song: verse chorus
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.bpm, 120);
        assert_eq!(pat.sections[0].bpm, Some(100));
        assert!((pat.sections[0].swing.unwrap() - 0.2).abs() < 0.001);
        assert_eq!(pat.sections[1].bpm, Some(140));
        assert!(pat.sections[1].swing.is_none()); // inherits global
    }

    #[test]
    fn note_velocity_accent_and_ghost() {
        let text = "\
bpm: 120
steps: 4
lead: C4! D4? E4 F4
";
        let pat = Pattern::parse(text).unwrap();
        let cells = notes(&pat.sections[0].tracks[0]);
        assert_eq!(cells[0], Cell::Note(60, VEL_ACCENT));
        assert_eq!(cells[1], Cell::Note(62, VEL_GHOST));
        assert_eq!(cells[2], Cell::Note(64, VEL_NORMAL));
        assert_eq!(cells[3], Cell::Note(65, VEL_NORMAL));
    }

    #[test]
    fn per_track_adsr_and_gain_parsing() {
        let text = "\
bpm: 120
steps: 4
bass.wave: sine
bass.attack: 10ms
bass.decay: 0.1
bass.sustain: 0.8
bass.release: 50ms
bass.gain: 1.5
bass.gate: 0.5
bass: C2 . . .
";
        let pat = Pattern::parse(text).unwrap();
        let t = &pat.sections[0].tracks[0];
        assert!((t.attack.unwrap() - 0.01).abs() < 0.001);
        assert!((t.decay.unwrap() - 0.1).abs() < 0.001);
        assert!((t.sustain.unwrap() - 0.8).abs() < 0.001);
        assert!((t.release.unwrap() - 0.05).abs() < 0.001);
        assert!((t.gain.unwrap() - 1.5).abs() < 0.001);
        assert!((t.gate.unwrap() - 0.5).abs() < 0.001);
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

    // ── slice 5a: per-track properties ─────────────────────────────────────

    #[test]
    fn parses_per_track_waveform_property() {
        let text = "\
bpm: 120
steps: 4
bass.wave: square
bass: C2 . . .
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.sections[0].tracks[0].wave, Some(Waveform::Square));
    }

    #[test]
    fn property_can_appear_after_track_row() {
        let text = "\
bpm: 120
steps: 4
bass: C2 . . .
bass.wave: triangle
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.sections[0].tracks[0].wave, Some(Waveform::Triangle));
    }

    #[test]
    fn waveform_aliases() {
        for (name, expected) in [
            ("sine", Waveform::Sine),
            ("sin", Waveform::Sine),
            ("square", Waveform::Square),
            ("sq", Waveform::Square),
            ("saw", Waveform::Sawtooth),
            ("sawtooth", Waveform::Sawtooth),
            ("triangle", Waveform::Triangle),
            ("tri", Waveform::Triangle),
            ("SAW", Waveform::Sawtooth),
        ] {
            assert_eq!(parse_waveform(name), Some(expected), "for {name}");
        }
        assert_eq!(parse_waveform("garbage"), None);
    }

    #[test]
    fn errors_on_unknown_property() {
        let text = "\
bpm: 120
steps: 4
bass.bogus: yes
bass: C2 . . .
";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::UnknownProperty { prop, .. } => assert_eq!(prop, "bogus"),
            other => panic!("expected UnknownProperty, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_invalid_property_value() {
        let text = "\
bpm: 120
steps: 4
bass.wave: cosine
bass: C2 . . .
";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::InvalidPropertyValue { prop, value, .. } => {
                assert_eq!(prop, "wave");
                assert_eq!(value, "cosine");
            }
            other => panic!("expected InvalidPropertyValue, got {other:?}"),
        }
    }

    // ── slice 5b: chord shorthand ──────────────────────────────────────────

    #[test]
    fn parses_basic_chord_shorthand() {
        // C major triad in default octave 3 → C3, E3, G3 = MIDI 48, 52, 55
        assert_eq!(parse_chord_shorthand("C", 3), Some(vec![48, 52, 55]));
        // C minor → C, Eb, G
        assert_eq!(parse_chord_shorthand("Cm", 3), Some(vec![48, 51, 55]));
        // C dominant 7 → C, E, G, Bb
        assert_eq!(parse_chord_shorthand("C7", 3), Some(vec![48, 52, 55, 58]));
        // C major 7 → C, E, G, B
        assert_eq!(parse_chord_shorthand("Cmaj7", 3), Some(vec![48, 52, 55, 59]));
        // C minor 7
        assert_eq!(parse_chord_shorthand("Cm7", 3), Some(vec![48, 51, 55, 58]));
        // C diminished
        assert_eq!(parse_chord_shorthand("Cdim", 3), Some(vec![48, 51, 54]));
        // C augmented
        assert_eq!(parse_chord_shorthand("Caug", 3), Some(vec![48, 52, 56]));
        // C sus2 / sus4
        assert_eq!(parse_chord_shorthand("Csus2", 3), Some(vec![48, 50, 55]));
        assert_eq!(parse_chord_shorthand("Csus4", 3), Some(vec![48, 53, 55]));
    }

    #[test]
    fn chord_shorthand_with_accidentals() {
        // Eb minor in octave 3: Eb3, Gb3, Bb3 = 51, 54, 58
        assert_eq!(parse_chord_shorthand("Ebm", 3), Some(vec![51, 54, 58]));
        // F# major in octave 4: F#4, A#4, C#5 = 66, 70, 73
        assert_eq!(parse_chord_shorthand("F#", 4), Some(vec![66, 70, 73]));
    }

    #[test]
    fn chord_shorthand_rejects_garbage() {
        assert_eq!(parse_chord_shorthand("Hm", 3), None);
        assert_eq!(parse_chord_shorthand("Cwhat", 3), None);
        assert_eq!(parse_chord_shorthand("", 3), None);
    }

    #[test]
    fn parses_chord_track() {
        let text = "\
bpm: 120
steps: 8
pad: Cm . . . Fm . . .
";
        let pat = Pattern::parse(text).unwrap();
        let cells = chords(&pat.sections[0].tracks[0]);
        assert_eq!(cells.len(), 8);
        assert_eq!(cells[0], chord(&[48, 51, 55])); // Cm in oct 3
        assert_eq!(cells[1], ChordCell::Sustain);
        assert_eq!(cells[4], chord(&[53, 56, 60])); // Fm in oct 3
    }

    #[test]
    fn chord_track_respects_octave_property() {
        let text = "\
bpm: 120
steps: 4
pad.octave: 4
pad: Cm . . .
";
        let pat = Pattern::parse(text).unwrap();
        let cells = chords(&pat.sections[0].tracks[0]);
        assert_eq!(cells[0], chord(&[60, 63, 67])); // Cm in oct 4
    }

    #[test]
    fn chord_octave_property_can_appear_after_row() {
        // Two-pass parser should pick up the octave even though it's declared
        // after the track row.
        let text = "\
bpm: 120
steps: 4
pad: Cm . . .
pad.octave: 5
";
        let pat = Pattern::parse(text).unwrap();
        let cells = chords(&pat.sections[0].tracks[0]);
        assert_eq!(cells[0], chord(&[72, 75, 79])); // Cm in oct 5
    }

    #[test]
    fn auto_detects_chord_row_from_unambiguous_token() {
        // Cm is unambiguous → entire row treated as chord row.
        let text = "\
bpm: 120
steps: 4
prog: Cm Fm G7 Cm
";
        let pat = Pattern::parse(text).unwrap();
        assert!(matches!(pat.sections[0].tracks[0].kind, TrackKind::Chord(_)));
    }

    #[test]
    fn note_row_when_no_chord_marker_present() {
        // C4, C7 are valid notes; no `m`/`maj`/etc. → note row.
        let text = "\
bpm: 120
steps: 4
melody: C4 D4 C7 E4
";
        let pat = Pattern::parse(text).unwrap();
        let cells = notes(&pat.sections[0].tracks[0]);
        assert_eq!(cells[0], note(60));
        assert_eq!(cells[2], note(96)); // C in octave 7, NOT C dom7
    }

    // ── slice 5c: sections + song chain ────────────────────────────────────

    #[test]
    fn single_pattern_files_become_one_main_section() {
        let text = "\
bpm: 120
steps: 4
kick: x---
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.sections.len(), 1);
        assert_eq!(pat.sections[0].name, "main");
        assert_eq!(pat.sections[0].steps, 4);
        assert_eq!(pat.song, vec![SongEntry { section: "main".into(), repeat: 1 }]);
    }

    #[test]
    fn parses_multiple_named_sections() {
        let text = "\
bpm: 120
steps: 4

[verse]
kick: x---

[chorus]
kick: x-x-
snare: ----
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.sections.len(), 2);
        assert_eq!(pat.sections[0].name, "verse");
        assert_eq!(pat.sections[0].tracks.len(), 1);
        assert_eq!(pat.sections[1].name, "chorus");
        assert_eq!(pat.sections[1].tracks.len(), 2);
    }

    #[test]
    fn default_song_chain_plays_each_section_once() {
        let text = "\
bpm: 120
steps: 4
[a]
kick: x---
[b]
kick: -x--
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(
            pat.song,
            vec![
                SongEntry { section: "a".into(), repeat: 1 },
                SongEntry { section: "b".into(), repeat: 1 },
            ]
        );
    }

    #[test]
    fn explicit_song_chain_with_repeats() {
        let text = "\
bpm: 120
steps: 4
song: intro verse x4 chorus verse x2 outro

[intro]
kick: x---
[verse]
kick: x-x-
[chorus]
kick: xx-x
[outro]
kick: x---
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(
            pat.song,
            vec![
                SongEntry { section: "intro".into(), repeat: 1 },
                SongEntry { section: "verse".into(), repeat: 4 },
                SongEntry { section: "chorus".into(), repeat: 1 },
                SongEntry { section: "verse".into(), repeat: 2 },
                SongEntry { section: "outro".into(), repeat: 1 },
            ]
        );
    }

    #[test]
    fn per_section_steps_overrides_global() {
        // `steps: 8` is the global default. `[short]` overrides it to 4;
        // `[normal]` has no override and inherits the global 8.
        let text = "\
bpm: 120
steps: 8
[short]
steps: 4
kick: x---
[normal]
kick: x-x-x-x-
[long]
steps: 16
kick: x-------x-------
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.section("short").unwrap().steps, 4);
        assert_eq!(pat.section("normal").unwrap().steps, 8);
        assert_eq!(pat.section("long").unwrap().steps, 16);
    }

    #[test]
    fn errors_on_song_referencing_unknown_section() {
        let text = "\
bpm: 120
steps: 4
song: intro mystery_section

[intro]
kick: x---
";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::UnknownSection { name, .. } => assert_eq!(name, "mystery_section"),
            other => panic!("expected UnknownSection, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_invalid_song_repeat() {
        let text = "\
bpm: 120
steps: 4
song: verse xfoo
[verse]
kick: x---
";
        match Pattern::parse(text).unwrap_err() {
            PatternParseError::InvalidSongRepeat { token, .. } => assert_eq!(token, "xfoo"),
            other => panic!("expected InvalidSongRepeat, got {other:?}"),
        }
    }

    #[test]
    fn properties_are_global_across_sections() {
        // `bass.wave: square` should apply to bass tracks in any section.
        let text = "\
bpm: 120
steps: 4
bass.wave: square

[verse]
bass: C2 . . .

[chorus]
bass: G2 . . .
";
        let pat = Pattern::parse(text).unwrap();
        assert_eq!(pat.sections[0].tracks[0].wave, Some(Waveform::Square));
        assert_eq!(pat.sections[1].tracks[0].wave, Some(Waveform::Square));
    }
}
