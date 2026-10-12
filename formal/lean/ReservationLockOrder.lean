/-! Strictly increasing resource ranks exclude finite circular waits among
participants obeying that order. This is a protocol theorem, not a proof that
every SQL path in the engine follows the protocol. -/
namespace ReservationLockOrder

structure WaitingThread where
  held : List Nat
  requested : Nat

def Ordered (t : WaitingThread) : Prop :=
  ∀ resource ∈ t.held, resource < t.requested

def WaitsFor (waiter owner : WaitingThread) : Prop :=
  waiter.requested ∈ owner.held

theorem wait_edge_increases (waiter owner : WaitingThread)
    (ordered : Ordered owner) (waits : WaitsFor waiter owner) :
    waiter.requested < owner.requested := ordered waiter.requested waits

inductive WaitPath (threads : Nat → WaitingThread) : Nat → Nat → Prop where
  | edge {a b : Nat} (waits : WaitsFor (threads a) (threads b)) : WaitPath threads a b
  | extend {a b c : Nat} (path : WaitPath threads a b)
      (waits : WaitsFor (threads b) (threads c)) : WaitPath threads a c

theorem wait_path_increases (threads : Nat → WaitingThread)
    (ordered : ∀ t, Ordered (threads t)) {a b : Nat} (path : WaitPath threads a b) :
    (threads a).requested < (threads b).requested := by
  induction path with
  | edge waits => exact wait_edge_increases _ _ (ordered _) waits
  | extend path waits ih =>
      exact Nat.lt_trans ih (wait_edge_increases _ _ (ordered _) waits)

/-- Any nonempty finite cycle would make a requested rank less than itself. -/
theorem no_circular_wait (threads : Nat → WaitingThread)
    (ordered : ∀ t, Ordered (threads t)) (start : Nat) :
    ¬ WaitPath threads start start := by
  intro cycle
  exact Nat.lt_irrefl _ (wait_path_increases threads ordered cycle)

def reservationRank (index : Nat) : Nat := index
def balanceRank (reservationCount index : Nat) : Nat := reservationCount + index

theorem empty_waiter_ordered (next : Nat) : Ordered ⟨[], next⟩ := by
  simp [Ordered]

/-- Acquiring the requested resource and then requesting a larger one
preserves the premise of the finite-cycle exclusion theorem. -/
theorem ordered_waiter_advance (held : List Nat) (requested next : Nat)
    (ordered : Ordered ⟨held, requested⟩) (advance : requested < next) :
    Ordered ⟨held ++ [requested], next⟩ := by
  intro resource member
  simp only [List.mem_append, List.mem_singleton] at member
  rcases member with old | justAcquired
  · exact Nat.lt_trans (ordered resource old) advance
  · subst resource; exact advance

theorem reservations_follow_identity_order (first second : Nat) (order : first < second) :
    reservationRank first < reservationRank second := order

/-- Deadline priority cannot replace identity ordering in a blocking scan:
the earlier deadline may belong to the later resource identity. -/
theorem reversed_reservations_are_not_ordered :
    ¬ Ordered ⟨[reservationRank 1], reservationRank 0⟩ := by
  intro ordered
  have impossible := ordered (reservationRank 1) (by simp)
  simp [reservationRank] at impossible

theorem reservation_before_every_balance (count reservation balance : Nat)
    (within : reservation < count) :
    reservationRank reservation < balanceRank count balance := by
  simp [reservationRank, balanceRank]
  omega

theorem balances_follow_index_order (count first second : Nat) (order : first < second) :
    balanceRank count first < balanceRank count second := by
  simp [balanceRank]
  omega

/-- Interleaving the shared balance before the second reservation violates
the order, even though reservation IDs themselves were traversed in order. -/
theorem interleaved_ship_is_not_ordered :
    ¬ Ordered ⟨[reservationRank 0, balanceRank 2 0], reservationRank 1⟩ := by
  intro ordered
  have impossible := ordered (balanceRank 2 0) (by simp)
  simp [balanceRank, reservationRank] at impossible

end ReservationLockOrder
