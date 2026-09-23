/-!
# Subscription cycle discount cap

The cycle's rounded subtotal and discount are represented as nonnegative
minor units. This proves the cap and subtraction after rounding; it does not
prove Rust's decimal rounding algorithm or plan-change timing.
-/

namespace SubscriptionBilling

def discount (subtotal proposed : Nat) : Nat := min subtotal proposed
def total (subtotal proposed : Nat) : Nat := subtotal - discount subtotal proposed

theorem discount_bounded (subtotal proposed : Nat) :
    discount subtotal proposed ≤ subtotal := by
  exact Nat.min_le_left subtotal proposed

theorem cycle_conserved (subtotal proposed : Nat) :
    discount subtotal proposed + total subtotal proposed = subtotal := by
  unfold total
  have h := discount_bounded subtotal proposed
  omega

theorem excessive_discount_zeroes_total (subtotal proposed : Nat)
    (h : subtotal ≤ proposed) : total subtotal proposed = 0 := by
  simp [total, discount, Nat.min_eq_left h]

end SubscriptionBilling
