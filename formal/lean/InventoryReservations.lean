/-!
# Reservation ledger safety for arbitrary finite executions

An exact-unit abstraction of the inventory repository and the guarded
`tla/inventory/InventoryReservations.tla` protocol. Quantities are natural
numbers at a common scale. A fulfilled row retains its recorded quantity but
contributes zero to the live ledger. Database atomicity is an assumption.

Unlike the bounded TLC instance, the proof quantifies over every capacity,
every finite number of reservations, and every finite sequence of steps.
It is not a refinement proof of the Rust/SQL implementation.
-/

namespace InventoryReservations

inductive Status where
  | pending | confirmed | allocated | fulfilled | released | expired | cancelled
  deriving DecidableEq, Repr

def Status.isOpen : Status → Bool
  | .pending | .confirmed | .allocated => true
  | _ => false

structure Row where
  status : Status
  quantity : Nat
  deriving DecidableEq, Repr

def Row.held (r : Row) : Nat := if r.status.isOpen then r.quantity else 0

def openUnits (rows : List Row) : Nat := (rows.map Row.held).sum

theorem openUnits_append (left right : List Row) :
    openUnits (left ++ right) = openUnits left + openUnits right := by
  induction left with
  | nil => simp [openUnits]
  | cons r rest ih => simp [openUnits, Nat.add_assoc] at ih ⊢; omega

theorem openUnits_context (before after : List Row) (r : Row) :
    openUnits (before ++ r :: after) = openUnits before + r.held + openUnits after := by
  rw [openUnits_append]
  simp [openUnits, Nat.add_assoc]

structure State where
  onHand : Nat
  allocated : Nat
  shipped : Nat
  rows : List Row

def initial (capacity : Nat) : State := ⟨capacity, 0, 0, []⟩

def Safe (capacity : Nat) (s : State) : Prop :=
  s.allocated = openUnits s.rows ∧
  s.allocated ≤ s.onHand ∧ s.onHand + s.shipped = capacity

def reserve (s : State) (quantity : Nat) : State :=
  { s with allocated := s.allocated + quantity,
           rows := ⟨.pending, quantity⟩ :: s.rows }

def replace (s : State) (before after : List Row) (r : Row) : State :=
  { s with rows := before ++ r :: after }

def fulfilledRow (r : Row) (quantity : Nat) : Row :=
  if quantity = r.quantity then { r with status := .fulfilled }
  else { r with quantity := r.quantity - quantity }

def fulfil (s : State) (before after : List Row) (r : Row) (quantity : Nat) : State :=
  { onHand := s.onHand - quantity,
    allocated := s.allocated - quantity,
    shipped := s.shipped + quantity,
    rows := before ++ fulfilledRow r quantity :: after }

def close (s : State) (before after : List Row) (r : Row) (status : Status) : State :=
  { s with allocated := s.allocated - r.quantity,
           rows := before ++ { r with status := status } :: after }

def splitConfirm (s : State) (before after : List Row) (r : Row) (n : Nat) : State :=
  { s with rows := before ++ { r with quantity := r.quantity - n } ::
                         ⟨.confirmed, n⟩ :: after }

