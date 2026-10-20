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
pub mod json;
pub mod lower;
pub mod parse;
pub mod piet;
pub mod server;
#[cfg(target_arch = "wasm32")]
pub mod wasm;
pub mod ws;

pub use error::CompileError;
pub use lower::{lower, Lowered};
pub use parse::{parse, Program};

/// Compile Brainfuck source to a Verilog netlist in one call.
pub fn compile(source: &str) -> Result<String, CompileError> {
    let program = parse(source)?;
    let lowered = lower(&program)?;
    Ok(codegen::emit(&lowered))
}

/// Compile Brainfuck source and report the outcome as a flat JSON
/// object — the wire format shared by the WebAssembly exports and
/// the WebSocket server: `{"ok":true,"bf":...,"cells":N,
/// "writes":N,"taps":N,"verilog":...}` on success,
/// `{"ok":false,"error":...}` on a compile error.
pub fn compile_report(source: &str) -> String {
    match parse(source).and_then(|program| lower(&program)) {
        Ok(lowered) => format!(
            "{{\"ok\":true,\"bf\":\"{}\",\"cells\":{},\"writes\":{},\"taps\":{},\"verilog\":\"{}\"}}",
            json::escape(source),
            lowered.deltas.len(),
            lowered.writes,
            lowered.output_taps.len(),
            json::escape(&codegen::emit(&lowered))
        ),
        Err(e) => format!(
            "{{\"ok\":false,\"error\":\"{}\"}}",
            json::escape(&e.to_string())
        ),
    }
}
