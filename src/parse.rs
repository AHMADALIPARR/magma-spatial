// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

use crate::error::CompileError;

/// One source operation whose tape position was resolved at compile
/// time. Pointer motion itself is not recorded: only the operations
/// that read or write a cell survive parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Increment,
    Decrement,
    Print,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Op {
    pub kind: OpKind,
    /// Absolute source pointer position (may be negative; the
    /// lowering stage normalizes positions to port indices).
    pub pointer: i64,
}

/// A parsed program: the surviving operations plus the full span of
/// pointer positions the source ever addressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub ops: Vec<Op>,
    pub min_pointer: i64,
    pub max_pointer: i64,
}

impl Program {
    /// Number of addressed cells in the pointer span.
    pub fn cell_count(&self) -> usize {
        (self.max_pointer - self.min_pointer + 1) as usize
    }

    /// Index of a pointer position within the addressed span.
    pub fn index_of(&self, pointer: i64) -> usize {
        (pointer - self.min_pointer) as usize
    }
}

/// Scan Brainfuck source, resolving the tape pointer statically.
/// `>` and `<` move the compile-time pointer; `+`, `-` and `.` are
/// recorded against it; every other character is ignored. Loops and
/// input are rejected with the character index that requires
/// machinery this stage does not synthesize.
pub fn parse(source: &str) -> Result<Program, CompileError> {
    let mut pointer: i64 = 0;
    let mut min_pointer: i64 = 0;
    let mut max_pointer: i64 = 0;
    let mut ops: Vec<Op> = Vec::new();
    for (index, c) in source.chars().enumerate() {
        match c {
            '>' => pointer += 1,
            '<' => pointer -= 1,
            '+' => ops.push(Op {
                kind: OpKind::Increment,
                pointer,
            }),
            '-' => ops.push(Op {
                kind: OpKind::Decrement,
                pointer,
            }),
            '.' => ops.push(Op {
                kind: OpKind::Print,
                pointer,
            }),
            '[' | ']' => return Err(CompileError::DynamicLoop { character: index }),
            ',' => return Err(CompileError::StreamingInput { character: index }),
            _ => {}
        }
        min_pointer = min_pointer.min(pointer);
        max_pointer = max_pointer.max(pointer);
    }
    Ok(Program {
        ops,
        min_pointer,
        max_pointer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_ops_against_static_pointer() {
        let p = parse("++>---<.").unwrap();
        assert_eq!(
            p.ops,
            vec![
                Op {
                    kind: OpKind::Increment,
                    pointer: 0
                },
                Op {
                    kind: OpKind::Increment,
                    pointer: 0
                },
                Op {
                    kind: OpKind::Decrement,
                    pointer: 1
                },
                Op {
                    kind: OpKind::Decrement,
                    pointer: 1
                },
                Op {
                    kind: OpKind::Decrement,
                    pointer: 1
                },
                Op {
                    kind: OpKind::Print,
                    pointer: 0
                },
            ]
        );
        assert_eq!(p.min_pointer, 0);
        assert_eq!(p.max_pointer, 1);
        assert_eq!(p.cell_count(), 2);
    }

    #[test]
    fn negative_positions_are_tracked() {
        let p = parse("<<+").unwrap();
        assert_eq!(p.min_pointer, -2);
        assert_eq!(p.max_pointer, 0);
        assert_eq!(p.cell_count(), 3);
        assert_eq!(p.index_of(-2), 0);
        assert_eq!(p.index_of(0), 2);
    }

    #[test]
    fn non_command_characters_are_ignored() {
        let p = parse("a b\nc+\td").unwrap();
        assert_eq!(p.ops.len(), 1);
        assert_eq!(p.cell_count(), 1);
    }

    #[test]
    fn empty_source_addresses_one_cell() {
        let p = parse("").unwrap();
        assert!(p.ops.is_empty());
        assert_eq!(p.cell_count(), 1);
    }

    #[test]
    fn loops_are_rejected_with_character_index() {
        assert_eq!(
            parse("++[>+<-]"),
            Err(CompileError::DynamicLoop { character: 2 })
        );
        assert_eq!(parse("]"), Err(CompileError::DynamicLoop { character: 0 }));
    }

    #[test]
    fn input_is_rejected_with_character_index() {
        assert_eq!(
            parse("+,"),
            Err(CompileError::StreamingInput { character: 1 })
        );
    }

    #[test]
    fn first_rejection_wins() {
        // A loop bracket precedes the input character, so the loop
        // error is the one reported (the scan is strictly in order).
        assert_eq!(parse("[,"), Err(CompileError::DynamicLoop { character: 0 }));
    }
}
