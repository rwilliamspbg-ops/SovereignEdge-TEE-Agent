# SovereignEdge-TEE-Agent

A Hardware-Secured, Post-Quantum Edge Agent Infrastructure with Zero-Copy Remote Offloading.

## Overview

SovereignEdge-TEE-Agent implements a complete edge-to-cloud pipeline featuring:

- **AF_XDP/eBPF kernel-bypass ingestion** for line-rate telemetry processing
- **Hybrid Post-Quantum cryptography** (X25519 + ML-KEM-768/FIPS 203) resistant to harvest-now-decrypt-later attacks
- **AES-256-GCM encrypted frames** with machine-verified nonce discipline
- **Graceful degradation** with automatic edge/cloud failover
- **Trusted Execution Environment (TEE)** gateway for confidential Qwen Cloud integration
- **Zero-Knowledge proofs** (arkworks Groth16 on BN254) for verifiable safety policy compliance

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              EDGE NODE                                       │
│  ┌──────────────────┐    ┌──────────────────┐    ┌─────────────────────┐   │
│  │  AF_XDP Ingest   │───▶│  PQC Transport   │───▶│  Edge Agent         │   │
│  │  (eBPF/XDP)      │    │  (X25519+MLKEM)  │    │  (Local Inference)  │   │
│  └──────────────────┘    └──────────────────┘    └─────────────────────┘   │
│                                                              │              │
│                            Network Quality Monitor           │              │
│                            Online/Degraded/Offline Modes     │              │
└──────────────────────────────────────────────────────────────│──────────────┘
                                                               │
                              Encrypted UDP (Port 47821)       │
                                                               ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ALIBABA CLOUD TEE                                    │
│  ┌──────────────────┐    ┌──────────────────┐    ┌─────────────────────┐   │
│  │  TEE Gateway     │───▶│  Qwen Cloud API  │───▶│  ZK Proof Generator │   │
│  │  (Sealed Storage)│    │  (qwen-max)      │    │  (Groth16/BN254)    │   │
│  └──────────────────┘    └──────────────────┘    └─────────────────────┘   │
│                                                                      │      │
└──────────────────────────────────────────────────────────────────────│──────┘
                                                                       │
                                                        Verifiable Execution Log
                                                                       ▼
                                                                Downstream Observers
```

## Project Structure

```
sovereign-edge-tee-agent/
├── crates/
│   ├── common/              # Shared types: frames, network quality, context buffer
│   ├── helpers/             # Time, builders, fixtures, metrics utilities
│   ├── xdp-ingest/          # Phase 1: AF_XDP daemon (bpf/xdp_prog.c = eBPF program)
│   ├── pqc-transport/       # Phase 1: X25519 + ML-KEM-768 hybrid KEX, AES-256-GCM
│   ├── edge-agent/          # Phase 2: mode state machine, hardware detection,
│   │                        #          llama.cpp local inference (feature "llama")
│   ├── tee-gateway/         # Phase 3: TEE trait abstraction + Qwen API (reqwest)
│   └── zk-proofs/           # Phase 4: arkworks Groth16 + policy constraints
├── verification/            # Lean 4 machine-checked proofs of core invariants
├── configs/                 # Configuration files
├── evidence/                # Alibaba Cloud deployment runbook
├── docs/                    # Documentation
├── scripts/                 # Build and demo scripts
└── tests/                   # Integration tests
```

## Quick Start

### Prerequisites

- **Rust 1.85+** (stable MSVC toolchain on Windows; stable on Linux)
- **Linux kernel 5.4+** with eBPF support (required for `xdp-ingest`)
- **clang** with BPF target (for eBPF compilation)
- **MSVC Build Tools** (on Windows)

### Building

```bash
git clone https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent.git
cd SovereignEdge-TEE-Agent

# Build all crates (excludes xdp-ingest on non-Linux)
cargo build --release

# Build with local llama.cpp inference
cargo build -p edge-agent --features llama --release

# Run all tests
cargo test --workspace
```

### Running the Edge Agent

```bash
# Start AF_XDP ingestion daemon (Linux only, requires root)
sudo ./target/release/af_xdp_daemon --iface eth0 --port 47821

# Configure edge agent mode
export AGENT_MODE=online  # or 'degraded' or 'offline'
```

### Hardware Detection & Local Inference

The edge agent detects local GPUs (DRM sysfs / NVIDIA proc) and NPUs
(Linux `accel` subsystem: AMD XDNA/Ryzen AI, Intel NPU, Qualcomm,
Rockchip, Hailo) at startup.

Real inference with llama.cpp requires the `llama` feature:

```bash
cargo build -p edge-agent --features llama
RUST_LOG=info ./target/debug/edge_agent --mode offline \
    --model path/to/model.gguf --gpu-layers 0
