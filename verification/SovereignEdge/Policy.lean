/-!
# Policy constraint evaluator — `crates/zk-proofs/src/lib.rs`

Models `ZkProofGenerator::check_constraint` over `Constraint`
(`Range` / `Threshold` / `And` / `Or`, the latter two over lists), with
Rust's exact control flow:

* a missing field is an error (`ZkError::MissingField`), modeled as `none`;
* `And` short-circuits on the first `false`, `Or` on the first `true` —
  **before** later conditions are evaluated, so later errors are masked
  (`and_masks_late_error` / `or_masks_late_error` document this);
* empty `And` is `true`, empty `Or` is `false`.

**Abstractions**: numeric values are `Int` (Rust compares `i64 as f64`
against an `f64` threshold — exact for |v| < 2^53, lossy above; see the
report). Operators are an enum; Rust's string operators reach these
semantics only after the `InvalidOperator` guard, so the enum models
exactly the valid cases.

Main theorems (by mutual structural induction):
* `check_wf_some` — on well-formed input (all referenced fields present)
  the evaluator never errors.
* `check_iff_sat` — soundness *and* completeness: the evaluator returns
  `some true` iff the declarative satisfaction semantics `Sat` holds.
-/

namespace SovereignEdge.Policy

/-- Valid comparison operators (post-`InvalidOperator`-guard). -/
inductive Cmp where
  | gt | ge | lt | le | eq
deriving DecidableEq, Repr

def Cmp.holds : Cmp → Int → Int → Prop
  | .gt, a, b => a > b
  | .ge, a, b => a ≥ b
  | .lt, a, b => a < b
  | .le, a, b => a ≤ b
  | .eq, a, b => a = b

instance : ∀ (op : Cmp) (a b : Int), Decidable (op.holds a b)
  | .gt, _, _ => inferInstanceAs (Decidable (_ > _))
  | .ge, _, _ => inferInstanceAs (Decidable (_ ≥ _))
  | .lt, _, _ => inferInstanceAs (Decidable (_ < _))
  | .le, _, _ => inferInstanceAs (Decidable (_ ≤ _))
  | .eq, _, _ => inferInstanceAs (Decidable (_ = _))

/-- Mirror of the Rust `Constraint` enum (`And`/`Or` over lists). -/
inductive Constraint where
  | range (field : String) (min max : Int)
  | threshold (field : String) (op : Cmp) (value : Int)
  | and (conditions : List Constraint)
  | or (conditions : List Constraint)

/-- `ActionData::get_numeric`. -/
def Data := String → Option Int

mutual
/-- `check_constraint`: `none` models `Err(MissingField)`. -/
def check (d : Data) : Constraint → Option Bool
  | .range f lo hi =>
    match d f with
    | none => none
    | some v => some (decide (lo ≤ v ∧ v ≤ hi))
  | .threshold f op x =>
    match d f with
    | none => none
    | some v => some (decide (op.holds v x))
  | .and cs => checkAll d cs
  | .or cs => checkAny d cs

/-- The `And` loop: first error or first `false` wins. -/
def checkAll (d : Data) : List Constraint → Option Bool
  | [] => some true
  | c :: cs =>
    match check d c with
    | none => none
    | some false => some false
    | some true => checkAll d cs

/-- The `Or` loop: first error or first `true` wins. -/
def checkAny (d : Data) : List Constraint → Option Bool
  | [] => some false
  | c :: cs =>
    match check d c with
    | none => none
    | some true => some true
    | some false => checkAny d cs
end

mutual
/-- Declarative satisfaction semantics. -/
def Sat (d : Data) : Constraint → Prop
  | .range f lo hi => ∃ v, d f = some v ∧ lo ≤ v ∧ v ≤ hi
  | .threshold f op x => ∃ v, d f = some v ∧ op.holds v x
  | .and cs => SatAll d cs
  | .or cs => SatAny d cs

def SatAll (d : Data) : List Constraint → Prop
  | [] => True
  | c :: cs => Sat d c ∧ SatAll d cs

def SatAny (d : Data) : List Constraint → Prop
  | [] => False
  | c :: cs => Sat d c ∨ SatAny d cs
end

mutual
/-- Well-formedness: every field the constraint references is present. -/
def WF (d : Data) : Constraint → Prop
  | .range f _ _ => (d f).isSome
  | .threshold f _ _ => (d f).isSome
  | .and cs => WFAll d cs
  | .or cs => WFAll d cs

def WFAll (d : Data) : List Constraint → Prop
  | [] => True
  | c :: cs => WF d c ∧ WFAll d cs
