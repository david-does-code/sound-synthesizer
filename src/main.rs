mod audio;
mod envelope;
mod filter;
mod keyboard;
mod pluck;
mod notes;
mod pattern;
mod render;
mod reverb;
mod sequencer;
mod visualizer;

use audio::{AudioEngine, Waveform};
use crossterm::terminal;
use envelope::AdsrParams;
use keyboard::{spawn_keyboard_listener, KeyboardEvent};
use notes::keyboard_help;
use pattern::{Cell, ChordCell, Pattern, TrackKind};
use sequencer::Sequencer;
use std::collections::HashMap;
use std::io::{self, Write};

/// Number of braille rows in the visualizer display.
const VIS_ROWS: u16 = 4;

/// Octave offset range.
const MIN_OCTAVE_OFFSET: i8 = -3;
const MAX_OCTAVE_OFFSET: i8 = 3;

/// Which UI mode we're in.
#[derive(PartialEq)]
enum Mode {
    Piano,
    AdsrEditor,
}

/// Which ADSR parameter is selected in the editor.
#[derive(Clone, Copy, PartialEq)]
enum AdsrParam {
    Attack,
    Decay,
    Sustain,
    Release,
}

impl AdsrParam {
    fn next(self) -> Self {
        match self {
            AdsrParam::Attack => AdsrParam::Decay,
            AdsrParam::Decay => AdsrParam::Sustain,
            AdsrParam::Sustain => AdsrParam::Release,
            AdsrParam::Release => AdsrParam::Release,
        }
    }
    fn prev(self) -> Self {
        match self {
            AdsrParam::Attack => AdsrParam::Attack,
            AdsrParam::Decay => AdsrParam::Attack,
            AdsrParam::Sustain => AdsrParam::Decay,
            AdsrParam::Release => AdsrParam::Sustain,
        }
    }
}

/// Total lines used by the piano mode live area.
/// Waveform (4) + status line (1) + blank (1)
const PIANO_LIVE_ROWS: u16 = VIS_ROWS + 2;

/// Total lines used by the ADSR editor live area.
/// Envelope graph (4) + blank (1) + param labels (1) + param values (1) + blank (1) + hint (1) + blank (1)
const ADSR_LIVE_ROWS: u16 = VIS_ROWS + 6;

