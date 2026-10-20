// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

use crate::error::CompileError;
use crate::parse::{OpKind, Program};

/// Maximum number of addressed cells the compiler will emit lanes
/// for. Beyond this the resource guard refuses the design.
pub const MAX_CELLS: usize = 4096;

/// A lowered program: one net delta per addressed cell, and one
/// output tap per `.` operation. Writes to the same cell are
/// combined; each tap snapshots the cell's delta at its point in
/// source execution, so intermediate values are preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    /// Net 16-bit-wrapping delta per cell index (unnormalized here;
    /// codegen reduces modulo 2^16).
    pub deltas: Vec<i64>,
    /// `(cell index, delta at print time)` per print, in source order.
    pub output_taps: Vec<(usize, i64)>,
    /// Total number of `+`/`-` operations in the source.
    pub writes: usize,
}

/// Combine a parsed program's writes per cell. Independent cells
/// become independent combinational lanes, so the lowering is a
/// pure fold over the operation list.
pub fn lower(program: &Program) -> Result<Lowered, CompileError> {
    let count = program.cell_count();
    if count > MAX_CELLS {
        return Err(CompileError::TooManyCells { count });
    }
    let mut deltas: Vec<i64> = vec![0; count];
    let mut output_taps: Vec<(usize, i64)> = Vec::new();
    let mut writes: usize = 0;
    for op in &program.ops {
        let index = program.index_of(op.pointer);
        match op.kind {
            OpKind::Increment => {
                deltas[index] += 1;
                writes += 1;
            }
            OpKind::Decrement => {
                deltas[index] -= 1;
                writes += 1;
            }
            OpKind::Print => output_taps.push((index, deltas[index])),
        }
    }
    Ok(Lowered {
        deltas,
        output_taps,
        writes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn lowered(source: &str) -> Lowered {
        lower(&parse(source).unwrap()).unwrap()
    }

    #[test]
    fn writes_to_same_cell_combine() {
        let l = lowered("+++>+++++>--.");
        assert_eq!(l.deltas, vec![3, 5, -2]);
        assert_eq!(l.writes, 10);
    }

    #[test]
    fn taps_snapshot_intermediate_values() {
        // The first print sees only the increments before it; the
        // second sees the decrements as well.
        let l = lowered("++.--.");
        assert_eq!(l.output_taps, vec![(0, 2), (0, 0)]);
        assert_eq!(l.deltas, vec![0]);
    }

    #[test]
    fn negative_pointer_span_is_normalized() {
        let l = lowered("<++>---.");
        assert_eq!(l.deltas, vec![2, -3]);
        assert_eq!(l.output_taps, vec![(1, -3)]);
    }

    #[test]
    fn empty_program_lowers_to_one_neutral_lane() {
        let l = lowered("");
        assert_eq!(l.deltas, vec![0]);
        assert!(l.output_taps.is_empty());
        assert_eq!(l.writes, 0);
    }

    #[test]
    fn resource_guard_boundary() {
        // 4095 moves right: 4096 addressed cells, accepted.
        let ok = ">".repeat(MAX_CELLS - 1);
        assert!(lower(&parse(&ok).unwrap()).is_ok());
        // 4096 moves right: 4097 addressed cells, refused.
        let too_far = ">".repeat(MAX_CELLS);
        assert_eq!(
            lower(&parse(&too_far).unwrap()),
            Err(CompileError::TooManyCells {
                count: MAX_CELLS + 1
            })
        );
    }
}
