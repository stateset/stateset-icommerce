import InventoryFulfilment

/-! A shipment and a terminal close serialize on their reservation row.
The second operation must use its current status AND remaining quantity.
Atomic commits and fresh locked reads are assumptions, not SQL proofs. -/
namespace InventoryReservations

def releaseIfOpen (s : State) (before after : List Row) (r : Row) : State :=
  if r.status.isOpen then close s before after r .released else s

theorem releaseIfOpen_step (s : State) (before after : List Row) (r : Row)
    (row : s.rows = before ++ r :: after) :
    Step s (releaseIfOpen s before after r) := by
  unfold releaseIfOpen
  split
  · next live => exact Step.close s before after r .released row live (Or.inl rfl)
  · exact Step.stutter s

theorem releaseIfOpen_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (safe : Safe capacity s) (row : s.rows = before ++ r :: after) :
    Safe capacity (releaseIfOpen s before after r) :=
  step_preserves_safe capacity s _ safe (releaseIfOpen_step s before after r row)

theorem ship_then_release_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Nat) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
    (positive : 0 < n) (within : n ≤ r.quantity) :
    Safe capacity (releaseIfOpen (fulfil s before after r n) before after (fulfilledRow r n)) := by
  apply releaseIfOpen_safe
  · exact step_preserves_safe capacity s _ safe (Step.fulfil s before after r n row live positive within)
  · rfl

/-- Releasing after a partial shipment frees just the remainder; after full
shipment it is a no-op. All other rows retain exactly their live allocation. -/
theorem ship_then_release_preserves_other_holds (s : State) (before after : List Row)
    (r : Row) (n : Nat) (ledger : s.allocated = openUnits s.rows)
    (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
    (within : n ≤ r.quantity) :
    (releaseIfOpen (fulfil s before after r n) before after (fulfilledRow r n)).allocated =
      openUnits before + openUnits after := by
  rw [row, openUnits_context] at ledger
  simp [Row.held, live] at ledger
  by_cases full : n = r.quantity
  · simp [releaseIfOpen, fulfilledRow, full, Status.isOpen, fulfil]
    omega
  · simp [releaseIfOpen, fulfilledRow, full, live, close, fulfil]
    omega

theorem full_ship_then_release_noop (s : State) (before after : List Row) (r : Row) :
    releaseIfOpen (fulfil s before after r r.quantity) before after
      (fulfilledRow r r.quantity) = fulfil s before after r r.quantity := by
  simp [releaseIfOpen, fulfilledRow, Status.isOpen]

/-- The opposite serialization rejects every shipment quantity after close,
including quantities smaller than the old hold. -/
theorem release_then_ship_noop (s : State) (before after : List Row) (r : Row) (n : Int) :
    fulfilQuantity (close s before after r .released) before after
      { r with status := .released } n = close s before after r .released := by
  apply refused_fulfilment_unchanged
  simp [fulfilDecision, Status.isOpen]
  split <;> simp

theorem release_then_ship_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Int) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true) :
    Safe capacity (fulfilQuantity (close s before after r .released) before after
      { r with status := .released } n) := by
  rw [release_then_ship_noop]
  exact step_preserves_safe capacity s _ safe (Step.close s before after r .released row live (Or.inl rfl))

/-- A cached pre-shipment quantity loses live sibling stock even if the
allocation stays nonnegative. Status checks alone do not prevent this. -/
theorem stale_partial_release_steals_sibling (sibling quantity shipped : Nat)
    (positive : 0 < shipped)
    (enoughSibling : shipped ≤ sibling) :
    (sibling + quantity - shipped) - quantity < sibling := by
  omega

end InventoryReservations
