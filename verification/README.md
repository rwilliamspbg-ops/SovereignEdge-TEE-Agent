# Machine-Verified Invariants (Lean 4)

Formal models and full proofs for the invariant-critical components of
SovereignEdge-TEE-Agent. Every theorem is completely proved — no `sorry`,
no `native_decide`; `#print axioms` shows only Lean's standard axioms.

## Building

```bash
cd verification
lake build   # requires elan / Lean 4.32.0 (pinned in lean-toolchain)
```

## What is proved

| Module | Rust source | Key theorems |
|---|---|---|
| `ModeMachine.lean` | `edge-agent/src/lib.rs` `determine_mode`, `common/src/lib.rs` quality predicates | `offline_implies_degraded` (threshold nesting), `determineMode_*_iff` (exact if-chain characterization), `determineMode_monotone` (worse network ⇒ never less severe mode) |
| `ContextBuffer.lean` | `common/src/lib.rs` `ContextBuffer::push` | `push_bytes_bound`, `push_frames_bound` (caps hold after every push) + machine-checked counterexamples proving the preconditions are necessary |
| `Nonce.lean` | `pqc-transport/src/lib.rs` `PqcSession::encrypt` | `run_noDup` (no AES-GCM nonce reuse within a session), `run_stops_at_limit` (counter never wraps) |
| `Policy.lean` | `zk-proofs/src/lib.rs` `check_constraint` | `check_iff_sat` (soundness + completeness vs. declarative semantics), `check_wf_some` (no error on well-formed input), short-circuit anomaly theorems |

## Findings surfaced by verification

1. **`ContextBuffer::push` can exceed `max_bytes`** — a single frame larger
   than the cap is admitted after draining the buffer
   (`oversized_frame_breaks_bytes_bound`). The Rust invariant holds only under
   the precondition `frame.payload.len() <= max_bytes`, which no caller
   currently enforces.
2. **`max_frames == 0` is not honored** (`zero_capacity_breaks_frames_bound`) —
   the count cap requires `max_frames >= 1`.
3. **Constraint evaluation masks late errors** — `Or` returns `Ok(true)` and
   `And` returns `Ok(false)` without evaluating later conditions, so a
   `MissingField` error behind a short-circuit is silently ignored
   (`or_masks_late_error`, `and_masks_late_error`). This is faithful to the
   Rust control flow; whether it is *desired* is a policy-design question.

## Modeling abstractions

- Rust `f64` quality metrics are modeled as exact integers in micro-units
  (order-isomorphic for finite non-NaN values).
- `Threshold` values are `Int`; Rust compares `i64 as f64` against an `f64`,
  which is exact only for |v| < 2^53 — the cast is lossy above that, an
  implementation caveat outside this model.
- Operator strings are modeled as an enum: invalid strings hit the
  `InvalidOperator` guard before the verified semantics apply.
- Frames are abstracted to payload sizes; eviction logic inspects nothing else.
