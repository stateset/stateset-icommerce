/-! Selection from line-keyed holds, in the backend-supplied row order.
Natural quantities represent exact decimal units at one common scale.
Legacy unkeyed holds and planner admission checks are outside these proofs. -/
namespace ReservationOwnership

structure Hold where
  owner : Nat
  quantity : Nat
  deriving Repr

def held (owner : Nat) : List Hold → Nat
  | [] => 0
  | row :: rest => (if row.owner = owner then row.quantity else 0) + held owner rest

def remaining (request : Nat → Nat) (row : Hold) : Nat → Nat :=
  fun owner => if owner = row.owner then request owner - min row.quantity (request owner)
              else request owner

/-- Keep zero selections to preserve row positions. Rust omits those rows. -/
def select (request : Nat → Nat) : List Hold → List Hold
  | [] => []
  | row :: rest => ⟨row.owner, min row.quantity (request row.owner)⟩ ::
      select (remaining request row) rest

/-- Selection consumes exactly the lesser of each line's request and its own
holds, for arbitrary line IDs, row counts, quantities, and row ordering. -/
theorem selected_per_owner (rows : List Hold) (request : Nat → Nat) (owner : Nat) :
    held owner (select request rows) = min (request owner) (held owner rows) := by
  induction rows generalizing request with
  | nil => simp [select, held]
  | cons row rest ih =>
      by_cases same : row.owner = owner
      · simp [select, held, ih, remaining, same]
        omega
      · simp [select, held, ih, remaining, same, Ne.symm same]

theorem selected_within_request (rows : List Hold) (request : Nat → Nat) (owner : Nat) :
    held owner (select request rows) ≤ request owner := by
  rw [selected_per_owner]
  exact Nat.min_le_left _ _

theorem covered_request_filled (rows : List Hold) (request : Nat → Nat) (owner : Nat)
    (covered : request owner ≤ held owner rows) :
    held owner (select request rows) = request owner := by
  rw [selected_per_owner, Nat.min_eq_left covered]

theorem unrequested_owner_untouched (rows : List Hold) (request : Nat → Nat)
    (owner : Nat) (unrequested : request owner = 0) :
    held owner (select request rows) = 0 := by
  simp [selected_per_owner, unrequested]

/-- Output positions retain ownership and cannot exceed the matching input row. -/
def Bounded : List Hold → List Hold → Prop
  | [], [] => True
  | chosen :: choices, original :: originals =>
      chosen.owner = original.owner ∧ chosen.quantity ≤ original.quantity ∧
      Bounded choices originals
  | _, _ => False

theorem rows_bounded (rows : List Hold) (request : Nat → Nat) :
    Bounded (select request rows) rows := by
  induction rows generalizing request with
  | nil => trivial
  | cons row rest ih =>
      exact ⟨rfl, Nat.min_le_left _ _, ih _⟩

/-- Equal SKU totals cannot replace line coverage: if a line is short, the
selection cannot meet its request regardless of another line's surplus. -/
theorem sibling_surplus_cannot_cover (rows : List Hold) (request : Nat → Nat)
    (owner : Nat) (short : held owner rows < request owner) :
    held owner (select request rows) < request owner := by
  rw [selected_per_owner, Nat.min_eq_right (Nat.le_of_lt short)]
  exact short

end ReservationOwnership