fn main() {
    // CLI: `--play <file.pat>` plays a pattern from disk and exits when you press Enter.
    // No flag = the existing interactive piano.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--play" {
        play_pattern(&args[2]);
        return;
    }
    if args.len() >= 4 && args[1] == "--render" {
        render_pattern(&args[2], &args[3]);
        return;
    }
    if args.len() >= 2 && (args[1] == "--help" || args[1] == "-h") {
        print_cli_help();
        return;
    }
    // Any other arg is a usage error — without this guard, unknown args silently
    // fell through to the interactive piano, which surprised users who expected
    // e.g. `cargo run foo.wav` to play the WAV.
    if args.len() >= 2 {
        eprintln!("Unknown argument: {}", args[1]);
        eprintln!();
        print_cli_help();
        std::process::exit(2);
    }

    let engine = AudioEngine::new();

    // Print the initial header BEFORE raw mode so newlines work normally
    print_piano_header();

    terminal::enable_raw_mode().expect("Failed to enable raw mode");

    let events = spawn_keyboard_listener();

    let mut mode = Mode::Piano;
    let mut octave_offset: i8 = 0;
    // Maps MIDI note (after offset) → voice index in the audio engine.
    // This lets us release the correct voice when a key is released.
    let mut note_to_voice: HashMap<u8, usize> = HashMap::new();
    let mut adsr = AdsrParams::default();
    let mut selected_param = AdsrParam::Attack;

    // Draw the live area (this part is fine in raw mode — uses explicit \r\n)
    draw_piano_live_area(engine.waveform(), octave_offset, None);

    for event in events {
        match event {
            // === Notes work in both modes ===
            KeyboardEvent::NoteOn { base_midi } => {
                let midi = apply_offset(base_midi, octave_offset);
                let voice_idx = engine.play_note(midi);
                note_to_voice.insert(midi, voice_idx);
                if mode == Mode::Piano {
                    let freq = audio::midi_to_freq(midi);
                    let name = audio::midi_to_name(midi);
                    redraw_piano_live_area(
                        engine.waveform(),
                        octave_offset,
                        Some((&name, freq, midi)),
                    );
                }
            }
            KeyboardEvent::NoteOff { base_midi } => {
                let midi = apply_offset(base_midi, octave_offset);
                if let Some(voice_idx) = note_to_voice.remove(&midi) {
                    engine.release_voice(voice_idx);
                }
                if mode == Mode::Piano && note_to_voice.is_empty() {
                    redraw_piano_live_area(engine.waveform(), octave_offset, None);
                }
            }

            // === Mode toggle ===
            KeyboardEvent::ToggleMode => {
                clear_screen();
                match mode {
                    Mode::Piano => {
                        mode = Mode::AdsrEditor;
                        print_adsr_header();
                        draw_adsr_live_area(&adsr, selected_param);
                    }
                    Mode::AdsrEditor => {
                        mode = Mode::Piano;
                        print_piano_header_raw();
                        draw_piano_live_area(engine.waveform(), octave_offset, None);
                    }
                }
            }

            // === Piano-mode controls ===
            KeyboardEvent::WaveformChange(waveform) if mode == Mode::Piano => {
                engine.set_waveform(waveform);
                redraw_piano_live_area(engine.waveform(), octave_offset, None);
            }
            KeyboardEvent::OctaveUp if mode == Mode::Piano => {
                if octave_offset < MAX_OCTAVE_OFFSET {
                    octave_offset += 1;
                }
                redraw_piano_live_area(engine.waveform(), octave_offset, None);
            }
            KeyboardEvent::OctaveDown if mode == Mode::Piano => {
                if octave_offset > MIN_OCTAVE_OFFSET {
                    octave_offset -= 1;
                }
                redraw_piano_live_area(engine.waveform(), octave_offset, None);
            }

            // === ADSR editor controls ===
            KeyboardEvent::ArrowLeft if mode == Mode::AdsrEditor => {
                selected_param = selected_param.prev();
                redraw_adsr_live_area(&adsr, selected_param);
            }
            KeyboardEvent::ArrowRight if mode == Mode::AdsrEditor => {
                selected_param = selected_param.next();
                redraw_adsr_live_area(&adsr, selected_param);
            }
            KeyboardEvent::ArrowUp if mode == Mode::AdsrEditor => {
                adjust_adsr_param(&mut adsr, selected_param, true);
                engine.set_adsr(adsr);
                redraw_adsr_live_area(&adsr, selected_param);
            }
            KeyboardEvent::ArrowDown if mode == Mode::AdsrEditor => {
                adjust_adsr_param(&mut adsr, selected_param, false);
                engine.set_adsr(adsr);
                redraw_adsr_live_area(&adsr, selected_param);
            }

            KeyboardEvent::Quit => break,
            _ => {}
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

fn apply_offset(base_midi: u8, octave_offset: i8) -> u8 {
    let shifted = base_midi as i16 + (octave_offset as i16 * 12);
    shifted.clamp(0, 127) as u8
}

/// Adjust the selected ADSR parameter up or down.
fn adjust_adsr_param(adsr: &mut AdsrParams, param: AdsrParam, increase: bool) {
    let time_step = 0.01; // 10ms per step
    let level_step = 0.02; // 2% per step
    match param {
        AdsrParam::Attack => {
            if increase {
                adsr.attack = (adsr.attack + time_step).min(2.0);
            } else {
                adsr.attack = (adsr.attack - time_step).max(0.0);
            }
        }
        AdsrParam::Decay => {
            if increase {
                adsr.decay = (adsr.decay + time_step).min(2.0);
            } else {
                adsr.decay = (adsr.decay - time_step).max(0.0);
            }
        }
        AdsrParam::Sustain => {
            if increase {
                adsr.sustain = (adsr.sustain + level_step).min(1.0);
            } else {
                adsr.sustain = (adsr.sustain - level_step).max(0.0);
            }
        }
        AdsrParam::Release => {
            if increase {
                adsr.release = (adsr.release + time_step).min(2.0);
            } else {
                adsr.release = (adsr.release - time_step).max(0.0);
            }
        }
    }
}

// ─── Screen drawing ────────────────────────────────────────────────

/// Print a string with raw-mode-safe newlines (\n → \r\n).
fn raw_println(stdout: &mut io::Stdout, s: &str) {
    for line in s.split('\n') {
        write!(stdout, "\r{line}\r\n").ok();
    }
}

fn print_piano_header() {
    println!("  🎹 Sound Synthesizer — Keyboard Piano");
    println!("  =======================================");
    println!("{}", keyboard_help());
    println!();
    println!("  Hold keys to play, release to stop. ESC to quit.");
    println!("  [1] Sine  [2] Square  [3] Sawtooth  [4] Triangle");
    println!("  [Z] Octave down  [X] Octave up  [Tab] ADSR Editor");
    println!();
}

/// Print the ADSR header in raw mode (used when switching modes).
fn print_adsr_header() {
    let mut stdout = io::stdout();
    raw_println(&mut stdout, "  🎛  ADSR Envelope Editor");
    raw_println(&mut stdout, "  ========================");
    raw_println(&mut stdout, "");
    raw_println(&mut stdout, "  Hold note keys to preview the envelope.");
    raw_println(&mut stdout, "");
    stdout.flush().ok();
}

/// Print the piano header in raw mode (used when switching back from ADSR).
fn print_piano_header_raw() {
    let mut stdout = io::stdout();
    raw_println(&mut stdout, "  🎹 Sound Synthesizer — Keyboard Piano");
    raw_println(&mut stdout, "  =======================================");
    // keyboard_help() contains embedded newlines — raw_println handles them
    raw_println(&mut stdout, keyboard_help());
    raw_println(&mut stdout, "");
    raw_println(&mut stdout, "  Hold keys to play, release to stop. ESC to quit.");
    raw_println(&mut stdout, "  [1] Sine  [2] Square  [3] Sawtooth  [4] Triangle");
    raw_println(&mut stdout, "  [Z] Octave down  [X] Octave up  [Tab] ADSR Editor");
    raw_println(&mut stdout, "");
    stdout.flush().ok();
}

/// Clear the entire screen and move cursor to top-left.
fn clear_screen() {
    let mut stdout = io::stdout();
    write!(stdout, "\x1b[2J\x1b[H").ok();
    stdout.flush().ok();
}

// ─── Piano mode live area ──────────────────────────────────────────

fn draw_piano_live_area(waveform: Waveform, octave_offset: i8, note: Option<(&str, f32, u8)>) {
    let mut stdout = io::stdout();
    let lines = visualizer::render_waveform(waveform);
    for line in &lines {
        write!(stdout, "\r  {line}  \r\n").ok();
    }
    write_piano_status(&mut stdout, waveform, octave_offset, note);
    write!(stdout, "\r{:65}\r\n", "").ok();
    stdout.flush().ok();
}

fn redraw_piano_live_area(waveform: Waveform, octave_offset: i8, note: Option<(&str, f32, u8)>) {
    let mut stdout = io::stdout();
    write!(stdout, "\x1b[{PIANO_LIVE_ROWS}A").ok();
    let lines = visualizer::render_waveform(waveform);
    for line in &lines {
        write!(stdout, "\r  {line}  \r\n").ok();
    }
    write_piano_status(&mut stdout, waveform, octave_offset, note);
    write!(stdout, "\r{:65}\r\n", "").ok();
    stdout.flush().ok();
}

fn write_piano_status(stdout: &mut io::Stdout, waveform: Waveform, octave_offset: i8, note: Option<(&str, f32, u8)>) {
    let oct = match octave_offset.cmp(&0) {
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
            ).ok();
        }
        None => {
            write!(
                stdout,
                "\r  [{wave}]  Octave: {oct}                                    \r\n",
                wave = waveform.name(),
            ).ok();
        }
    }
}

