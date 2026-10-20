import InventoryReservations

/-! Expiry candidates must be read under the reservation lock. A candidate
list read before a wait can contain a row another transaction has expired.
The proofs abstract a successful locked read as a fresh row value. -/
namespace InventoryReservations

def expiredRow (r : Row) (due : Bool) : Row :=
  if r.status.isOpen && due then { r with status := .expired } else r

/-- Re-reading the row after a serialized expiry makes the second expiry a
no-op, for any quantities and any surrounding reservation ledger. -/
theorem expiry_idempotent_with_fresh_row (s : State) (before after : List Row)
    (r : Row) (due : Bool) :
    expireIfDue (expireIfDue s before after r due) before after (expiredRow r due) due =
      expireIfDue s before after r due := by
  rcases r with ⟨status, quantity⟩
  cases status <;> cases due <;> simp [expireIfDue, expiredRow, Status.isOpen, close]

theorem expired_row_never_releases (s : State) (before after : List Row)
    (quantity : Nat) (due : Bool) :
    expireIfDue s before after ⟨.expired, quantity⟩ due = s := by
  simp [expireIfDue, Status.isOpen]

/-- A stale candidate can release another row's units without making the
balance negative, so the repository's negative-allocation repair cannot save it. -/
theorem stale_expiry_breaks_ledger (capacity : Nat) (s : State)
    (before after : List Row) (quantity : Nat) (positive : 0 < quantity)
    (within : quantity ≤ s.allocated) (safe : Safe capacity s)
    (row : s.rows = before ++ ⟨.expired, quantity⟩ :: after) :
    ¬ Safe capacity (close s before after ⟨.expired, quantity⟩ .expired) := by
  intro broken
  have ledger := safe.1
  rw [row, openUnits_context] at ledger
  have wrong := broken.1
  simp [close, openUnits_context, Row.held, Status.isOpen] at ledger wrong
  omega

end InventoryReservations
