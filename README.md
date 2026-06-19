# SovereignEdge-TEE-Agent

A Hardware-Secured, Post-Quantum Edge Agent Infrastructure with Zero-Copy Remote Offloading.

## Overview

SovereignEdge-TEE-Agent implements a complete edge-to-cloud pipeline featuring:

- **AF_XDP/eBPF kernel-bypass ingestion** for line-rate telemetry processing
- **Hybrid Post-Quantum cryptography** (X25519 + ML-KEM-768) resistant to harvest-now-decrypt-later attacks
- **Graceful degradation** with automatic edge/cloud failover
- **Trusted Execution Environment (TEE)** gateway for confidential Qwen Cloud integration
- **Zero-Knowledge proofs** for verifiable safety policy compliance

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
│  │  (Sealed Storage)│    │  (qwen-max)      │    │  (Policy Verification)│  │
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
├── src/
│   ├── xdp_ingest/          # Phase 1: AF_XDP/eBPF ingestion
│   │   ├── xdp_prog.c       # eBPF XDP program
│   │   └── af_xdp_daemon.rs # Rust user-space binding
│   ├── pqc_transport/       # Phase 1: Post-Quantum security
│   │   └── hybrid_pqc.rs    # X25519 + ML-KEM-768 hybrid KEX
│   ├── edge_agent/          # Phase 2: Edge intelligence
│   │   └── agent.rs         # Local inference & graceful degradation
│   ├── tee_gateway/         # Phase 3: Confidential cloud backend
│   │   └── gateway.rs       # TEE runtime with Qwen API integration
│   └── zk_proofs/           # Phase 4: Verification
│       └── zk_verifier.rs   # ZK-SNARK policy verification
├── configs/                  # Configuration files
├── evidence/                 # Alibaba Cloud deployment proof
├── docs/                     # Documentation
├── scripts/                  # Build and deployment scripts
└── tests/                    # Integration tests
```

## Quick Start

### Prerequisites

- Linux kernel 5.4+ with eBPF support
- Rust 1.70+ with nightly toolchain
- libbpf or aya for eBPF development
- Alibaba Cloud account with TEE-enabled VM

### Building

```bash
# Clone the repository
git clone https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent.git
cd SovereignEdge-TEE-Agent

# Build eBPF programs
cd src/xdp_ingest
clang -O2 -target bpf -c xdp_prog.c -o xdp_prog.o

# Build Rust components
cargo build --release
```

### Running the Edge Agent

```bash
# Start AF_XDP ingestion daemon
sudo ./target/release/af_xdp_daemon --iface eth0 --port 47821

# Configure edge agent mode
export AGENT_MODE=online  # or 'degraded' or 'offline'
```

### Deploying TEE Gateway on Alibaba Cloud

1. Provision a confidential VM (SGX/SEV-enabled)
2. Deploy the TEE gateway binary inside the enclave
3. Seal Qwen API tokens using TEE-specific sealing
4. Configure firewall rules for port 47821

See `evidence/alibaba_cloud_setup.md` for detailed deployment instructions.

## Phases

### Phase 1: High-Performance Transport & Core Ingestion
- ✅ eBPF XDP program for kernel-bypass packet filtering
- ✅ AF_XDP socket binding with zero-copy ring buffers
- ✅ Hybrid PQC key exchange (X25519 + ML-KEM-768)
- ✅ AES-256-GCM encrypted data frames

### Phase 2: Edge Intelligence & Orchestration
- ✅ Local inference engine integration
- ✅ Rolling context buffer for situational awareness
- ✅ Network quality monitoring
- ✅ Automatic online/degraded/offline mode transitions

### Phase 3: Confidential Cloud Backend
- ✅ TEE gateway with sealed storage
- ✅ Qwen Cloud API integration
- ✅ Structured prompt management
- ✅ Session caching and statistics

### Phase 4: Verification & ZK-Proofs
- ✅ Safety policy constraint system
- ✅ ZK-SNARK proof generation
- ✅ Execution trace logging
- ✅ Verifiable output export

### Phase 5: Submission & Demo Polish
- 🔄 Repository cleanup and documentation
- 🔄 Alibaba Cloud deployment evidence
- 🔄 Architecture diagrams
- 🔄 Demo video recording

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

## Security Considerations

1. **Post-Quantum Security**: All control messages use hybrid X25519 + ML-KEM-768 key exchange
2. **TEE Isolation**: Qwen API tokens are sealed within the enclave and never exposed to the host
3. **Zero-Knowledge Verification**: Agent actions are cryptographically proven to satisfy safety policies
4. **Harvest-Now-Decrypt-Later Resistance**: Ephemeral symmetric keys derived from hybrid KEX

## License

MIT License - See [LICENSE](LICENSE) file for details.

## References

- [Alibaba Cloud TEE Documentation](https://www.alibabacloud.com/help/en/confidential-computing)
- [Qwen Cloud API Reference](https://help.aliyun.com/zh/dashscope)
- [eBPF and XDP Documentation](https://ebpf.io/)
- [ML-KEM-768 (Kyber) Specification](https://csrc.nist.gov/projects/post-quantum-cryptography)

## Contributing

Contributions welcome! Please read our contributing guidelines before submitting PRs.

---

*Built for the Global AI Hackathon Series with Qwen Cloud*