```

Without `--model`, the agent falls back to the simulated backend.

### Deploying TEE Gateway on Alibaba Cloud

Deployment has **not been performed yet**. A step-by-step runbook is in
`evidence/alibaba_cloud_setup.md`.

## Machine-Verified Invariants (Lean 4)

Core invariants are formally proved in [`verification/`](verification/)
(28 theorems, zero `sorry`; standard axioms only):

- **Mode state machine** — offline thresholds nest inside degraded ones;
  `determine_mode` is exactly characterized and monotone.
- **Context buffer** — byte and frame caps hold after every `push`.
- **AES-GCM nonces** — no (key, nonce) reuse; counter hard-stops at 2^64.
- **Policy evaluator** — sound and complete against declarative semantics.

Build with `cd verification && lake build`. See `verification/README.md`.

## Component Status

Legend: ✅ implemented & tested · 🔧 trait abstraction, simulated backend · 🔄 planned

### Phase 1: Transport & Ingestion
- ✅ eBPF XDP program (`bpf/xdp_prog.c`)
- 🔧 AF_XDP socket binding — daemon runs with simulated reception (Linux-only for real binding)
- ✅ Hybrid PQC key exchange — **both X25519 (`x25519-dalek`) and ML-KEM-768 (`ml-kem`/RustCrypto) are real**
- ✅ AES-256-GCM encrypted frames with verified nonce discipline

### Phase 2: Edge Intelligence
- ✅ Local inference — GGUF via llama.cpp (`--features llama`), simulated fallback
- ✅ GPU/NPU hardware detection with live sensors
- ✅ Rolling context buffer (bounds machine-verified)
- 🔧 Network quality monitoring — probe loop present, RTT measurement simulated
- ✅ Automatic online/degraded/offmode transitions (machine-verified)

### Phase 3: Confidential Cloud Backend
- 🔧 TEE gateway — `TeeBackend` trait with `SimulatedTee` default; SGX/SEV-SNP/Alibaba backends pluggable
- ✅ Qwen Cloud API — **real HTTP via `reqwest`** with JSON body, retry logic
- ✅ Structured prompt management
- ✅ Session caching and statistics

### Phase 4: Verification & ZK-Proofs
- ✅ Safety policy constraint system (evaluator machine-verified)
- ✅ ZK-SNARK proof generation — **arkworks Groth16 on BN254** wired in with R1CS circuit
- ✅ Execution trace logging
- ✅ Verifiable output export

### Phase 5: Polish & Deployment
- ✅ Repository cleanup and documentation
- ✅ Formal verification package
- ✅ Architecture diagrams
- ✅ Scripted demo with transcript
- 🔄 Alibaba Cloud deployment (runbook ready)

## Performance Benchmarks

Measured on Windows MSVC (release, LTO). Linux with AVX2 will be ~2-3x faster.

| Operation | Time |
|-----------|------|
| ML-KEM-768 keygen | 54 µs |
| Hybrid KEX roundtrip (keygen + encapsulate + decapsulate) | 286 µs |
| AES-256-GCM encrypt+decrypt (1 KB) | 1.9 µs |
| ZK proof generation (Groth16 setup + prove) | 7.6 ms |

## Configuration

### Edge Agent Settings

| Parameter | Default | Description |
|-----------|---------|-------------|
| `EDGE_TELEMETRY_UDP_PORT` | 47821 | UDP port for telemetry frames |
| `PROBE_INTERVAL_SECS` | 5 | Network quality probe interval |
| `SESSION_TIMEOUT_SECS` | 300 | PQC session timeout |
| `MAX_CONTEXT_FRAMES` | 100 | Maximum buffered frames |
| `LATENCY_THRESHOLD_MS` | 200 | Degraded mode trigger |

### TEE Gateway Settings

| Parameter | Default | Description |
|-----------|---------|-------------|
| `QWEN_API_ENDPOINT` | https://dashscope.aliyuncs.com | Qwen Cloud API URL |
| `SEALED_STORAGE_PATH` | /var/lib/tee/sealed | Sealed token storage |
| `ATTESTATION_PROVIDER` | alibaba-cas | Remote attestation service |

## Security

The following components are **production-ready**:
- X25519 key exchange (`x25519-dalek`) — real forward secrecy
- ML-KEM-768 key exchange (`ml-kem`/RustCrypto, FIPS 203) — post-quantum security
- AES-256-GCM encryption with monotonic nonce (Lean-verified)
- Qwen Cloud API via reqwest with rustls-tls
- Groth16 ZK proofs on BN254 (arkworks)

The following remain **simulated** (trait interfaces ready for real backends):
- TEE sealing/attestation — `TeeBackend` trait; SGX/SEV-SNP implementations pending
- AF_XDP socket binding — Linux-only, requires `aya` integration

> **Note**: Run `cargo audit` before deploying. See [CHANGELOG.md](CHANGELOG.md) for dependency updates.

## License

MIT License - See [LICENSE](LICENSE) file for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Branch naming: `feature/`, `fix/`, `docs/`.
All PRs require passing `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt`.

## References

- [ML-KEM-768 (FIPS 203)](https://csrc.nist.gov/pubs/fips/203/final)
- [RustCrypto ML-KEM](https://github.com/RustCrypto/KEMs/tree/master/ml-kem)
- [arkworks Groth16](https://github.com/arkworks-rs/groth16)
- [Alibaba Cloud TEE](https://www.alibabacloud.com/help/en/confidential-computing)
- [eBPF and XDP](https://ebpf.io/)

---

*Built for the Global AI Hackathon Series with Qwen Cloud*
