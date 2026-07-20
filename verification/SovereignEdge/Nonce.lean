/-!
# AES-GCM nonce discipline — `crates/pqc-transport/src/lib.rs`

Models `PqcSession::encrypt`'s send-nonce counter: each call uses the
current counter as the GCM nonce and increments it, refusing to encrypt
once the counter reaches `2^64` (`PqcError::NonceExhausted`).

The security-critical property for AES-GCM is that a (key, nonce) pair is
never reused. Send keys are per-session and per-direction (domain-separated
in `establish_session`), so within the model it suffices that the sequence
of nonces emitted by any run of `encrypt` calls on one session is
duplicate-free.

Theorems:
* `mem_run_bounds` — every emitted nonce is ≥ the starting counter and
  < `2^64` (so the 12-byte truncation `nonce_bytes[0..12]` is injective
  on emitted values: they differ in their low 64 bits alone).
* `run_noDup` — **no nonce is ever emitted twice**, for any session state
  and any number of encrypt calls.
* `run_stops_at_limit` — the session hard-stops rather than wrapping.
-/

namespace SovereignEdge.Nonce

/-- `PqcSession::encrypt` rejects when `send_nonce ≥ 1 << 64`. -/
def LIMIT : Nat := 2 ^ 64

/-- Nonces emitted by `k` consecutive `encrypt` calls starting from
counter `c`. A rejected call emits nothing and leaves the counter
unchanged, so the run ends. -/
def run (c : Nat) : Nat → List Nat
  | 0 => []
  | k + 1 => if c ≥ LIMIT then [] else c :: run (c + 1) k

/-- Duplicate-freedom, stated without stdlib dependencies. -/
def NoDup : List Nat → Prop
  | [] => True
  | x :: xs => x ∉ xs ∧ NoDup xs

theorem mem_run_bounds (k c : Nat) : ∀ n ∈ run c k, c ≤ n ∧ n < LIMIT := by
  induction k generalizing c with
  | zero => intro n hn; simp [run] at hn
  | succ k ih =>
    intro n hn
    unfold run at hn
    split at hn
    · simp at hn
    · rcases List.mem_cons.mp hn with rfl | hmem
      · omega
      · have := ih (c + 1) n hmem
        omega

/-- **Nonce uniqueness.** No run of `encrypt` calls on a session ever
emits the same GCM nonce twice. -/
theorem run_noDup (k c : Nat) : NoDup (run c k) := by
  induction k generalizing c with
  | zero => simp [run, NoDup]
  | succ k ih =>
    unfold run
    split
    · simp [NoDup]
    · refine ⟨fun hmem => ?_, ih (c + 1)⟩
      have := (mem_run_bounds k (c + 1) c hmem).1
      omega

/-- The counter never wraps: once the limit is reached, nothing more is
emitted no matter how many further calls are made. -/
theorem run_stops_at_limit (k c : Nat) (h : c ≥ LIMIT) : run c k = [] := by
  cases k with
  | zero => rfl
  | succ k => simp [run, h]

/-- A run of `k` calls emits at most `k` nonces (each call emits at most one). -/
theorem run_length_le (k c : Nat) : (run c k).length ≤ k := by
  induction k generalizing c with
  | zero => simp [run]
  | succ k ih =>
    unfold run
    split
    · simp
    · simpa using ih (c + 1)

end SovereignEdge.Nonce
