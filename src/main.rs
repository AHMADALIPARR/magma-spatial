// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

use std::{env, fs, process};

// MAGMA MVP: compile statically-addressed Brainfuck into a combinational
// Verilog datapath. There is no program counter in the synthesized design.
// Logical cells are 16-bit wrapping integers; source code uses BF syntax.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 || args[1] != "compile" {
        eprintln!("usage: magma-spatial compile <source.bf> <output.v>");
        process::exit(2);
    }
    let source = fs::read_to_string(&args[2]).unwrap_or_else(|e| {
        eprintln!("cannot read source: {e}");
        process::exit(1)
    });
    let program = magma_spatial::parse(&source).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1)
    });
    let lowered = magma_spatial::lower(&program).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1)
    });
    let v = magma_spatial::codegen::emit(&lowered);
    fs::write(&args[3], v).unwrap_or_else(|e| {
        eprintln!("cannot write netlist: {e}");
        process::exit(1)
    });
    println!(
        "compiled: {} cells, {} writes, {} output taps, no instruction pointer",
        lowered.deltas.len(),
        lowered.writes,
        lowered.output_taps.len()
    );
}
