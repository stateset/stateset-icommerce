import InventoryReservations

/-! Balance-version predictions for reservation lifecycle previews. The version
is a balance version: confirming or splitting a live hold does not increment it.
These statements assume the same snapshot and expiry decision at apply time. -/
namespace InventoryReservations

def changesBalance (status : Status) (release due : Bool) : Bool :=
  status.isOpen && (release || (status != .confirmed && due))

def releaseRow (s : State) (before after : List Row) (r : Row) (due : Bool) : State :=
  if r.status.isOpen then close s before after r (if due then .expired else .released)
  else s

theorem releaseRow_step (s : State) (before after : List Row) (r : Row)
    (due : Bool) (row : s.rows = before ++ r :: after) :
    Step s (releaseRow s before after r due) := by
  unfold releaseRow
  split
  · next live =>
      apply Step.close s before after r _ row live
      cases due <;> simp
  · exact Step.stutter s

/-- A preview predicts the exact allocation delta of any quantity confirmation,
including full confirmation, partial confirmation, and terminal no-ops. -/
theorem confirm_prediction (s : State) (before after : List Row) (r : Row)
    (n : Nat) (due : Bool) :
    (confirmQuantity s before after r n due).allocated =
      if changesBalance r.status false due then s.allocated - r.quantity else s.allocated := by
  rcases r with ⟨status, quantity⟩
  by_cases full : quantity ≤ n <;> by_cases positive : 0 < n <;>
    cases status <;> cases due <;>
    simp [confirmQuantity, changesBalance, Status.isOpen, close, replace, splitConfirm, full, positive]

theorem release_prediction (s : State) (before after : List Row) (r : Row)
    (due : Bool) :
    (releaseRow s before after r due).allocated =
      if changesBalance r.status true due then s.allocated - r.quantity else s.allocated := by
  rcases r with ⟨status, quantity⟩
  cases status <;> cases due <;>
    simp [releaseRow, changesBalance, Status.isOpen, close]

/-- For a valid positive hold, decrementing it really changes the balance. -/
theorem positive_hold_changes (allocated quantity : Nat)
    (positive : 0 < quantity) (within : quantity ≤ allocated) :
    allocated - quantity ≠ allocated := by omega

theorem terminal_prediction (status : Status) (release due : Bool)
    (closed : status.isOpen = false) : changesBalance status release due = false := by
  simp [changesBalance, closed]

end InventoryReservations
