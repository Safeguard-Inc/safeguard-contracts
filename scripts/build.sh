#!/usr/bin/env bash
set -euo pipefail

echo "========================================================"
echo "    Safeguard Soroban Contracts - Build System         "
echo "========================================================"

echo "[1/3] Checking rust target wasm32v1-none..."
rustup target add wasm32v1-none

echo "[2/3] Compiling contracts in release mode..."
cargo build --target wasm32v1-none --release -p safeguard-payments -p safeguard-policy

echo "[3/3] WASM binaries compiled successfully:"
ls -lh target/wasm32v1-none/release/*.wasm

echo "Done! WASM artifacts are ready in target/wasm32v1-none/release/"
