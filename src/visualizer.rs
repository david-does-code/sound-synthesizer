use crate::audio::Waveform;

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

/// Render one cycle of the given waveform as a string of braille lines.
/// Returns a Vec of strings, one per display row.
pub fn render_waveform(waveform: Waveform) -> Vec<String> {
    let dot_cols = WIDTH * 2;
    let dot_rows = HEIGHT * 4;

    // A 2D grid of braille cells, storing the accumulated bits
    let mut grid = vec![vec![0u8; WIDTH]; HEIGHT];

    // Sample one full cycle of the waveform across the display width
    for dx in 0..dot_cols {
        let phase = dx as f32 / dot_cols as f32;
        let sample = waveform.sample(phase); // -1.0 to 1.0

        // Map sample value to a dot row (0 = top, dot_rows-1 = bottom)
        // -1.0 → bottom, +1.0 → top
        let y = ((1.0 - sample) * 0.5 * (dot_rows - 1) as f32)
            .round()
            .clamp(0.0, (dot_rows - 1) as f32) as usize;

        // Which braille cell does this dot land in?
        let cell_col = dx / 2;
        let cell_row = y / 4;
        let dot_col = dx % 2;
        let dot_row = y % 4;

        grid[cell_row][cell_col] |= braille_bit(dot_row, dot_col);
    }

    // Convert the bit grid to braille characters
    grid.iter()
        .map(|row| {
            row.iter()
                .map(|&bits| char::from_u32(0x2800 + bits as u32).unwrap_or(' '))
                .collect::<String>()
        })
        .collect()
}

