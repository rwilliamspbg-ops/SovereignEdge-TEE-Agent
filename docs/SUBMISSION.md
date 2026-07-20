# Devpost Submission — SovereignEdge-TEE-Agent

**Track 5: EdgeAgent** (physical devices — robots, IoT agents, smart hardware)

## Devpost text description (paste into the form)

SovereignEdge-TEE-Agent is a Rust edge-agent infrastructure for physical
devices that keeps making safe decisions when the network doesn't cooperate —
and can prove it behaved.

An edge node ingests device telemetry, encrypts it with a hybrid
post-quantum handshake (X25519 + ML-KEM-768 interface, AES-256-GCM sessions),
and routes every frame through a graceful-degradation state machine:
**Online** frames are relayed by a TEE gateway that makes real **qwen-max**
calls on **Qwen Cloud (DashScope)**; when connectivity degrades or drops, the
agent fails over to **real on-device llama.cpp inference**, selected using
live GPU/NPU detection (it finds NVIDIA/AMD GPUs and NPUs like AMD XDNA/
Ryzen AI through Linux sysfs, with live temperature and power sensors).
Every cloud decision is written to a hash-chained execution log checked
against a safety-policy constraint system.

What makes it different: the core invariants are **machine-verified in
Lean 4** — 28 fully proved theorems (no `sorry`) covering the degradation
state machine (a worse network can never yield a less severe mode), buffer
eviction bounds, AES-GCM nonce uniqueness, and soundness + completeness of
the policy evaluator. Verification surfaced three real bugs/preconditions,
documented in the repo. The README labels every component honestly:
implemented & tested vs. simulated scaffold vs. planned.

Built with: Rust (7-crate workspace), Qwen Cloud via DashScope
OpenAI-compatible API, llama.cpp, eBPF/XDP, Lean 4.

## How judges reproduce it (2 minutes)

```bash
git clone https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent
cd SovereignEdge-TEE-Agent
./scripts/demo.sh                          # build, tests, both agent modes, Lean proofs

export QWEN_API_KEY=sk-...                 # Alibaba Cloud Model Studio key
cargo run --bin tee_gateway -- --prompt "engine temp 92C, vibration rising"
```

## 3-minute video script (~460 words at demo pace)

**[0:00–0:20] Hook (face or title card)**
> "Edge devices lose connectivity at the worst possible moments. SovereignEdge
> is an edge agent that keeps making safe, AI-driven decisions offline, uses
> Qwen Cloud when it's reachable — and mathematically proves its failover
> logic can't do the wrong thing. Let me show you."

**[0:20–1:00] Terminal: `DEMO_PAUSE=2.5 MODEL_GGUF=... ./scripts/demo.sh`**
While Step 1–2 scroll:
> "One command builds the seven-crate Rust workspace and runs the full test
> suite. Now watch the hardware detection — it finds my RTX 5070, the AMD
> iGPU, and the Ryzen AI NPU straight from Linux sysfs, with live
> temperatures. The agent uses this to pick its local inference backend."

**[1:00–1:40] Steps 3–4 on screen**
> "Online mode: every telemetry frame is offloaded to the cloud path.
> Now the network dies — the state machine transitions to offline, and the
> same frames are handled by a real llama.cpp model running locally on the
> device. No cloud, no problem. This transition logic isn't just tested —
> it's proven."

**[1:40–2:20] Terminal: `cargo run --bin tee_gateway -- --prompt "engine temp 92C, vibration rising"`**
> "When the cloud IS reachable, here's the real thing: the TEE gateway seals
> my API key, sends the telemetry to qwen-max on Qwen Cloud through
> DashScope, and gets back a structured decision — action, confidence,
> reasoning — plus a hash-chained execution log so downstream systems can
> verify what the agent did." *(point at the request_id and content)*

**[2:20–2:50] Show `verification/` — scroll ModeMachine.lean, run `lake build`**
> "And the part I'm proudest of: 28 Lean 4 theorems, fully proved, no sorry.
> The offline thresholds provably nest inside the degraded ones, a worse
> network can never produce a less severe mode, and AES-GCM nonces can never
> repeat. The proofs even caught three real bugs before any packet did."

**[2:50–3:00] Close (architecture diagram on screen)**
> "SovereignEdge: edge autonomy powered by Qwen Cloud, verified by
> mathematics. Track 5, EdgeAgent. Thanks!"

### Recording tips
- Record at 1080p, terminal font ≥ 16pt, one take per segment is fine
- `DEMO_PAUSE=4` gives you comfortable narration gaps
- OBS Studio → YouTube (set visibility **Public**, not unlisted)

## Submission checklist (Devpost form)

- [ ] Repo URL: https://github.com/rwilliamspbg-ops/SovereignEdge-TEE-Agent (public, MIT license ✓)
- [ ] Track: **Track 5 — EdgeAgent**
- [ ] Text description: section above
- [ ] Architecture diagram: `docs/ARCHITECTURE.md` (link or screenshot)
- [ ] Alibaba Cloud usage code file: `crates/tee-gateway/src/main.rs` (+ live run capture)
- [ ] Video: YouTube/Vimeo/Facebook link, ~3 min, **public**
- [ ] Optional blog post for Blog Post Prize
