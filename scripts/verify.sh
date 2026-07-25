#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname -- "${BASH_SOURCE[0]}")/.."

export RUST_TEST_THREADS=1

cargo fmt --all -- --check
cargo metadata --locked --no-deps --format-version 1 >/dev/null
cargo test --all-targets
cargo test --all-targets --features research
cargo test --doc --no-default-features
cargo test --doc --features research
cargo clippy --all-targets --all-features -- -D warnings
cargo check --target i686-unknown-linux-gnu --no-default-features --lib
cargo check --target i686-unknown-linux-gnu --features research --lib
RUSTDOCFLAGS="-D warnings" CARGO_TARGET_DIR=target/rustdoc cargo doc --all-features --no-deps
sh proofs/check-cost-model-laws.sh
cargo package --allow-dirty