// ─── ADSR editor live area ─────────────────────────────────────────

fn draw_adsr_live_area(adsr: &AdsrParams, selected: AdsrParam) {
    let mut stdout = io::stdout();
    write_adsr_content(&mut stdout, adsr, selected);
    stdout.flush().ok();
}

fn redraw_adsr_live_area(adsr: &AdsrParams, selected: AdsrParam) {
    let mut stdout = io::stdout();
    write!(stdout, "\x1b[{ADSR_LIVE_ROWS}A").ok();
    write_adsr_content(&mut stdout, adsr, selected);
    stdout.flush().ok();
}

fn write_adsr_content(stdout: &mut io::Stdout, adsr: &AdsrParams, selected: AdsrParam) {
    // Envelope visualization
    let lines = visualizer::render_envelope(adsr);
    for line in &lines {
        write!(stdout, "\r  {line}  \r\n").ok();
    }
    write!(stdout, "\r\n").ok();

    // Parameter labels with selector
    let labels = [
        (AdsrParam::Attack, "Attack"),
        (AdsrParam::Decay, "Decay"),
        (AdsrParam::Sustain, "Sustain"),
        (AdsrParam::Release, "Release"),
    ];
    write!(stdout, "\r  ").ok();
    for (param, name) in &labels {
        if *param == selected {
            write!(stdout, "  ▶ {name:<10}").ok();
        } else {
            write!(stdout, "    {name:<10}").ok();
        }
    }
    write!(stdout, "  \r\n").ok();

    // Parameter values
    let attack_ms = (adsr.attack * 1000.0).round() as u32;
    let decay_ms = (adsr.decay * 1000.0).round() as u32;
    let sustain_pct = (adsr.sustain * 100.0).round() as u32;
    let release_ms = (adsr.release * 1000.0).round() as u32;

    let values = [
        (AdsrParam::Attack, format!("{attack_ms}ms")),
        (AdsrParam::Decay, format!("{decay_ms}ms")),
        (AdsrParam::Sustain, format!("{sustain_pct}%")),
        (AdsrParam::Release, format!("{release_ms}ms")),
    ];
    write!(stdout, "\r  ").ok();
    for (param, val) in &values {
        if *param == selected {
            write!(stdout, "  ▶ {val:<10}").ok();
        } else {
            write!(stdout, "    {val:<10}").ok();
        }
    }
    write!(stdout, "  \r\n").ok();

    write!(stdout, "\r\n").ok();
    write!(stdout, "\r  ◀/▶ Select   ▲/▼ Adjust   [Tab] Back to piano       \r\n").ok();
    write!(stdout, "\r{:65}\r\n", "").ok();
}

