# Architecture

## Component Overview

Status legend: **solid** = implemented & tested, **dashed** = simulated
placeholder (real interface, mock backend).

```mermaid
flowchart LR
    subgraph EDGE["Edge Node"]
        direction TB
        XDP["xdp-ingest<br/>eBPF XDP filter + AF_XDP daemon"]
        PQC["pqc-transport<br/>X25519 + ML-KEM-768 hybrid KEX<br/>AES-256-GCM sessions"]
        AGENT["edge-agent<br/>mode state machine<br/>GPU/NPU detection + sensors<br/>llama.cpp local inference"]
        XDP -->|TelemetryFrame| PQC
        PQC -->|EncryptedFrame| AGENT
    end

    subgraph CLOUD["Alibaba Cloud TEE (planned deployment)"]
        direction TB
        GW["tee-gateway<br/>sealed API tokens<br/>prompt construction"]
        QWEN["Qwen Cloud API<br/>qwen-max"]
        ZK["zk-proofs<br/>policy constraints<br/>execution logs"]
        GW -->|prompt| QWEN
        QWEN -->|response| GW
        GW -->|execution log| ZK
    end

    AGENT -.->|"encrypted UDP :47821 (planned wiring)"| GW
    ZK -->|verifiable log| OBS["Downstream observers"]

    COMMON["common<br/>shared types"] -.- EDGE
    COMMON -.- CLOUD
    LEAN["verification/ (Lean 4)<br/>28 machine-checked theorems"] -.-|proves invariants of| EDGE
    LEAN -.-|proves invariants of| ZK
```

## Crate Dependency Graph

```mermaid
flowchart BT
    common
    helpers --> common
    xdp-ingest --> common
    xdp-ingest --> helpers
    pqc-transport --> common
    pqc-transport --> helpers
    edge-agent --> common
    edge-agent --> helpers
    edge-agent --> pqc-transport
    tee-gateway --> common
    zk-proofs --> common
```

`edge-agent` optionally links llama.cpp behind the `llama` cargo feature.

## Graceful Degradation State Machine

Machine-verified in `verification/SovereignEdge/ModeMachine.lean`:
the offline thresholds nest inside the degraded ones
(`offline_implies_degraded`), the transitions below are exactly
`determine_mode` (`determineMode_*_iff`), and a worse network can never
produce a less severe mode (`determineMode_monotone`).

```mermaid
stateDiagram-v2
    [*] --> Online
    Online --> Degraded: latency > 200ms ∨ loss > 5% ∨ jitter > 50ms
    Degraded --> Offline: latency > 5000ms ∨ loss > 50%
    Online --> Offline: (offline thresholds crossed directly)
    Degraded --> Online: quality recovers
    Offline --> Degraded: partial recovery
    Offline --> Online: full recovery (flush spooled state)

    note right of Online: all frames → cloud offload
    note right of Degraded: cloud with local fallback
    note right of Offline: all frames → local inference,<br/>state spooled for reconciliation
```

## Frame Processing Sequence (target end-to-end path)

```mermaid
sequenceDiagram
    participant NIC as NIC / XDP
    participant D as af_xdp_daemon
    participant P as PqcSession
    participant A as EdgeAgent
    participant G as TeeGateway (TEE)
    participant Q as Qwen API
    participant Z as ZkProofGenerator

    NIC->>D: UDP :47821 (XDP-filtered)
    D->>P: TelemetryFrame
    P->>P: AES-256-GCM encrypt<br/>(counter nonce, verified unique)
    P->>A: EncryptedFrame
    A->>A: determine_mode(network quality)
    alt Online / Degraded with reachable cloud
        A->>G: encrypted offload
        G->>G: attest + unseal API token
        G->>Q: structured prompt
        Q-->>G: inference result
        G->>Z: execution log
        Z->>Z: check policy constraints<br/>(evaluator machine-verified)
        Z-->>A: verifiable result
    else Offline / unreachable cloud
        A->>A: local inference (llama.cpp)
    end
```

**Current wiring status**: each stage works in isolation (unit tests, demo
binaries); the cross-stage arrows marked "planned" in the overview are not
yet connected in code — see `tests/integration/` for the closest
end-to-end exercise, and the Phases section of the README for what is
simulated.
