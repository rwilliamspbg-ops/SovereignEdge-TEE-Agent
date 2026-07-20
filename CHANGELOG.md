# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Real ML-KEM-768 (FIPS 203) post-quantum KEM** via `ml-kem` crate (RustCrypto)
  - `HybridKem::generate()` produces real ML-KEM-768 keypairs (replaces random-byte placeholder)
  - `encapsulate_to_peer()` — real ML-KEM encapsulation to remote public key
  - `decapsulate_from_peer()` — real ML-KEM decapsulation from peer ciphertext
  - Bidirectional key exchange roundtrip: both parties derive identical shared secrets
  - Benchmark: 54 µs keygen, 286 µs full roundtrip (release build)
- **TEE backend trait abstraction** (`TeeBackend` with `seal()`, `unseal()`, `attest()`)
  - `SimulatedTee` as default backend
  - `SealedStorage<T: TeeBackend>` generic over any TEE implementation
  - Ready for SGX (`aesm-client`), SEV-SNP (`sev`), Alibaba CAS backends
- **Real Qwen Cloud API** via `reqwest` HTTP client
  - Replaced hardcoded mock response with real HTTP POST
  - `TeeGateway::process_frame()` is now `async`
  - rustls-tls for transport security
  - JSON body with Bearer token authentication
- **arkworks Groth16 ZK-SNARK integration** on BN254 curve
  - `PolicyCircuit` implements `ConstraintSynthesizer<Fr>` for R1CS
  - `Groth16::<Bn254>::circuit_specific_setup()` generates CRS per policy
  - `Groth16::<Bn254>::prove()` generates real Groth16 proofs
  - `serialize_proof()` for 192-byte proof serialization
  - Benchmark: 7.6 ms per proof (setup + prove)
- Feature flags: `simulated`, `real-zk` in `tee-gateway` and `zk-proofs`
- Rust toolchain pinned to stable (1.85+) via `rust-toolchain.toml`
- MSVC target configured via `.cargo/config.toml` for Windows builds
- Inline benchmark tests in `pqc-transport` and `zk-proofs`

### Changed

- **BREAKING**: `HybridSecretKey` stores ML-KEM seed (64 bytes) instead of expanded key (1632 bytes)
- **BREAKING**: `HybridPublicKey.mlkem_pubkey` is now `Vec<u8>` instead of `[u8; 1088]`
- **BREAKING**: `HybridEncapsulation.mlkem_ciphertext` is now `Vec<u8>` instead of `[u8; 1568]`
- **BREAKING**: `PqcTransportManager::establish_session()` returns `(Session, Encapsulation)` tuple
- **BREAKING**: `TeeGateway` is now generic over `T: TeeBackend`
- **BREAKING**: `TeeGateway::process_frame()` is now `async`
- AES-GCM associated data standardized to `frame_id` for encrypt/decrypt consistency
- `rust-toolchain.toml` pins `stable` with MSVC target
- Updated workspace dependencies to include `ml-kem`, `reqwest`, `ark-*` crates

### Fixed

- AES-GCM encrypt/decrypt roundtrip: associated data mismatch between `encrypt_frame()` and `decrypt_frame()`
- Session key derivation: receiver's send/recv keys correctly swapped relative to sender
- ML-KEM ciphertext size constant corrected from 1568 to 1088 bytes (actual BN254 size)

### Security

- `cargo audit` findings:
  - ⚠️ `tracing-subscriber` 0.2.25: ANSI escape sequence poisoning (RUSTSEC-2025-0055) — upgrade to >=0.3.20
  - ℹ️ `derivative` 2.2.0: unmaintained (proc-macro only, low risk)
  - ℹ️ `paste` 1.0.15: unmaintained (proc-macro only, low risk)

### Removed

- Placeholder ML-KEM-768 random-byte generation
- Hardcoded Qwen API mock responses
- SHA-256-only ZK proof generation (replaced with arkworks Groth16)

[Unreleased]: https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent/compare/v0.1.0...HEAD
