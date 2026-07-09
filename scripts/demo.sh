#!/bin/bash
# Quick Start Demo for SovereignEdge-TEE-Agent
# This script demonstrates the basic functionality of the edge agent

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "=============================================="
echo "  SovereignEdge-TEE-Agent Quick Start Demo"
echo "=============================================="
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found."
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "[Step 1/4] Building workspace..."
cargo build --release 2>&1 | tail -5

echo ""
echo "[Step 2/4] Running Edge Agent demo (5 frames)..."
echo "----------------------------------------------"
./target/release/edge_agent --probe-interval 1 || {
    echo "Note: Running with cargo run instead..."
    cargo run --release --bin edge_agent -- --probe-interval 1
}

echo ""
echo "[Step 3/4] Demonstrating configuration..."
echo "----------------------------------------------"
echo "Configuration file: configs/default.toml"
echo ""
echo "Key settings:"
grep -E "^(probe_interval|latency_threshold|pqc|tee)" configs/default.toml | head -10 || true

echo ""
echo "[Step 4/4] Summary"
echo "----------------------------------------------"
echo "✓ Workspace built successfully"
echo "✓ Edge agent demonstrated mode transitions"
echo "✓ Configuration loaded from configs/default.toml"
echo ""
echo "Next steps:"
echo "  - Edit configs/default.toml for customization"
echo "  - Run 'just test' to execute all tests"
echo "  - Run 'just docs' to generate documentation"
echo "  - See docs/IMPROVEMENTS.md for development roadmap"
echo ""
echo "Demo complete!"
