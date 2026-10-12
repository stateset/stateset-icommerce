import InventoryExpiry
import InventoryFulfilment

/-! Shipment admission uses the deadline read after acquiring its locks.
An admitted shipment retains those locks until commit; expiry skips it and
can subsequently close only the remaining open units. Natural timestamps
abstract the planner's strict `checkedAt < deadline` condition. -/
namespace InventoryReservations

def shipBeforeDeadline (s : State) (before after : List Row) (r : Row)
    (n : Int) (deadline checkedAt : Nat) : State :=
  if checkedAt < deadline then fulfilQuantity s before after r n else s

theorem shipBeforeDeadline_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Int) (deadline checkedAt : Nat) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) :
    Safe capacity (shipBeforeDeadline s before after r n deadline checkedAt) := by
  unfold shipBeforeDeadline
  split
  · exact fulfilQuantity_safe capacity s before after r n safe row
  · exact safe

theorem expired_deadline_refuses (s : State) (before after : List Row) (r : Row)
    (n : Int) (deadline checkedAt : Nat) (due : deadline ≤ checkedAt) :
    shipBeforeDeadline s before after r n deadline checkedAt = s := by
  simp [shipBeforeDeadline, Nat.not_lt.mpr due]

theorem shipment_effect_requires_live_deadline (s : State) (before after : List Row)
    (r : Row) (n : Int) (deadline checkedAt : Nat)
    (effect : shipBeforeDeadline s before after r n deadline checkedAt ≠ s) :
    checkedAt < deadline := by
  by_cases live : checkedAt < deadline
  · exact live
  · exact False.elim (effect (by simp [shipBeforeDeadline, live]))

theorem stale_admission_differs (startedAt deadline checkedAt : Nat)
    (early : startedAt < deadline) (late : deadline ≤ checkedAt) :
    (startedAt < deadline) ∧ ¬ (checkedAt < deadline) := by omega

theorem shipment_then_expiry_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Nat) (due : Bool) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
    (positive : 0 < n) (within : n ≤ r.quantity) :
    Safe capacity (expireIfDue (fulfil s before after r n) before after (fulfilledRow r n) due) := by
  apply expireIfDue_safe
  · exact step_preserves_safe capacity s _ safe (Step.fulfil s before after r n row live positive within)
  · rfl

theorem shipment_then_due_expiry_preserves_other_holds (s : State) (before after : List Row)
    (r : Row) (n : Nat) (ledger : s.allocated = openUnits s.rows)
    (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
    (within : n ≤ r.quantity) :
    (expireIfDue (fulfil s before after r n) before after (fulfilledRow r n) true).allocated =
      openUnits before + openUnits after := by
  rw [row, openUnits_context] at ledger
  simp [Row.held, live] at ledger
  by_cases full : n = r.quantity
  · simp [expireIfDue, fulfilledRow, full, Status.isOpen, fulfil]
    omega
  · simp [expireIfDue, fulfilledRow, full, live, close, fulfil]
    omega

theorem fully_shipped_hold_never_expires (s : State) (before after : List Row)
    (r : Row) (due : Bool) :
    expireIfDue (fulfil s before after r r.quantity) before after
      (fulfilledRow r r.quantity) due = fulfil s before after r r.quantity := by
  simp [expireIfDue, fulfilledRow, Status.isOpen]

theorem expired_hold_cannot_ship (s : State) (before after : List Row) (r : Row) (n : Int) :
    fulfilQuantity (close s before after r .expired) before after
      { r with status := .expired } n = close s before after r .expired := by
  apply refused_fulfilment_unchanged
  simp [fulfilDecision, Status.isOpen]
  split <;> simp

end InventoryReservations
