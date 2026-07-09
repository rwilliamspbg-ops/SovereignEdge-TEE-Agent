# Contributing to SovereignEdge-TEE-Agent

Thank you for considering contributing! This document provides guidelines for contributions.

## Development Setup

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Linux kernel 5.4+ with eBPF support (for XDP features)
- clang with BPF target support (for eBPF programs)
- libbpf development headers

### Building

```bash
# Clone the repository
git clone https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent.git
cd SovereignEdge-TEE-Agent

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Check code quality
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Code Style

- Follow standard Rust formatting (`cargo fmt`)
- All code must pass clippy lints without warnings
- Use descriptive variable and function names
- Document public APIs with rustdoc comments
- Include unit tests for new functionality

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes with tests
4. Ensure all tests pass (`cargo test --workspace`)
5. Run clippy and fix any warnings
6. Commit with clear, descriptive messages
7. Open a pull request

## Testing Guidelines

- Write unit tests for all public functions
- Include integration tests for cross-module functionality
- Test error conditions and edge cases
- Aim for >80% code coverage on new code

## Architecture Overview

The project is organized as a Cargo workspace with these crates:

- **common**: Shared types and utilities
- **xdp-ingest**: AF_XDP/eBPF packet ingestion
- **pqc-transport**: Post-quantum cryptographic transport
- **edge-agent**: Edge intelligence and mode management
- **tee-gateway**: TEE-secured cloud gateway
- **zk-proofs**: Zero-knowledge proof generation

## Security Considerations

When contributing security-sensitive code:

1. Never commit secrets, tokens, or keys
2. Use constant-time comparisons for cryptographic operations
3. Validate all inputs from untrusted sources
4. Follow the principle of least privilege
5. Document any security assumptions

## Questions?

Open an issue for questions or discussions about contributions.
