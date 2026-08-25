#!/usr/bin/env bash
# Build the simulator to WASM and copy the artifacts into the frontend (app2/).
set -euo pipefail
# Use the absolute path of this script's directory; relative "." resolves
# differently under sandboxes/shims and breaks the toolchain fallback below.
cd "$(cd "$(dirname "$0")" && pwd -P)"

# $PWD is sim/ after the cd above; the toolchain lives at <workspace>/.rust
CARGO_HOME="${CARGO_HOME:-$PWD/../.rust/cargo}"
RUSTUP_HOME="${RUSTUP_HOME:-$PWD/../.rust/rustup}"
export PATH="$CARGO_HOME/bin:$PATH"

echo "==> Cleaning stale wasm artifacts (avoids silent cache hits)..."
cargo clean --target wasm32-unknown-unknown

echo "==> Building sim (release, wasm32-unknown-unknown)..."
cargo build --release --target wasm32-unknown-unknown

WASM_OUT="../web/src/sim"
mkdir -p "$WASM_OUT"

echo "==> Generating wasm-bindgen bindings..."
wasm-bindgen --target web --out-dir "$WASM_OUT" \
  target/wasm32-unknown-unknown/release/sim.wasm

# wasm-bindgen produces sim.js + sim_bg.wasm; ensure the .wasm is present.
ls -la "$WASM_OUT"
echo "==> Done. Artifacts in $WASM_OUT"
