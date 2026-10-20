import InventoryReservations

namespace InventoryReservations

inductive FulfilDecision where
  | invalidQuantity | closed | excess | accepted
  deriving DecidableEq, Repr

/-- The repository rejects nonpositive quantities before checking the row.
Signed input models negative decimal amounts as well as zero. -/
def fulfilDecision (r : Row) (n : Int) : FulfilDecision :=
  if n ≤ 0 then .invalidQuantity
  else if r.status.isOpen = false then .closed
  else if (r.quantity : Int) < n then .excess
  else .accepted

theorem fulfil_accepted_iff (r : Row) (n : Int) :
    fulfilDecision r n = .accepted ↔
      0 < n ∧ r.status.isOpen = true ∧ n ≤ (r.quantity : Int) := by
  simp only [fulfilDecision]
  split <;> rename_i h
  · simp; omega
  · split <;> rename_i live
    · simp [live]
    · have isOpen : r.status.isOpen = true := by cases e : r.status.isOpen <;> simp_all
      split <;> simp_all <;> omega

def fulfilQuantity (s : State) (before after : List Row) (r : Row) (n : Int) : State :=
  if fulfilDecision r n = .accepted then fulfil s before after r n.toNat else s

theorem fulfilQuantity_step (s : State) (before after : List Row) (r : Row)
    (n : Int) (row : s.rows = before ++ r :: after) :
    Step s (fulfilQuantity s before after r n) := by
  unfold fulfilQuantity
  split
  · next accepted =>
      obtain ⟨positive, live, within⟩ := (fulfil_accepted_iff r n).mp accepted
      exact Step.fulfil s before after r n.toNat row live (by omega) (by omega)
  · exact Step.stutter s

theorem fulfilQuantity_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Int) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) :
    Safe capacity (fulfilQuantity s before after r n) :=
  step_preserves_safe capacity s _ safe (fulfilQuantity_step s before after r n row)

theorem refused_fulfilment_unchanged (s : State) (before after : List Row)
    (r : Row) (n : Int) (refused : fulfilDecision r n ≠ .accepted) :
    fulfilQuantity s before after r n = s := by
  simp [fulfilQuantity, refused]

/-- Shipping held units reduces on-hand and allocated equally, preserving
the stock available to other customers. -/
theorem fulfilQuantity_available (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Int) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) :
    (fulfilQuantity s before after r n).onHand -
      (fulfilQuantity s before after r n).allocated = s.onHand - s.allocated := by
  unfold fulfilQuantity
  split
  · next accepted =>
      obtain ⟨positive, live, within⟩ := (fulfil_accepted_iff r n).mp accepted
      have ledger := safe.1
      rw [row, openUnits_context] at ledger
      simp [Row.held, live] at ledger
      have bounds := safe.2.1
      simp only [fulfil]
      omega
  · rfl

end InventoryReservations