/-- Each constructor is one atomic commit; refused/replayed calls stutter. -/
inductive Step : State → State → Prop where
  | reserve (s : State) (n : Nat) (positive : 0 < n)
      (available : s.allocated + n ≤ s.onHand) : Step s (reserve s n)
  | confirm (s : State) (before after : List Row) (r : Row)
      (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true) :
      Step s (replace s before after { r with status := .confirmed })
  | fulfil (s : State) (before after : List Row) (r : Row) (n : Nat)
      (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
      (positive : 0 < n) (within : n ≤ r.quantity) :
      Step s (fulfil s before after r n)
  | close (s : State) (before after : List Row) (r : Row) (status : Status)
      (row : s.rows = before ++ r :: after) (live : r.status.isOpen = true)
      (terminal : status = .released ∨ status = .expired) :
      Step s (close s before after r status)
  | splitConfirm (s : State) (before after : List Row) (r : Row) (n : Nat)
      (row : s.rows = before ++ r :: after)
      (splittable : r.status = .pending ∨ r.status = .allocated)
      (positive : 0 < n) (less : n < r.quantity) :
      Step s (splitConfirm s before after r n)
  | stutter (s : State) : Step s s

theorem initial_safe (capacity : Nat) : Safe capacity (initial capacity) := by
  simp [Safe, initial, openUnits]

theorem fulfilledRow_held (r : Row) (n : Nat) (live : r.status.isOpen = true) :
    (fulfilledRow r n).held = r.quantity - n := by
  by_cases full : n = r.quantity
  · simp [fulfilledRow, full, Row.held, Status.isOpen]
  · simp [fulfilledRow, full, Row.held, live]

theorem step_preserves_safe (capacity : Nat) (s t : State)
    (safe : Safe capacity s) (step : Step s t) : Safe capacity t := by
  rcases safe with ⟨ledger, bounds, conservation⟩
  cases step with
  | reserve n positive available =>
      simp only [Safe, reserve]
      simp [openUnits, Row.held, Status.isOpen] at ledger ⊢
      omega
  | confirm before after r row live =>
      rw [row, openUnits_context] at ledger
      simp [Row.held, live] at ledger
      simp [Safe, replace, openUnits_context, Row.held, Status.isOpen]
      exact ⟨ledger, bounds, conservation⟩
  | fulfil before after r n row live positive within =>
      rw [row, openUnits_context] at ledger
      simp [Row.held, live] at ledger
      simp only [Safe, fulfil, openUnits_context, fulfilledRow_held r n live]
      omega
  | close before after r status row live terminal =>
      rw [row, openUnits_context] at ledger
      simp [Row.held, live] at ledger
      have closed : status.isOpen = false := by
        rcases terminal with h | h <;> simp [h, Status.isOpen]
      simp [Safe, close, openUnits_context, Row.held, closed]
      omega
  | stutter => exact ⟨ledger, bounds, conservation⟩
  | splitConfirm before after r n row splittable positive less =>
      have live : r.status.isOpen = true := by
        rcases splittable with h | h <;> simp [h, Status.isOpen]
      rw [row, openUnits_context] at ledger
      simp [Row.held, live] at ledger
      simp only [Safe, splitConfirm, openUnits_context]
      simp only [Row.held, live, ↓reduceIte]
      simp [openUnits, Row.held, Status.isOpen] at ledger ⊢
      omega

/-- Executable specification of quantity confirmation, including no-op calls.
The due flag abstracts the repository's strict `expires_at < now` comparison. -/
def confirmQuantity (s : State) (before after : List Row) (r : Row)
    (n : Nat) (due : Bool) : State :=
  if r.status = .pending ∨ r.status = .allocated then
    if due then close s before after r .expired
    else if r.quantity ≤ n then replace s before after { r with status := .confirmed }
    else if 0 < n then splitConfirm s before after r n
    else s
  else s

def expireIfDue (s : State) (before after : List Row) (r : Row) (due : Bool) : State :=
  if r.status.isOpen ∧ due then close s before after r .expired else s

/-- Generated database expectations execute this function; it refines Step. -/
theorem confirmQuantity_step (s : State) (before after : List Row) (r : Row)
    (n : Nat) (due : Bool) (row : s.rows = before ++ r :: after) :
    Step s (confirmQuantity s before after r n due) := by
  unfold confirmQuantity
  split
  next splittable =>
    have live : r.status.isOpen = true := by
      rcases splittable with h | h <;> simp [h, Status.isOpen]
    split
    · exact Step.close s before after r .expired row live (Or.inr rfl)
    · split
      · exact Step.confirm s before after r row live
      · rename_i notFull
        split
        · next positive =>
            exact Step.splitConfirm s before after r n row splittable positive (by omega)
        · exact Step.stutter s
  next => exact Step.stutter s

theorem expireIfDue_step (s : State) (before after : List Row) (r : Row)
    (due : Bool) (row : s.rows = before ++ r :: after) :
    Step s (expireIfDue s before after r due) := by
  unfold expireIfDue
  split
  · next h => exact Step.close s before after r .expired row h.1 (Or.inr rfl)
  · exact Step.stutter s

theorem confirmQuantity_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (n : Nat) (due : Bool) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) :
    Safe capacity (confirmQuantity s before after r n due) :=
  step_preserves_safe capacity s _ safe (confirmQuantity_step s before after r n due row)

theorem expireIfDue_safe (capacity : Nat) (s : State) (before after : List Row)
    (r : Row) (due : Bool) (safe : Safe capacity s)
    (row : s.rows = before ++ r :: after) :
    Safe capacity (expireIfDue s before after r due) :=
  step_preserves_safe capacity s _ safe (expireIfDue_step s before after r due row)

inductive Reachable (capacity : Nat) : State → Prop where
  | init : Reachable capacity (initial capacity)
  | next {s t : State} : Reachable capacity s → Step s t → Reachable capacity t

/-- Safety for all reachable states, with no numeric or execution-length bound. -/
theorem reachable_safe (capacity : Nat) (s : State) (reachable : Reachable capacity s) :
    Safe capacity s := by
  induction reachable with
  | init => exact initial_safe capacity
  | next _ step ih => exact step_preserves_safe capacity _ _ ih step

theorem reachable_stock_bounds (capacity : Nat) (s : State)
    (reachable : Reachable capacity s) :
    s.allocated ≤ s.onHand ∧ s.onHand ≤ capacity := by
  have := reachable_safe capacity s reachable
  unfold Safe at this
  omega

theorem fulfilled_never_holds (quantity : Nat) :
    (Row.mk .fulfilled quantity).held = 0 := by
  rfl

/-- Reopening a fulfilled row without restoring its allocation breaks safety. -/
theorem reopening_fulfilled_breaks_ledger (capacity : Nat) (s : State)
    (before after : List Row) (quantity : Nat) (positive : 0 < quantity)
    (safe : Safe capacity s)
    (row : s.rows = before ++ ⟨.fulfilled, quantity⟩ :: after) :
    ¬ Safe capacity (replace s before after ⟨.confirmed, quantity⟩) := by
  intro broken
  have ledger := safe.1
  rw [row, openUnits_context] at ledger
  have wrong := broken.1
  simp [replace, openUnits_context, Row.held, Status.isOpen] at ledger wrong
  omega

/-- Releasing a fulfilled row can consume another live reservation's units. -/
theorem releasing_fulfilled_breaks_ledger (capacity : Nat) (s : State)
    (before after : List Row) (quantity : Nat) (positive : 0 < quantity)
    (within : quantity ≤ s.allocated) (safe : Safe capacity s)
    (row : s.rows = before ++ ⟨.fulfilled, quantity⟩ :: after) :
    ¬ Safe capacity (close s before after ⟨.fulfilled, quantity⟩ .released) := by
  intro broken
  have ledger := safe.1
  rw [row, openUnits_context] at ledger
  have wrong := broken.1
  simp [close, openUnits_context, Row.held, Status.isOpen] at ledger wrong
  omega

end InventoryReservations
