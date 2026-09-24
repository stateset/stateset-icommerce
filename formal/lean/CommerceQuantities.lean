/-!
# Commerce quantity and value transitions

All quantities use Nat at a common exact scale. These prove the accepted
transition equations; the TLA+ models cover bounded interleavings.
-/

namespace MaterialConsumption

theorem accepted_within_reservation (reserved consumed delta : Nat)
    (h : consumed + delta ≤ reserved) :
    consumed + delta ≤ reserved ∧
    consumed + delta + (reserved - (consumed + delta)) = reserved := by
  omega

end MaterialConsumption

namespace StoredValue

theorem charge_conserved (opening charged refunded balance amount : Nat)
    (h : balance + charged = opening + refunded)
    (bound : amount ≤ balance) :
    balance - amount + (charged + amount) = opening + refunded := by
  omega

theorem refund_conserved (opening charged refunded balance amount : Nat)
    (h : balance + charged = opening + refunded) :
    balance + amount + charged = opening + (refunded + amount) := by
  omega

end StoredValue

namespace ReceivingPutAway

theorem completion_within_received (received planned quantity : Nat)
    (h : planned + quantity ≤ received) :
    planned + quantity ≤ received ∧
    planned + quantity + (received - (planned + quantity)) = received := by
  omega

theorem stock_and_movement_advance (stock movement quantity : Nat)
    (h : stock = quantity * movement) :
    stock + quantity = quantity * (movement + 1) := by
  simp [h, Nat.mul_add]

end ReceivingPutAway

namespace Backorder

theorem allocate_within_remaining (remaining allocated quantity : Nat)
    (h : allocated + quantity ≤ remaining) :
    allocated + quantity + (remaining - (allocated + quantity)) = remaining := by
  omega

theorem fulfill_conserved (ordered fulfilled remaining quantity : Nat)
    (h : fulfilled + remaining = ordered)
    (bound : quantity ≤ remaining) :
    fulfilled + quantity + (remaining - quantity) = ordered := by
  omega

end Backorder

namespace Receivable

theorem payment_conserved (invoice paid credited written balance amount : Nat)
    (h : paid + credited + written + balance = invoice)
    (bound : amount ≤ balance) :
    paid + amount + credited + written + (balance - amount) = invoice := by
  omega

theorem credit_conserved (invoice paid credited written balance amount : Nat)
    (h : paid + credited + written + balance = invoice)
    (bound : amount ≤ balance) :
    paid + (credited + amount) + written + (balance - amount) = invoice := by
  omega

theorem writeoff_conserved (invoice paid credited written balance : Nat)
    (h : paid + credited + written + balance = invoice) :
    paid + credited + (written + balance) = invoice := by
  omega

end Receivable
