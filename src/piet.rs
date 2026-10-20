// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! The Piet-style 2D spatial surface for MAGMA programs.
//!
//! A program is a two-dimensional field of colored blocks, in the
//! spirit of Piet: the picture *is* the source. Each block holds
//! one Brainfuck command (see [`PALETTE`] for the color mapping);
//! empty blocks are void. The field is read along a continuous
//! spatial path — a boustrophedon (snake) traversal: the first row
//! left-to-right, the second right-to-left, alternating — skipping
//! void blocks. The resulting command stream is ordinary MAGMA
//! source and compiles through the usual pipeline.
//!
//! This is a spatial *notation* for MAGMA programs, not an
//! implementation of the Piet language: there is no direction
//! pointer, codel chooser, or stack. The 2D field is the layout;
//! the snake path is the reading order.

/// One palette entry: a command, its display name, and its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteEntry {
    pub command: char,
    pub name: &'static str,
    pub hex: &'static str,
}

/// The five command colors. The front end renders its palette
/// from this table (the server sends it in the WebSocket hello),
/// so this is the single source of truth for the mapping.
pub const PALETTE: [PaletteEntry; 5] = [
    PaletteEntry {
        command: '+',
        name: "increment",
        hex: "#E53935",
    },
    PaletteEntry {
        command: '-',
        name: "decrement",
        hex: "#43A047",
    },
    PaletteEntry {
        command: '>',
        name: "move right",
        hex: "#1E88E5",
    },
    PaletteEntry {
        command: '<',
        name: "move left",
        hex: "#FB8C00",
    },
    PaletteEntry {
        command: '.',
        name: "print tap",
        hex: "#8E24AA",
    },
];

