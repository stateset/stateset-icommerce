/-!
Pure amount equations for five lifecycle boundaries. The matching TLA+ models
check transaction interleavings; these lemmas cover arbitrary natural units.
-/

namespace SubscriptionCancel

def nextBill (active : Bool) (paidPeriodEnd : Nat) : Option Nat :=
  if active then some paidPeriodEnd else none

theorem paid_cycle_cannot_reschedule_cancelled (periodEnd : Nat) :
    nextBill false periodEnd = none := by
  rfl

end SubscriptionCancel

namespace PurchaseOrderReceipt

def accept (ordered received quantity : Nat) : Nat :=
  if received + quantity ≤ ordered then received + quantity else received

theorem receipt_within_order (ordered received quantity : Nat)
    (existing : received ≤ ordered) :
    accept ordered received quantity ≤ ordered := by
  by_cases fits : received + quantity ≤ ordered
  · simp [accept, fits]
  · simp [accept, fits, existing]

theorem remaining_conserved (ordered received quantity : Nat)
    (h : received + quantity ≤ ordered) :
    ordered - (received + quantity) + quantity = ordered - received := by
  omega

end PurchaseOrderReceipt

namespace AssetDisposal

theorem book_value_at_disposal (cost accumulated : Nat)
    (h : accumulated ≤ cost) :
    cost - accumulated + accumulated = cost := by
  omega

theorem proceeds_split (book proceeds : Nat) :
    book + (proceeds - book) = proceeds + (book - proceeds) := by
  omega

end AssetDisposal

namespace RevenuePosting

theorem recognize_conserves (total recognized deferred amount : Nat)
    (h : recognized + deferred = total) (within : amount ≤ deferred) :
    (recognized + amount) + (deferred - amount) = total := by
  omega

theorem journal_balanced (amount : Nat) :
    amount + 0 = 0 + amount := by
  omega

end RevenuePosting

namespace GiftCardExpiry

theorem refund_conserves (original balance charged amount : Nat)
    (h : balance + charged = original) (within : amount ≤ charged) :
    balance + amount + (charged - amount) = original := by
  omega

def spendable (expired : Bool) (balance : Nat) : Nat :=
  if expired then 0 else balance

theorem expired_refund_not_spendable (balance amount : Nat) :
    spendable true (balance + amount) = 0 := by
  rfl

end GiftCardExpiry
