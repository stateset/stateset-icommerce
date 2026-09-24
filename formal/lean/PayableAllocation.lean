/-!
# Accounts payable allocations

Amounts are nonnegative minor units. The payment-run transaction pays a bill's
current due amount under the same lock that records the payment allocation.
-/

namespace PayableAllocation

structure Bill where
  due : Nat
  paid : Nat
  deriving Repr

def allocate (bill : Bill) (request : Nat) : Bill :=
  let amount := min request bill.due
  { due := bill.due - amount, paid := bill.paid + amount }

theorem bill_conserved (bill : Bill) (request : Nat) :
    (allocate bill request).due + (allocate bill request).paid = bill.due + bill.paid := by
  unfold allocate
  dsimp
  have h := Nat.min_le_right request bill.due
  omega

theorem allocation_never_overpays (bill : Bill) (request : Nat) :
    (allocate bill request).paid - bill.paid ≤ bill.due := by
  unfold allocate
  dsimp
  omega

def allocateAll (bill : Bill) (requests : List Nat) : Bill :=
  requests.foldl allocate bill

theorem allocations_conserve_bill (bill : Bill) (requests : List Nat) :
    (allocateAll bill requests).due + (allocateAll bill requests).paid =
      bill.due + bill.paid := by
  unfold allocateAll
  have aux (xs : List Nat) (current : Bill) :
      (xs.foldl allocate current).due + (xs.foldl allocate current).paid =
        current.due + current.paid := by
    induction xs generalizing current with
    | nil => simp
    | cons x rest ih =>
      simp only [List.foldl_cons]
      rw [ih, bill_conserved]
  exact aux requests bill

end PayableAllocation