/// The palette as a JSON array string — the front end's single
/// source of truth for the color mapping, served by both the
/// WebAssembly export and the WebSocket hello message.
pub fn palette_json() -> String {
    let entries: Vec<String> = PALETTE
        .iter()
        .map(|e| {
            format!(
                "{{\"command\":\"{}\",\"name\":\"{}\",\"hex\":\"{}\"}}",
                e.command, e.name, e.hex
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

/// The character marking a void block in a grid.
pub const VOID: char = ' ';

/// Is this character a grid command?
pub fn is_command(c: char) -> bool {
    PALETTE.iter().any(|e| e.command == c)
}

/// A 2D field of blocks, stored row-major. Rows may be short in
/// text form; the grid always keeps a full rectangle, padding
/// with void.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<char>,
}

impl Grid {
    /// An empty field of the given size.
    pub fn new(width: usize, height: usize) -> Grid {
        Grid {
            width,
            height,
            cells: vec![VOID; width * height],
        }
    }

    /// Parse the text form: one row per line, one block per
    /// character. Characters that are not commands become void.
    /// Short rows are padded with void.
    pub fn from_text(text: &str) -> Grid {
        let rows: Vec<&str> = text.lines().collect();
        let height = rows.len();
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut grid = Grid::new(width, height);
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if is_command(c) {
                    grid.set(x, y, c);
                }
            }
        }
        grid
    }

    /// The text form: one row per line, one block per character,
    /// trailing void trimmed per row.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            let row: String = (0..self.width).map(|x| self.get(x, y)).collect();
            out.push_str(row.trim_end());
            if y + 1 < self.height {
                out.push('\n');
            }
        }
        out
    }

    pub fn get(&self, x: usize, y: usize) -> char {
        self.cells[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, c: char) {
        let c = if is_command(c) { c } else { VOID };
        self.cells[y * self.width + x] = c;
    }

    /// Block positions in reading order: the snake path through
    /// the field (row 0 left-to-right, row 1 right-to-left, ...).
    pub fn snake_positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::with_capacity(self.width * self.height);
        for y in 0..self.height {
            if y % 2 == 0 {
                for x in 0..self.width {
                    positions.push((x, y));
                }
            } else {
                for x in (0..self.width).rev() {
                    positions.push((x, y));
                }
            }
        }
        positions
    }

    /// Serialize the field to a Brainfuck command stream by
    /// walking the snake path and skipping void blocks.
    pub fn to_bf(&self) -> String {
        let mut out = String::new();
        for (x, y) in self.snake_positions() {
            let c = self.get(x, y);
            if c != VOID {
                out.push(c);
            }
        }
        out
    }

    /// Lay a Brainfuck program out on a field of the given width:
    /// the source's command stream (non-command characters are
    /// comments in Brainfuck and are not laid out) is placed along
    /// the snake path, one block per command. Height is the
    /// smallest that fits.
    pub fn from_bf(source: &str, width: usize) -> Grid {
        let width = width.max(1);
        let commands: Vec<char> = source.chars().filter(|&c| is_command(c)).collect();
        let height = if commands.is_empty() {
            1
        } else {
            commands.len().div_ceil(width)
        };
        let mut grid = Grid::new(width, height);
        let positions = grid.snake_positions();
        for (i, &c) in commands.iter().enumerate() {
            let (x, y) = positions[i];
            grid.set(x, y, c);
        }
        grid
    }

    /// Compile the field: serialize, then run the MAGMA pipeline.
    pub fn compile(&self) -> Result<String, crate::error::CompileError> {
        crate::compile(&self.to_bf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_covers_exactly_the_five_commands() {
        let mut commands: Vec<char> = PALETTE.iter().map(|e| e.command).collect();
        commands.sort();
        assert_eq!(commands, vec!['+', '-', '.', '<', '>']);
        for e in PALETTE {
            assert!(e.hex.starts_with('#') && e.hex.len() == 7);
        }
    }

    #[test]
    fn snake_traversal_is_a_continuous_path() {
        let g = Grid::new(3, 3);
        assert_eq!(
            g.snake_positions(),
            vec![
                (0, 0),
                (1, 0),
                (2, 0),
                (2, 1),
                (1, 1),
                (0, 1),
                (0, 2),
                (1, 2),
                (2, 2),
            ]
        );
    }

    #[test]
    fn serialization_skips_void_and_snakes() {
        // Row 0 reads "++>", row 1 is read right-to-left: "-<"
        // laid out as '<' at x=0, '-' at x=1 reads back "-<" .
        let g = Grid::from_text("++>\n<- ");
        assert_eq!(g.to_bf(), "++>-<");
    }

    #[test]
    fn text_roundtrip() {
        let text = "+ +\n  -\n.<";
        let g = Grid::from_text(text);
        assert_eq!(g.to_text(), text);
        assert_eq!(
            g.to_bf().chars().collect::<Vec<_>>(),
            vec!['+', '+', '-', '.', '<']
        );
    }

    #[test]
    fn bf_layout_roundtrip() {
        let source = "+++>+++++>--.<.<.";
        let g = Grid::from_bf(source, 6);
        assert_eq!(g.width, 6);
        assert_eq!(g.height, 3);
        assert_eq!(g.to_bf(), source);
    }

    #[test]
    fn layout_ignores_comments_and_empty_is_one_row() {
        let g = Grid::from_bf("a b c", 4);
        assert_eq!(g.height, 1);
        assert_eq!(g.to_bf(), "");
        let g = Grid::from_bf("+ +", 4);
        assert_eq!(g.to_bf(), "++");
    }

    #[test]
    fn grid_compiles_like_its_serialization() {
        let g = Grid::from_text("+++>+\n    -");
        // Snake: row0 "+++>+", row1 reversed "<-..." -> wait, row 1
        // is "    -": the '-' sits at x=4, read first in reverse.
        assert_eq!(g.to_bf(), "+++>+-");
        assert_eq!(g.compile().unwrap(), crate::compile("+++>+-").unwrap());
    }

    #[test]
    fn non_commands_become_void() {
        let mut g = Grid::new(2, 1);
        g.set(0, 0, 'x');
        assert_eq!(g.get(0, 0), VOID);
        g.set(1, 0, '+');
        assert_eq!(g.to_bf(), "+");
    }
}
