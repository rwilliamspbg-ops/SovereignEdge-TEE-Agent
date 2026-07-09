# Justfile for SovereignEdge-TEE-Agent
# Install just from https://github.com/casey/just

# Default target - show help
default:
    @just --list

# Build the entire workspace
build:
    @echo "Building workspace..."
    cargo build --workspace --release

# Build in debug mode
build-debug:
    @echo "Building workspace (debug)..."
    cargo build --workspace

# Run all tests
test:
    @echo "Running tests..."
    cargo test --workspace --verbose

# Run tests with output
test-output:
    @echo "Running tests with output..."
    cargo test --workspace -- --nocapture

# Check code without building
check:
    @echo "Checking workspace..."
    cargo check --workspace

# Format all code
fmt:
    @echo "Formatting code..."
    cargo fmt --all

# Check formatting
fmt-check:
    @echo "Checking formatting..."
    cargo fmt --all -- --check

# Run clippy lints
lint:
    @echo "Running clippy..."
    cargo clippy --workspace --all-targets -- -D warnings

# Clean build artifacts
clean:
    @echo "Cleaning build artifacts..."
    cargo clean

# Run edge agent demo
demo-agent:
    @echo "Running edge agent demo..."
    cargo run --bin edge_agent -- --probe-interval 2

# Run XDP daemon (requires sudo and eBPF support)
demo-xdp:
    @echo "Running AF_XDP daemon (simulated)..."
    cargo run --bin af_xdp_daemon -- --iface lo --port 47821

# Generate sample configuration
config-gen:
    @echo "Generating sample configuration..."
    cp configs/default.toml configs/local.toml
    @echo "Created configs/local.toml - edit for local customization"

# Run integration tests
test-integration:
    @echo "Running integration tests..."
    cargo test --test '*' --workspace

# Benchmark (requires nightly)
bench:
    @echo "Running benchmarks..."
    cargo bench --workspace

# Document all crates
docs:
    @echo "Generating documentation..."
    cargo doc --workspace --no-deps
    @echo "Documentation available at target/doc/index.html"

# Open documentation in browser
docs-open: docs
    @echo "Opening documentation..."
    xdg-open target/doc/index.html 2>/dev/null || open target/doc/index.html 2>/dev/null || echo "Open target/doc/index.html manually"

# Security audit
audit:
    @echo "Running security audit..."
    cargo audit || echo "cargo-audit not installed. Install with: cargo install cargo-audit"

# Update dependencies
update:
    @echo "Updating dependencies..."
    cargo update

# Show dependency tree
deps-tree:
    @echo "Dependency tree:"
    cargo tree --depth 2

# Quick start - build and run basic demo
quickstart: build-debug demo-agent

# Development cycle - check, fmt, lint, test
dev-cycle: check fmt lint test
    @echo "Development cycle complete!"

# Create release build with all optimizations
release:
    @echo "Creating optimized release build..."
    cargo build --workspace --release --locked
    @echo "Binaries in target/release/"

# Help command
help:
    @echo "SovereignEdge-TEE-Agent Development Commands"
    @echo ""
    @echo "Common commands:"
    @echo "  just              - Show this help"
    @echo "  just build        - Build workspace in release mode"
    @echo "  just test         - Run all tests"
    @echo "  just lint         - Run clippy lints"
    @echo "  just fmt          - Format all code"
    @echo "  just demo-agent   - Run edge agent demo"
    @echo "  just quickstart   - Build and run demo"
    @echo "  just dev-cycle    - Full development cycle"
    @echo ""
    @echo "Documentation:"
    @echo "  just docs         - Generate documentation"
    @echo "  just docs-open    - Generate and open docs"
    @echo ""
    @echo "Maintenance:"
    @echo "  just clean        - Remove build artifacts"
    @echo "  just audit        - Security audit"
    @echo "  just update       - Update dependencies"
