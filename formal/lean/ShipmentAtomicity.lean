/-! One logical dispatch has five durable effect groups: stock/reservations,
shipment ledger, outbox, order, and receipt. Counts are batches, not quantities.
SQL transaction isolation and rollback are assumptions tested by fault injection.
-/
namespace ShipmentAtomicity

abbrev Effects := Fin 5 → Nat
def initial : Effects := fun _ => 0
def Coherent (s : Effects) : Prop := ∀ i, s i = s 4
def Safe (s : Effects) : Prop := Coherent s ∧ s 4 ≤ 1

/-- A private prefix of the transaction's writes. -/
def staged (s : Effects) (completed : Nat) : Effects :=
  fun i => s i + if i.val < completed then 1 else 0

/-- Publish only a complete batch, while holding the command's serialization
lock. Existing successful receipts make subsequent calls no-ops. -/
def attempt (s : Effects) (completed : Nat) (commit : Bool) : Effects :=
  if commit = true ∧ completed = 5 ∧ s 4 = 0 then staged s 5 else s

theorem complete_batch_coherent (s : Effects) (h : Coherent s) :
    Coherent (staged s 5) := by
  intro i
  simp [staged, i.isLt, h i]

theorem attempt_safe (s : Effects) (completed : Nat) (commit : Bool)
    (safe : Safe s) : Safe (attempt s completed commit) := by
  unfold attempt
  split
  · next publish =>
      constructor
      · exact complete_batch_coherent s safe.1
      · simp [staged, publish.2.2]
  · exact safe

theorem rollback_restores_every_group (s : Effects) (completed : Nat) :
    attempt s completed false = s := by simp [attempt]

theorem incomplete_cannot_publish (s : Effects) (completed : Nat) (commit : Bool)
    (incomplete : completed < 5) : attempt s completed commit = s := by
  have ne : completed ≠ 5 := by omega
  simp [attempt, ne]

theorem same_command_retry_is_stable (s : Effects) :
    attempt (attempt s 5 true) 5 true = attempt s 5 true := by
  by_cases receipt : s 4 = 0 <;> simp [attempt, staged, receipt]

/-- Publishing any nonempty proper prefix separates stock effects from the
receipt. This is the negative control for early commit and partial rollback. -/
theorem partial_publication_breaks_coherence (s : Effects) (completed : Nat)
    (coherent : Coherent s) (positive : 0 < completed) (incomplete : completed < 5) :
    ¬ Coherent (staged s completed) := by
  intro broken
  have mismatch := broken 0
  have same := coherent 0
  have notLast : ¬ 4 < completed := by omega
  change s 0 + (if 0 < completed then 1 else 0) =
    s 4 + (if 4 < completed then 1 else 0) at mismatch
  rw [if_pos positive, if_neg notLast] at mismatch
  omega

inductive Reachable : Effects → Prop where
  | init : Reachable initial
  | next {s : Effects} (completed : Nat) (commit : Bool) :
      Reachable s → Reachable (attempt s completed commit)

theorem reachable_safe (s : Effects) (h : Reachable s) : Safe s := by
  induction h with
  | init => simp [Safe, Coherent, initial]
  | next completed commit _ ih => exact attempt_safe _ _ _ ih

theorem each_group_at_most_once (s : Effects) (h : Reachable s) (i : Fin 5) :
    s i ≤ 1 := by
  have safe := reachable_safe s h
  rw [safe.1 i]
  exact safe.2

end ShipmentAtomicity
