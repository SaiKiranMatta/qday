#!/usr/bin/env bash
# Runs every check a reviewer would want, from the repository root: formatting and lints, the Rust
# tests (signature scheme, device logic, screens, CLI), the contract tests on real signatures, and the
# web wallet's typecheck and tests. Stops at the first failure.
#
# The contract tests verify signatures made by the Rust scheme (contracts/test/fixtures), and the web
# tests check the browser's WebAssembly codec against the CLI byte for byte, so passing here means the
# device, the chain and the browser agree. Needs cargo, forge and pnpm; the firmware is built separately.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$1"; }

step "Rust: format and lints"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings

step "Rust: tests"
cargo test --workspace --locked

step "Contracts: tests"
(cd contracts && forge test)

step "Web: install, typecheck and tests"
pnpm install --frozen-lockfile
pnpm -r typecheck
pnpm -r test

printf '\nAll checks passed.\n'
