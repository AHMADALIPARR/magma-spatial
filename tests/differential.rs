// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! Differential test: the generated Verilog, evaluated as
//! arithmetic, must agree with a reference straight-line Brainfuck
//! interpreter on final cell values and on every print tap, for
//! the shipped example and for randomized programs.

use std::collections::BTreeMap;

/// Reference interpreter for the straight-line subset. Cells are
/// 16-bit wrapping; the tape span is the pointer's full travel,
/// exactly as the compiler addresses it.
fn interpret(source: &str, inputs: &BTreeMap<i64, u16>) -> (BTreeMap<i64, u16>, Vec<u16>) {
    let mut pointer: i64 = 0;
    let mut min_pointer: i64 = 0;
    let mut max_pointer: i64 = 0;
    for c in source.chars() {
        match c {
            '>' => pointer += 1,
            '<' => pointer -= 1,
            _ => {}
        }
        min_pointer = min_pointer.min(pointer);
        max_pointer = max_pointer.max(pointer);
    }
    let mut cells: BTreeMap<i64, u16> = BTreeMap::new();
    for position in min_pointer..=max_pointer {
        cells.insert(position, inputs.get(&position).copied().unwrap_or(0));
    }
    let mut prints = Vec::new();
    let mut pointer: i64 = 0;
    for c in source.chars() {
        match c {
            '>' => pointer += 1,
            '<' => pointer -= 1,
            '+' => {
                let cell = cells.get_mut(&pointer).unwrap();
                *cell = cell.wrapping_add(1);
            }
            '-' => {
                let cell = cells.get_mut(&pointer).unwrap();
                *cell = cell.wrapping_sub(1);
            }
            '.' => prints.push(*cells.get(&pointer).unwrap()),
            _ => {}
        }
    }
    (cells, prints)
}

/// Evaluate the generated netlist: parse each `assign` line and
/// compute outputs from the given cell inputs (keyed by source
/// pointer position, mapped through the span minimum the netlist
/// was generated from).
fn evaluate(
    netlist: &str,
    min_pointer: i64,
    inputs: &BTreeMap<i64, u16>,
) -> (BTreeMap<i64, u16>, Vec<u16>) {
    let input_of = |index: usize| -> u16 {
        let position = min_pointer + index as i64;
        inputs.get(&position).copied().unwrap_or(0)
    };
    let mut cells: BTreeMap<i64, u16> = BTreeMap::new();
    let mut prints: Vec<(usize, u16)> = Vec::new();
    for line in netlist.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("assign ") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(';') else {
            continue;
        };
        let (target, expr) = rest.split_once(" = ").unwrap();
        let (src, konst) = expr.split_once(" + ").unwrap();
        let src_index: usize = src.strip_prefix("cell_in_").unwrap().parse().unwrap();
        let konst: u32 = konst.strip_prefix("16'd").unwrap().parse().unwrap();
        let value = (input_of(src_index) as u32 + konst) as u16;
        if let Some(i) = target.strip_prefix("cell_out_") {
            let i: usize = i.parse().unwrap();
            cells.insert(min_pointer + i as i64, value);
        } else if let Some(i) = target.strip_prefix("print_") {
            let i: usize = i.parse().unwrap();
            prints.push((i, value));
        } else {
            panic!("unknown assign target {target}");
        }
    }
    prints.sort_by_key(|(i, _)| *i);
    (cells, prints.into_iter().map(|(_, v)| v).collect())
}

fn check(source: &str, inputs: &BTreeMap<i64, u16>) {
    let program = magma_spatial::parse(source).unwrap();
    let netlist = magma_spatial::compile(source).unwrap();
    let (want_cells, want_prints) = interpret(source, inputs);
    let (got_cells, got_prints) = evaluate(&netlist, program.min_pointer, inputs);
    assert_eq!(want_cells, got_cells, "cells differ for {source:?}");
    assert_eq!(want_prints, got_prints, "prints differ for {source:?}");
}

/// Small deterministic PRNG (xorshift64) so the randomized cases
/// are reproducible without external crates.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn shipped_example_matches() {
    let mut inputs = BTreeMap::new();
    inputs.insert(0, 100);
    inputs.insert(1, 65535);
    inputs.insert(2, 7);
    check("+++>+++++>--.<.<.", &inputs);
}

#[test]
fn randomized_straight_line_programs_match() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for case in 0..300 {
        // Random program over the straight-line alphabet, pointer
        // kept within +-6 by construction.
        let len = 1 + rng.below(60);
        let mut source = String::new();
        let mut pointer: i64 = 0;
        for _ in 0..len {
            let choice = rng.below(10);
            match choice {
                0..=2 => source.push('+'),
                3..=4 => source.push('-'),
                5 => source.push('.'),
                6..=7 => {
                    if pointer < 6 {
                        source.push('>');
                        pointer += 1;
                    } else {
                        source.push('+');
                    }
                }
                _ => {
                    if pointer > -6 {
                        source.push('<');
                        pointer -= 1;
                    } else {
                        source.push('-');
                    }
                }
            }
        }
        // Random inputs across the whole reachable span.
        let mut inputs = BTreeMap::new();
        for position in -6..=6 {
            inputs.insert(position, rng.next() as u16);
        }
        check(&source, &inputs);
        let _ = case;
    }
}

#[test]
fn long_delta_runs_wrap_like_the_interpreter() {
    let mut inputs = BTreeMap::new();
    inputs.insert(0, 3);
    check(&"-".repeat(70000), &inputs);
    check(&"+".repeat(131073), &inputs);
}
