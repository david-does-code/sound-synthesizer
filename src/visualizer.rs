use crate::audio::Waveform;
use crate::envelope::AdsrParams;

/// Width of the waveform display in terminal columns.
/// Each braille character encodes a 2-wide × 4-tall dot grid,
/// so we get (WIDTH × 2) horizontal dots and (HEIGHT × 4) vertical dots.
const WIDTH: usize = 60;
const HEIGHT: usize = 4; // rows of braille characters

/// Braille character rendering.
///
/// Unicode braille characters (U+2800–U+28FF) encode a 2×4 dot grid
/// in a single character. Each of the 8 dots maps to a bit:
///
///   dot1 (0x01)  dot4 (0x08)
///   dot2 (0x02)  dot5 (0x10)
///   dot3 (0x04)  dot6 (0x20)
///   dot7 (0x40)  dot8 (0x80)
///
/// By setting the right bits, we can draw at sub-character resolution —
/// effectively 2× horizontal and 4× vertical resolution compared to
/// regular text characters. This gives us smooth-looking curves.

/// Maps a (row, col) position within a braille cell to its bit.
/// row: 0-3 (top to bottom), col: 0-1 (left to right)
fn braille_bit(row: usize, col: usize) -> u8 {
    match (row, col) {
        (0, 0) => 0x01,
        (1, 0) => 0x02,
        (2, 0) => 0x04,
        (3, 0) => 0x40,
        (0, 1) => 0x08,
        (1, 1) => 0x10,
        (2, 1) => 0x20,
        (3, 1) => 0x80,
        _ => 0,
    }
}

/// Plot a series of (x, y) points where y is 0.0–1.0 (0=bottom, 1=top)
/// into braille lines. Shared by both waveform and envelope renderers.
fn render_braille(samples: &[(f32, f32)], width: usize, height: usize) -> Vec<String> {
    let dot_cols = width * 2;
    let dot_rows = height * 4;

    let mut grid = vec![vec![0u8; width]; height];

    for &(x_frac, y_val) in samples {
        let dx = (x_frac * (dot_cols - 1) as f32)
            .round()
            .clamp(0.0, (dot_cols - 1) as f32) as usize;

        // y_val 1.0 = top (row 0), 0.0 = bottom
        let dy = ((1.0 - y_val) * (dot_rows - 1) as f32)
            .round()
            .clamp(0.0, (dot_rows - 1) as f32) as usize;

        let cell_col = dx / 2;
        let cell_row = dy / 4;
        let dot_col = dx % 2;
        let dot_row = dy % 4;

        if cell_col < width && cell_row < height {
            grid[cell_row][cell_col] |= braille_bit(dot_row, dot_col);
        }
    }

    grid.iter()
        .map(|row| {
            row.iter()
                .map(|&bits| char::from_u32(0x2800 + bits as u32).unwrap_or(' '))
                .collect::<String>()
        })
        .collect()
}

/// Render one cycle of the given waveform as braille lines.
pub fn render_waveform(waveform: Waveform) -> Vec<String> {
    let dot_cols = WIDTH * 2;
    let samples: Vec<(f32, f32)> = (0..dot_cols)
        .map(|dx| {
            let phase = dx as f32 / dot_cols as f32;
            let sample = waveform.sample(phase); // -1.0 to 1.0
            // Map to 0.0–1.0 range for the renderer
            (phase, (sample + 1.0) * 0.5)
        })
        .collect();

    render_braille(&samples, WIDTH, HEIGHT)
}

/// Render the ADSR envelope shape as braille lines.
///
/// The display is divided proportionally:
/// - Attack ramp (0 → 1.0)
/// - Decay slope (1.0 → sustain)
/// - Sustain flat (held at sustain level, ~40% of display width)
/// - Release slope (sustain → 0)
pub fn render_envelope(params: &AdsrParams) -> Vec<String> {
    let dot_cols = WIDTH * 2;

    // Allocate horizontal space proportionally to the time values,
    // with sustain getting a fixed portion since it's a level, not a time.
    let total_time = params.attack + params.decay + params.release;
    let sustain_frac = 0.35; // sustain always gets 35% of display width
    let time_frac = 1.0 - sustain_frac;

    let attack_frac = if total_time > 0.0 {
        (params.attack / total_time) * time_frac
    } else {
        0.1
    };
    let decay_frac = if total_time > 0.0 {
        (params.decay / total_time) * time_frac
    } else {
        0.1
    };
    let release_frac = if total_time > 0.0 {
        (params.release / total_time) * time_frac
    } else {
        0.1
    };

    let mut samples = Vec::with_capacity(dot_cols);

    for dx in 0..dot_cols {
        let x = dx as f32 / dot_cols as f32;
        let y = if x < attack_frac {
            // Attack: ramp 0 → 1
            x / attack_frac
        } else if x < attack_frac + decay_frac {
            // Decay: ramp 1 → sustain
            let t = (x - attack_frac) / decay_frac;
            1.0 - t * (1.0 - params.sustain)
        } else if x < attack_frac + decay_frac + sustain_frac {
            // Sustain: flat at sustain level
            params.sustain
        } else {
            // Release: ramp sustain → 0
            let release_start = attack_frac + decay_frac + sustain_frac;
            let t = (x - release_start) / release_frac;
            params.sustain * (1.0 - t.clamp(0.0, 1.0))
        };

        samples.push((dx as f32 / dot_cols as f32, y.clamp(0.0, 1.0)));
    }

    render_braille(&samples, WIDTH, HEIGHT)
}