end

mutual
/-- On well-formed input the evaluator never reports a missing field. -/
theorem check_wf_some (d : Data) : ∀ c, WF d c → (check d c).isSome
  | .range f lo hi, h => by
    unfold check
    unfold WF at h
    cases hd : d f with
    | none => rw [hd] at h; simp [Option.isSome] at h
    | some v => simp
  | .threshold f op x, h => by
    unfold check
    unfold WF at h
    cases hd : d f with
    | none => rw [hd] at h; simp [Option.isSome] at h
    | some v => simp
  | .and cs, h => by
    unfold check
    unfold WF at h
    exact checkAll_wf_some d cs h
  | .or cs, h => by
    unfold check
    unfold WF at h
    exact checkAny_wf_some d cs h

theorem checkAll_wf_some (d : Data) : ∀ cs, WFAll d cs → (checkAll d cs).isSome
  | [], _ => by simp [checkAll]
  | c :: cs, h => by
    unfold checkAll
    obtain ⟨hc, hcs⟩ := h
    cases hchk : check d c with
    | none => have := check_wf_some d c hc; rw [hchk] at this; simp at this
    | some b => cases b <;> simp_all [checkAll_wf_some d cs hcs]

theorem checkAny_wf_some (d : Data) : ∀ cs, WFAll d cs → (checkAny d cs).isSome
  | [], _ => by simp [checkAny]
  | c :: cs, h => by
    unfold checkAny
    obtain ⟨hc, hcs⟩ := h
    cases hchk : check d c with
    | none => have := check_wf_some d c hc; rw [hchk] at this; simp at this
    | some b => cases b <;> simp_all [checkAny_wf_some d cs hcs]
end

mutual
/-- **Soundness and completeness** of `check_constraint` on well-formed
input: it answers `true` exactly on satisfied constraints. -/
theorem check_iff_sat (d : Data) : ∀ c, WF d c → (check d c = some true ↔ Sat d c)
  | .range f lo hi, h => by
    unfold check Sat
    cases hd : d f with
    | none => unfold WF at h; rw [hd] at h; simp [Option.isSome] at h
    | some v => simp
  | .threshold f op x, h => by
    unfold check Sat
    cases hd : d f with
    | none => unfold WF at h; rw [hd] at h; simp [Option.isSome] at h
    | some v => simp
  | .and cs, h => by
    unfold check Sat
    unfold WF at h
    exact checkAll_iff_satAll d cs h
  | .or cs, h => by
    unfold check Sat
    unfold WF at h
    exact checkAny_iff_satAny d cs h

theorem checkAll_iff_satAll (d : Data) :
    ∀ cs, WFAll d cs → (checkAll d cs = some true ↔ SatAll d cs)
  | [], _ => by simp [checkAll, SatAll]
  | c :: cs, h => by
    unfold checkAll SatAll
    obtain ⟨hc, hcs⟩ := h
    have hhead := check_iff_sat d c hc
    have htail := checkAll_iff_satAll d cs hcs
    cases hchk : check d c with
    | none => have := check_wf_some d c hc; rw [hchk] at this; simp at this
    | some b =>
      rw [hchk] at hhead
      cases b with
      | false => simp_all
      | true => simp_all

theorem checkAny_iff_satAny (d : Data) :
    ∀ cs, WFAll d cs → (checkAny d cs = some true ↔ SatAny d cs)
  | [], _ => by simp [checkAny, SatAny]
  | c :: cs, h => by
    unfold checkAny SatAny
    obtain ⟨hc, hcs⟩ := h
    have hhead := check_iff_sat d c hc
    have htail := checkAny_iff_satAny d cs hcs
    cases hchk : check d c with
    | none => have := check_wf_some d c hc; rw [hchk] at this; simp at this
    | some b =>
      rw [hchk] at hhead
      cases b with
      | false => simp_all
      | true => simp_all
end

/-! ## Short-circuit anomalies (documented behavior, machine-checked) -/

/-- A data environment with only field `"x"` present. -/
def dx : Data := fun f => if f = "x" then some 3 else none

/-- `Or` masks a missing-field error in a later condition: the result is
`some true`, not an error, even though `"missing"` is absent. -/
theorem or_masks_late_error :
    checkAny dx [.threshold "x" .gt 0, .range "missing" 0 1] = some true := by
  decide

/-- `And` masks a missing-field error behind an earlier `false`. -/
theorem and_masks_late_error :
    checkAll dx [.threshold "x" .lt 0, .range "missing" 0 1] = some false := by
  decide

end SovereignEdge.Policy
