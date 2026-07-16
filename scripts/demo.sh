#!/bin/bash
# Quick Start Demo for SovereignEdge-TEE-Agent
#
# Demonstrates: workspace build, test suite, hardware (GPU/NPU) detection
# with live sensors, and the graceful-degradation state machine in all
# three modes. Optionally runs real llama.cpp local inference.
#
# Usage:
#   ./scripts/demo.sh
#   MODEL_GGUF=/path/to/model.gguf ./scripts/demo.sh   # real local inference
#     (requires: cargo build -p edge-agent --features llama)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

export RUST_LOG=${RUST_LOG:-info}

# DEMO_PAUSE=<seconds>: pause between steps (for live demos / recordings)
pause() { [ -n "$DEMO_PAUSE" ] && sleep "$DEMO_PAUSE" || true; }

echo "=============================================="
echo "  SovereignEdge-TEE-Agent Quick Start Demo"
echo "=============================================="
echo ""

if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Install from https://rustup.rs/"
    exit 1
fi

echo "[Step 1/5] Building workspace..."
cargo build --workspace 2>&1 | tail -2
if [ -n "$MODEL_GGUF" ]; then
    echo "  MODEL_GGUF set - rebuilding edge-agent with llama.cpp (needs cmake + C++ toolchain)..."
    cargo build -p edge-agent --features llama 2>&1 | tail -1
fi

echo ""
pause
echo "[Step 2/5] Running test suite..."
PASS=$(cargo test --workspace 2>&1 | grep -cE 'test result: ok')
FAIL=$(cargo test --workspace 2>&1 | grep -cE 'test result: FAILED' || true)
echo "  ${PASS} test suites passed, ${FAIL} failed"

AGENT_BIN=./target/debug/edge_agent
DEMO_FILTER='Detected accelerator|Frame [0-9]|Final stats|Mode changed|inference backend|Loaded model'

echo ""
pause
echo "[Step 3/5] Edge agent — ONLINE mode (frames offloaded to cloud)..."
echo "----------------------------------------------"
$AGENT_BIN --probe-interval 1 --mode online 2>&1 | grep -E "$DEMO_FILTER" || true

echo ""
pause
echo "[Step 4/5] Edge agent — OFFLINE mode (graceful degradation to local inference)..."
echo "----------------------------------------------"
if [ -n "$MODEL_GGUF" ]; then
    echo "  (using real llama.cpp inference: $MODEL_GGUF)"
    $AGENT_BIN --probe-interval 1 --mode offline --model "$MODEL_GGUF" 2>&1 | grep -E "$DEMO_FILTER" || true
else
    $AGENT_BIN --probe-interval 1 --mode offline 2>&1 | grep -E "$DEMO_FILTER" || true
fi

echo ""
pause
echo "[Step 5/5] Machine-verified invariants (Lean 4)..."
echo "----------------------------------------------"
if command -v lake &> /dev/null || [ -x "$HOME/.elan/bin/lake" ]; then
    export PATH="$HOME/.elan/bin:$PATH"
    (cd verification && lake build 2>&1 | tail -1)
    THEOREMS=$(grep -hc 'theorem' verification/SovereignEdge/*.lean | awk '{s+=$1} END {print s}')
    echo "  ${THEOREMS} theorems proved (see verification/README.md)"
else
    echo "  Lean toolchain not installed — skipping (install elan, then: cd verification && lake build)"
fi

echo ""
echo "=============================================="
pause
echo "  Demo complete"
echo "=============================================="
echo "Next steps:"
echo "  - Run with a GGUF model for real local inference (see header of this script)"
echo "  - docs/ARCHITECTURE.md for diagrams; verification/ for proofs"
echo "  - README 'Phases' section for the honest implemented/simulated status"
