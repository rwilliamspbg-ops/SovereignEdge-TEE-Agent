# Demo Transcript

Real, unedited output of `MODEL_GGUF=stories260K.gguf ./scripts/demo.sh`,
captured 2026-07-16 (ANSI colors stripped).

Environment: Linux 7.0.0, Ryzen laptop with NVIDIA RTX 5070 Laptop GPU,
AMD HawkPoint iGPU, and AMD XDNA (Ryzen AI) NPU. The GGUF model is
**stories260K** — a ~260K-parameter TinyStories toy model (~1.1 MB) used to
demonstrate that the llama.cpp inference path is real; its generated text
is children's-story babble by design, not meaningful telemetry analysis.
Swap in e.g. qwen2.5-0.5b-instruct GGUF for sensible output.

Note the honest boundaries: ONLINE-mode "cloud offload" results are the
simulated TEE-gateway path (see README Phases); OFFLINE-mode results are
genuine llama.cpp token generation. Hardware lines are live sysfs/nvidia-smi
readings, including temperatures.

```text
==============================================
  SovereignEdge-TEE-Agent Quick Start Demo
==============================================

[Step 1/5] Building workspace...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
  MODEL_GGUF set - rebuilding edge-agent with llama.cpp (needs cmake + C++ toolchain)...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s

[Step 2/5] Running test suite...
  14 test suites passed, 0 failed

[Step 3/5] Edge agent — ONLINE mode (frames offloaded to cloud)...
----------------------------------------------
2026-07-16T17:20:24.968972Z  INFO edge_agent: Detected accelerator: [GPU] NVIDIA NVIDIA GeForce RTX 5070 Laptop GPU (driver: nvidia) [44°C, 3.3W, 0% util]
2026-07-16T17:20:24.969234Z  INFO edge_agent: Detected accelerator: [GPU] AMD GPU 0x1900 (driver: amdgpu) [42°C, 32.0W, 0% util]
2026-07-16T17:20:24.969266Z  INFO edge_agent: Detected accelerator: [NPU] AMD XDNA NPU (Ryzen AI) (driver: amdxdna) [no sensors]
2026-07-16T17:20:24.969398Z  INFO edge_agent: Frame 1: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
2026-07-16T17:20:25.069856Z  INFO edge_agent: Frame 2: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
2026-07-16T17:20:25.171298Z  INFO edge_agent: Frame 3: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
2026-07-16T17:20:25.272721Z  INFO edge_agent: Frame 4: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
2026-07-16T17:20:25.374042Z  INFO edge_agent: Frame 5: action=CLOUD_ANALYSIS_COMPLETE, confidence=0.95, source=Cloud
2026-07-16T17:20:25.475366Z  INFO edge_agent: Final stats: 5 frames, 5 cloud offloads, 0 local inferences, 0 mode transitions

[Step 4/5] Edge agent — OFFLINE mode (graceful degradation to local inference)...
----------------------------------------------
  (using real llama.cpp inference: /tmp/claude-1000/-tmp-SovereignEdge-TEE-Agent/060f2334-2f6f-44e5-bf19-498b9a544a8c/scratchpad/stories260K.gguf)
2026-07-16T17:20:25.508280Z  INFO edge_agent: Detected accelerator: [GPU] NVIDIA NVIDIA GeForce RTX 5070 Laptop GPU (driver: nvidia) [44°C, 3.3W, 0% util]
2026-07-16T17:20:25.508533Z  INFO edge_agent: Detected accelerator: [GPU] AMD GPU 0x1900 (driver: amdgpu) [41°C, 29.1W, 0% util]
2026-07-16T17:20:25.508550Z  INFO edge_agent: Detected accelerator: [NPU] AMD XDNA NPU (Ryzen AI) (driver: amdxdna) [no sensors]
2026-07-16T17:20:25.511755Z  INFO edge_agent::inference::llama: [LlamaCpp] Loaded model 'stories260K' (0 GPU layers requested)
2026-07-16T17:20:25.511769Z  INFO edge_agent: [EdgeAgent] Local inference backend: llama.cpp
2026-07-16T17:20:25.511790Z  INFO edge_agent: Mode changed: Online -> Offline
2026-07-16T17:20:25.517351Z  INFO edge_agent: Frame 1: action=" Tom says.
2026-07-16T17:20:25.623527Z  INFO edge_agent: Frame 2: action=Anna, you can go to the park. We can find a way to rea, confidence=0.75, source=Local
2026-07-16T17:20:25.730943Z  INFO edge_agent: Frame 3: action=" Tom says.
2026-07-16T17:20:25.837687Z  INFO edge_agent: Frame 4: action=" Tom says.
2026-07-16T17:20:25.945133Z  INFO edge_agent: Frame 5: action="Hello, Anna. What are you doing?"
2026-07-16T17:20:26.046517Z  INFO edge_agent: Final stats: 5 frames, 0 cloud offloads, 5 local inferences, 1 mode transitions

[Step 5/5] Machine-verified invariants (Lean 4)...
----------------------------------------------
Build completed successfully (7 jobs).
  28 theorems proved (see verification/README.md)

==============================================
  Demo complete
==============================================
Next steps:
  - Run with a GGUF model for real local inference (see header of this script)
  - docs/ARCHITECTURE.md for diagrams; verification/ for proofs
  - README 'Phases' section for the honest implemented/simulated status
```