// ─── Pattern playback (CLI) ────────────────────────────────────────

fn print_cli_help() {
    println!("Sound Synthesizer");
    println!();
    println!("Usage:");
    println!("  cargo run                       Interactive keyboard piano");
    println!("  cargo run -- --play <file.pat>  Play a pattern file in a loop");
    println!("  cargo run -- --render <file.pat> <out.wav>  Render one pass to WAV");
    println!("  cargo run -- --help             Show this help");
}

fn render_pattern(input: &str, output: &str) {
    let pattern = match Pattern::from_file(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to load pattern '{input}': {e}");
            std::process::exit(1);
        }
    };
    println!("  Rendering {input}  →  {output}");
    match render::render_to_wav(&pattern, std::path::Path::new(output)) {
        Ok(stats) => {
            println!(
                "  Done. {:.2}s, {} sections, peak {:.3}",
                stats.duration_secs, stats.sections_played, stats.peak_amplitude
            );
            if stats.peak_amplitude >= 1.0 {
                println!("  ⚠  output clipped (peak ≥ 1.0)");
            }
        }
        Err(e) => {
            eprintln!("Render failed: {e}");
            std::process::exit(1);
        }
    }
}

fn play_pattern(path: &str) {
    let pattern = match Pattern::from_file(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to load pattern '{path}': {e}");
            std::process::exit(1);
        }
    };

    println!("  🥁 Playing {path}");
    println!("  ────────────────────────────────────────");
    print!("  Tempo: {} BPM", pattern.bpm);
    if pattern.swing > 0.0 {
        print!("  Swing: {:.0}%", pattern.swing * 100.0);
    }
    println!();

    // Song chain summary.
    let chain: Vec<String> = pattern
        .song
        .iter()
        .map(|e| {
            if e.repeat == 1 {
                e.section.clone()
            } else {
                format!("{} x{}", e.section, e.repeat)
            }
        })
        .collect();
    println!("  Song:  {}", chain.join("  →  "));
    let total_bars: u32 = pattern
        .song
        .iter()
        .map(|e| e.repeat)
        .sum();
    println!("  ({} sections, {} total plays per loop)", pattern.sections.len(), total_bars);
    println!();

    for section in &pattern.sections {
        let mut section_info = format!("  [{}]  ({} steps", section.name, section.steps);
        if let Some(bpm) = section.bpm {
            section_info.push_str(&format!(", {} BPM", bpm));
        }
        if let Some(swing) = section.swing {
            section_info.push_str(&format!(", swing {:.0}%", swing * 100.0));
        }
        section_info.push(')');
        println!("{section_info}");
        for t in &section.tracks {
            let wave_label = match t.wave {
                Some(w) => format!(" [{}]", w.name().to_lowercase()),
                None => String::new(),
            };
            match &t.kind {
                TrackKind::Drum(hits) => {
                    let visual: String = hits.iter().map(|v| {
                        if *v >= crate::pattern::VEL_ACCENT - 0.01 { 'X' }
                        else if *v > 0.0 { 'x' }
                        else { '-' }
                    }).collect();
                    println!("    🥁 {:<8}{wave_label} {}", t.name, visual);
                }
                TrackKind::Notes(cells) => {
                    let visual: String = cells
                        .iter()
                        .map(|c| match c {
                            Cell::Note(_, _) => 'N',
                            Cell::Sustain => '.',
                            Cell::Rest => '-',
                        })
                        .collect();
                    println!("    ♪  {:<8}{wave_label} {}", t.name, visual);
                }
                TrackKind::Chord(cells) => {
                    let visual: String = cells
                        .iter()
                        .map(|c| match c {
                            ChordCell::Chord(_) => 'C',
                            ChordCell::Sustain => '.',
                            ChordCell::Rest => '-',
                        })
                        .collect();
                    println!("    ♬  {:<8}{wave_label} {}", t.name, visual);
                }
            }
        }
        println!();
    }
    println!("  Press Enter to stop.");

    let engine = AudioEngine::new();
    let engine_handle = engine.engine_handle();

    let sample_rate = engine_handle.sample_rate() as f64;
    println!("  audio: {sample_rate} Hz");
    println!();

    let mut seq = Sequencer::new(pattern, engine_handle);
    seq.start();

    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);

    seq.stop();
    println!("  Stopped.");
}
