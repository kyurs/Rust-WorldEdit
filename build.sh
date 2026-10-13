#!/bin/sh
set -eu
cd "$(dirname "$0")"
cargo test --locked
cargo clippy --locked --target wasm32-wasip2 -- -D warnings
cargo build --locked --release --target wasm32-wasip2
mkdir -p dist
cp target/wasm32-wasip2/release/pumpkin_worldedit.wasm dist/worldedit.wasm
