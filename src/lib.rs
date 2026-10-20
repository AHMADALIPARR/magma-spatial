// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! MAGMA spatial compiler library: statically-addressed Brainfuck
//! lowered to a combinational Verilog datapath. There is no
//! program counter in the synthesized design. Logical cells are
//! 16-bit wrapping integers; source code uses BF syntax.
//!
//! Pipeline: [`parse`] resolves the tape pointer at compile time,
//! [`lower`] combines writes per cell, [`codegen::emit`] writes
//! the Verilog lanes.

pub mod codegen;
pub mod error;
pub mod lower;
pub mod parse;

pub use error::CompileError;
pub use lower::{lower, Lowered};
pub use parse::{parse, Program};

/// Compile Brainfuck source to a Verilog netlist in one call.
pub fn compile(source: &str) -> Result<String, CompileError> {
    let program = parse(source)?;
    let lowered = lower(&program)?;
    Ok(codegen::emit(&lowered))
}
