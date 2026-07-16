/-!
# Mode state machine — `crates/edge-agent/src/lib.rs`

Models `NetworkQuality::is_degraded` / `is_offline` (`crates/common/src/lib.rs`)
and `EdgeAgent::determine_mode`.

**Abstraction**: Rust uses `f64` in ms / percent. We use exact integer
micro-units (µs latency/jitter, ppm packet loss), so thresholds
`200.0 ms`, `5.0 %`, `50.0 ms`, `5000.0 ms`, `50.0 %` become
`200_000`, `50_000`, `50_000`, `5_000_000`, `500_000`. The comparisons
are order-isomorphic to the Rust ones for all finite non-NaN inputs.

Theorems:
* `offline_implies_degraded` — the threshold sets are nested, so the
  three modes are a well-ordered severity scale.
* `determineMode_offline_iff` / `online_iff` / `degraded_iff` — exact
  characterization of `determine_mode`'s if-chain.
* `determineMode_monotone` — a componentwise-worse network never yields
  a less severe mode (no priority inversion in the state machine).
-/

namespace SovereignEdge.ModeMachine

structure NetworkQuality where
  latencyUs : Nat
  lossPpm   : Nat
  jitterUs  : Nat

/-- `NetworkQuality::is_degraded`: latency > 200ms ∨ loss > 5% ∨ jitter > 50ms -/
def isDegraded (q : NetworkQuality) : Bool :=
  decide (q.latencyUs > 200000) || decide (q.lossPpm > 50000) || decide (q.jitterUs > 50000)

/-- `NetworkQuality::is_offline`: latency > 5000ms ∨ loss > 50% -/
def isOffline (q : NetworkQuality) : Bool :=
  decide (q.latencyUs > 5000000) || decide (q.lossPpm > 500000)

inductive AgentMode where
  | online
  | degraded
  | offline
deriving DecidableEq, Repr

/-- `EdgeAgent::determine_mode`: offline checked first, then degraded. -/
def determineMode (q : NetworkQuality) : AgentMode :=
  if isOffline q then .offline
  else if isDegraded q then .degraded
  else .online

/-- Severity scale used to state monotonicity. -/
def severity : AgentMode → Nat
  | .online => 0
  | .degraded => 1
  | .offline => 2

/-- An offline-quality network is always also degraded-quality:
the offline thresholds are strictly inside the degraded thresholds. -/
theorem offline_implies_degraded (q : NetworkQuality) :
    isOffline q = true → isDegraded q = true := by
  simp only [isOffline, isDegraded, Bool.or_eq_true, decide_eq_true_eq]
  rintro (h | h)
  · exact Or.inl (Or.inl (by omega))
  · exact Or.inl (Or.inr (by omega))

theorem determineMode_offline_iff (q : NetworkQuality) :
    determineMode q = .offline ↔ isOffline q = true := by
  unfold determineMode
  cases ho : isOffline q with
  | true => simp
  | false =>
    cases hd : isDegraded q with
    | true => simp
    | false => simp

theorem determineMode_online_iff (q : NetworkQuality) :
    determineMode q = .online ↔ isDegraded q = false := by
  unfold determineMode
  cases ho : isOffline q with
  | true => simp [offline_implies_degraded q ho]
  | false =>
    cases hd : isDegraded q with
    | true => simp
    | false => simp

theorem determineMode_degraded_iff (q : NetworkQuality) :
    determineMode q = .degraded ↔ (isDegraded q = true ∧ isOffline q = false) := by
  unfold determineMode
  cases ho : isOffline q with
  | true => simp
  | false =>
    cases hd : isDegraded q with
    | true => simp
    | false => simp

/-- Componentwise "no better than" order on network quality. -/
def Worse (q q' : NetworkQuality) : Prop :=
  q.latencyUs ≤ q'.latencyUs ∧ q.lossPpm ≤ q'.lossPpm ∧ q.jitterUs ≤ q'.jitterUs

theorem isDegraded_mono {q q' : NetworkQuality} (h : Worse q q') :
    isDegraded q = true → isDegraded q' = true := by
  obtain ⟨h1, h2, h3⟩ := h
  simp only [isDegraded, Bool.or_eq_true, decide_eq_true_eq]
  rintro ((hl | hp) | hj)
  · exact Or.inl (Or.inl (by omega))
  · exact Or.inl (Or.inr (by omega))
  · exact Or.inr (by omega)

theorem isOffline_mono {q q' : NetworkQuality} (h : Worse q q') :
    isOffline q = true → isOffline q' = true := by
  obtain ⟨h1, h2, _⟩ := h
  simp only [isOffline, Bool.or_eq_true, decide_eq_true_eq]
  rintro (hl | hp)
  · exact Or.inl (by omega)
  · exact Or.inr (by omega)

/-- The mode decision is monotone: making the network worse (componentwise)
never produces a less severe mode. -/
theorem determineMode_monotone {q q' : NetworkQuality} (h : Worse q q') :
    severity (determineMode q) ≤ severity (determineMode q') := by
  unfold determineMode
  cases ho : isOffline q with
  | true =>
    simp [isOffline_mono h ho]
  | false =>
    cases ho' : isOffline q' with
    | true =>
      cases isDegraded q <;> simp [severity]
    | false =>
      cases hd : isDegraded q with
      | true => simp [isDegraded_mono h hd, severity]
      | false => cases isDegraded q' <;> simp [severity]

end SovereignEdge.ModeMachine
