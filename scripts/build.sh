#!/bin/bash
# Build script for SovereignEdge-TEE-Agent
# Builds all workspace crates and eBPF programs

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "=== SovereignEdge Build Script ==="
echo ""

# Check for Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Build eBPF programs (requires clang)
echo "[1/3] Building eBPF XDP programs..."
if command -v clang &> /dev/null; then
    mkdir -p target/bpfel-unknown-none/release
    clang -O2 -target bpf -c src/xdp_ingest/xdp_prog.c -o target/bpfel-unknown-none/release/xdp_prog.o 2>/dev/null || {
        echo "  Warning: eBPF build skipped (clang may need BPF target support)"
    }
else
    echo "  Warning: clang not found, skipping eBPF build"
fi

# Build Rust workspace
echo "[2/3] Building Rust workspace..."
cargo build --release

# Show build results
echo "[3/3] Build complete!"
echo ""
echo "Binaries built:"
ls -la target/release/af_xdp_daemon target/release/edge_agent 2>/dev/null || echo "  (binaries will be in target/release/)"
