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

## Front end — Piet 2D spatial, WASM, WebSockets

The front end is a 2D spatial surface in the spirit of Piet: the
program is a field of colored blocks, read along a continuous
snake path (row 1 left-to-right, row 2 right-to-left, alternating).
Each color is one Brainfuck command — the palette lives in
`src/piet.rs` and is the single source of truth. This is a spatial
notation for MAGMA programs, not the Piet language itself (no
direction pointer or stack).

```sh
sh web/build-wasm.sh                    # builds web/magma.wasm
cargo run --release --bin magma-server  # serves web/ on 127.0.0.1:8765
```

Open the page and paint. Three views of the one program, all
computed by the library itself: the grid (the program as a
picture); a Turing machine (`src/turing.rs`, full Brainfuck —
tape, head, step/run transport, output tray) that *runs* the
program, including the loops the spatial stage cannot compile
yet; and the MAGMA datapath drawn as a live diagram whose outputs
compute from the input values you set. No code blocks are shown:
the netlist is a download, not a panel. Compilation runs locally in WebAssembly
(the library compiled to `web/magma.wasm`, hand-rolled C ABI in
`src/wasm.rs`, no bindgen); a WebSocket channel (RFC 6455,
hand-rolled on std only in `src/ws.rs` / `src/server.rs`) shares
the grid, its Brainfuck stream, and its compile report with every
connected browser, so several people can watch and edit the same
spatial program live. Either engine works alone: no server means
WASM-only, no WASM means the server compiles.

Checks: `cargo test` includes a live server end-to-end test
(`tests/server_e2e.rs`); `node web/test-wasm.mjs` verifies the
WASM module's output against the native compiler.

## Next integration stages

1. Represent SUBLEQ signed arithmetic and branch selection as hardware control/dataflow nodes.
2. Add stateful feedback nodes for Brainfuck loops and SUBLEQ branches.
3. Lower occam channel rendezvous to ready/valid hardware handshakes.
4. Add fixed-point phase registers and Bloch projection datapaths.
5. Use an open synthesis flow such as Yosys for FPGA-ready netlists.
