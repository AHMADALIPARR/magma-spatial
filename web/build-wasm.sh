#!/bin/sh
# Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
# Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
# SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque
# Build the WebAssembly front-end module into web/magma.wasm.
# Uses the workspace Rust toolchain (system cargo works too, as
# long as the wasm32-unknown-unknown target is installed).
set -eu
cd "$(dirname "$0")/.."
if [ -x "$HOME/workspace/.tools/cargo/bin/cargo" ]; then
  CARGO="$HOME/workspace/.tools/cargo/bin/cargo"
  export RUSTUP_HOME="$HOME/workspace/.tools/rustup"
  export CARGO_HOME="$HOME/workspace/.tools/cargo"
else
  CARGO=cargo
fi
"$CARGO" build --release --target wasm32-unknown-unknown --lib
cp target/wasm32-unknown-unknown/release/magma_spatial.wasm web/magma.wasm
echo "web/magma.wasm built"
