# MAGMA Spatial Compiler

An executable Rust prototype that compiles statically-addressed Brainfuck into parallel, combinational Verilog lanes. No C, Python, JavaScript, runtime interpreter, or program counter in generated RTL.

## Build

```sh
cargo build --release
cargo run -- compile examples/parallel.bf parallel.v
```

`parallel.v` exposes one 16-bit input and output for each tape cell, plus `print_N` output taps for each `.` operation. Every lane can evaluate concurrently. The source pointer is resolved at compile time; negative tape positions are normalized to nonnegative port indices.

## Current semantics

- `>` and `<` move the compile-time tape address.
- `+` and `-` perform wrapping 16-bit additions/subtractions.
- `.` creates a 16-bit output tap at that point in source execution (not an ASCII stream).
- `[` and `]` are rejected: data-dependent loops require sequential feedback/state or proven bounded unrolling.
- `,` is rejected until streaming inputs and their temporal semantics are implemented.
- Input tape cells are externally supplied; output tape cells are the resulting values.
- Unreferenced and unused intermediate source pointer steps are eliminated.
- Different cell lanes are independent; writes to the same cell are combined while output taps preserve intermediate values.

This is **not** a complete Brainfuck implementation, SUBLEQ compiler, occam compiler, FPGA bitstream generator, or quantum projector. It is the first working spatial lowering stage for MAGMA. Finite combinational logic cannot directly realize arbitrary unbounded Turing-machine execution.

## Tests

```sh
cargo test
sh tests/smoke.sh
```

`cargo test` runs the unit tests in each module, the CLI contract
tests in `tests/cli.rs` (exit codes and diagnostics), and the
differential test in `tests/differential.rs`, which evaluates the
generated Verilog as arithmetic against a reference straight-line
Brainfuck interpreter on the shipped example and on randomized
programs. `tests/smoke.sh` is the original end-to-end check.

## Layout

- `src/parse.rs` — static pointer resolution and source scanning.
- `src/lower.rs` — per-cell write combination and output taps.
- `src/codegen.rs` — Verilog emission.
- `src/error.rs` — compile diagnostics.
- `src/main.rs` — the `compile` command-line interface.

## Next integration stages

1. Represent SUBLEQ signed arithmetic and branch selection as hardware control/dataflow nodes.
2. Add stateful feedback nodes for Brainfuck loops and SUBLEQ branches.
3. Lower occam channel rendezvous to ready/valid hardware handshakes.
4. Add fixed-point phase registers and Bloch projection datapaths.
5. Use an open synthesis flow such as Yosys for FPGA-ready netlists.
