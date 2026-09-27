/-!
# Credit memo application

An application can consume at most the credit memo's unapplied amount and
the invoice's due amount. All values are exact nonnegative minor units.
-/

namespace CreditAllocation

structure State where
  available : Nat
  applied : Nat
  due : Nat
  deriving Repr

def apply (s : State) (request : Nat) : State :=
  let amount := min request (min s.available s.due)
  { available := s.available - amount
    applied := s.applied + amount
    due := s.due - amount }

/-- The Rust mutation accepts a request only when it fits both balances. -/
def applyAccepted (s : State) (request : Nat) : State :=
  { available := s.available - request
    applied := s.applied + request
    due := s.due - request }

theorem accepted_matches_cap (s : State) (request : Nat)
    (ha : request ≤ s.available) (hd : request ≤ s.due) :
    applyAccepted s request = apply s request := by
  have hinner : request ≤ min s.available s.due := by omega
  simp [applyAccepted, apply, Nat.min_eq_left hinner]

theorem credit_conserved (s : State) (request : Nat) :
    (apply s request).available + (apply s request).applied =
      s.available + s.applied := by
  unfold apply
  dsimp
  have h := Nat.min_le_left request (min s.available s.due)
  have ha := Nat.min_le_left s.available s.due
  omega

theorem invoice_never_overpaid (s : State) (request : Nat) :
    (apply s request).due ≤ s.due := by
  unfold apply
  dsimp
  omega

theorem accepted_credit_conserved (s : State) (request : Nat)
    (ha : request ≤ s.available) (hd : request ≤ s.due) :
    (applyAccepted s request).available + (applyAccepted s request).applied =
      s.available + s.applied := by
  rw [accepted_matches_cap s request ha hd]
  exact credit_conserved s request

def applyAll (s : State) (requests : List Nat) : State :=
  requests.foldl apply s

theorem applications_conserve_credit (s : State) (requests : List Nat) :
    (applyAll s requests).available + (applyAll s requests).applied =
      s.available + s.applied := by
  unfold applyAll
  have aux (xs : List Nat) (current : State) :
      (xs.foldl apply current).available + (xs.foldl apply current).applied =
        current.available + current.applied := by
    induction xs generalizing current with
    | nil => simp
    | cons x rest ih =>
      simp only [List.foldl_cons]
      rw [ih, credit_conserved]
  exact aux requests s

end CreditAllocation
