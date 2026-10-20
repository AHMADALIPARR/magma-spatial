#!/bin/sh
set -eu
cargo run --quiet -- compile examples/parallel.bf /tmp/magma_test.v
# Initial values: 0,0,0 => final values 3,5,65534.
grep -q "assign cell_out_0 = cell_in_0 + 16'd3;" /tmp/magma_test.v
grep -q "assign cell_out_1 = cell_in_1 + 16'd5;" /tmp/magma_test.v
grep -q "assign cell_out_2 = cell_in_2 + 16'd65534;" /tmp/magma_test.v
grep -q "assign print_0 = cell_in_2 + 16'd65534;" /tmp/magma_test.v
grep -q "assign print_1 = cell_in_1 + 16'd5;" /tmp/magma_test.v
grep -q "assign print_2 = cell_in_0 + 16'd3;" /tmp/magma_test.v
if cargo run --quiet -- compile examples/loops_rejected.bf /tmp/magma_bad.v 2>/dev/null; then
  echo 'FAIL: dynamic loops must be rejected' >&2
  exit 1
fi
printf 'PASS: spatial lane generation, output taps, dynamic loop rejection\n'
