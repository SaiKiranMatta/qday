#!/usr/bin/env bash
# Rebuilds the browser copy of the device's frame code (crates/qday-wasm) into pkg/.
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.130.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p qday-wasm --target wasm32-unknown-unknown
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
wasm-bindgen --target web --out-dir packages/frame/pkg "$TARGET/wasm32-unknown-unknown/release/qday_wasm.wasm"
echo "packages/frame/pkg: $(du -sh packages/frame/pkg | cut -f1)"
