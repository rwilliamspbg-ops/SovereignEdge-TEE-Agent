# Quick Start Guide

Get up and running with SovereignEdge-TEE-Agent in minutes.

## Prerequisites

- **Rust** (1.75+): Install from [rustup.rs](https://rustup.rs/)
- **just** (optional but recommended): `cargo install just`
- **clang** (for eBPF, optional): `apt install clang` or `brew install llvm`

## Installation

```bash
# Clone the repository
git clone https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent.git
cd SovereignEdge-TEE-Agent
```

## Quick Demo

### Option 1: Using just (recommended)

```bash
# Show all available commands
just

# Build and run demo
just quickstart
```

### Option 2: Using cargo directly

```bash
# Build the workspace
cargo build --release

# Run the edge agent demo
cargo run --release --bin edge_agent -- --probe-interval 2
```

### Option 3: Using the demo script

```bash
./scripts/demo.sh
```

## Expected Output

When you run the edge agent demo, you should see output like:

```
SovereignEdge Edge Agent starting...
Probe interval: 2s, Port: 47821
Frame 1: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
Frame 2: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
...
Final stats: 5 frames, 5 cloud offloads, 0 local inferences, 0 mode transitions
Edge agent shutting down...
```

## Project Structure

```
SovereignEdge-TEE-Agent/
├── crates/
│   ├── common/        # Shared types and utilities
│   ├── xdp-ingest/    # AF_XDP packet ingestion
│   ├── pqc-transport/ # Post-quantum crypto transport
│   ├── edge-agent/    # Edge agent with mode switching
│   ├── tee-gateway/   # TEE gateway for cloud API
│   ├── zk-proofs/     # Zero-knowledge proof system
│   └── helpers/       # Development helpers and fixtures
├── configs/           # Configuration files
├── scripts/           # Build and demo scripts
├── tests/             # Integration tests and fixtures
└── docs/              # Documentation
```

## Next Steps

1. **Explore the code**: Each crate has its own README and documentation
2. **Run tests**: `cargo test --workspace`
3. **Generate docs**: `just docs` then open `target/doc/index.html`
4. **Customize config**: Copy `configs/default.toml` to `configs/local.toml`
5. **Read the improvement plan**: See `docs/IMPROVEMENTS.md`

## Common Commands

| Command | Description |
|---------|-------------|
| `just build` | Build workspace in release mode |
| `just test` | Run all tests |
| `just lint` | Run clippy lints |
| `just fmt` | Format all code |
| `just demo-agent` | Run edge agent demo |
| `just clean` | Remove build artifacts |
| `just docs` | Generate documentation |

## Troubleshooting

### Build fails with "package not found"
Run `cargo update` to fetch all dependencies.

### eBPF build warnings
eBPF compilation requires clang with BPF target support. The demo works without it using simulated components.

### Tests fail
Ensure you have the latest dependencies: `cargo update && cargo build`

## Getting Help

- **Documentation**: `just docs` or see `docs/` directory
- **Issues**: GitHub Issues
- **Code Review**: See `docs/IMPROVEMENTS.md` for known areas for improvement
