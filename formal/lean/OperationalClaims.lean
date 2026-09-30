/-!
Exact-unit arithmetic for cycle counts, transfer receipts, warranty slots,
vendor credit applications, and x402 credit balances. The TLA+ models check
bounded concurrent traces; these theorems cover arbitrary natural quantities
under each transition's acceptance guard.
-/

namespace CycleCount

theorem variance_reconciles (current expected counted : Nat)
    (nonnegative : expected ≤ current + counted) :
    current + counted - expected + expected = current + counted := by
  omega

theorem zero_variance_leaves_stock (current expected : Nat) :
    current + expected - expected = current := by
  omega

end CycleCount

namespace TransferReceipt

theorem accepted_within_shipped (shipped received delta : Nat)
    (h : received + delta ≤ shipped) :
    received + delta ≤ shipped ∧
    received + delta + (shipped - (received + delta)) = shipped := by
  omega

theorem line_totals (left right leftDelta rightDelta : Nat) :
    (left + leftDelta) + (right + rightDelta) =
      (left + right) + (leftDelta + rightDelta) := by
  omega

end TransferReceipt

namespace WarrantyClaims

theorem claim_consumes_one_slot (limit used : Nat)
    (h : used < limit) :
    used + 1 ≤ limit ∧
    used + 1 + (limit - (used + 1)) = limit := by
  omega

end WarrantyClaims

namespace VendorCredit

theorem apply_conserves (original remaining applied amount : Nat)
    (h : remaining + applied = original)
    (bound : amount ≤ remaining) :
    remaining - amount + (applied + amount) = original := by
  omega

theorem reverse_conserves (original remaining applied amount : Nat)
    (h : remaining + applied = original)
    (bound : amount ≤ applied) :
    remaining + amount + (applied - amount) = original := by
  omega

end VendorCredit

namespace X402Credit

theorem debit_conserves (opening credits debits balance amount : Nat)
    (h : balance + debits = opening + credits)
    (bound : amount ≤ balance) :
    balance - amount + (debits + amount) = opening + credits := by
  omega

theorem credit_conserves (opening credits debits balance amount : Nat)
    (h : balance + debits = opening + credits) :
    balance + amount + debits = opening + (credits + amount) := by
  omega

end X402Credit
