/-!
Exact-unit accounting for five operational transitions. Amounts are natural
numbers at a shared minor-unit scale; the transition preconditions correspond
to the guards in the repository methods documented in formal/README.md.
-/

namespace CreditExposure

theorem reserve_preserves_available (limit balance holds amount : Nat)
    (h : balance + holds ≤ limit)
    (request : amount ≤ limit - balance - holds) :
    balance + (holds + amount) ≤ limit ∧
    limit - balance - (holds + amount) + balance + holds + amount = limit := by
  omega

theorem charge_converts_own_hold (limit balance own other amount : Nat)
    (h : balance + own + other ≤ limit)
    (charge : amount ≤ own) :
    balance + amount + (own - amount) + other ≤ limit := by
  omega

end CreditExposure

namespace SerialQuarantine

theorem quarantine_moves_only_sellable (available blocked quantity : Nat)
    (h : quantity ≤ available) :
    available - quantity + (blocked + quantity) = available + blocked := by
  omega

theorem block_all_sellable (available blocked : Nat) :
    available - available = 0 ∧ blocked + available = available + blocked := by
  omega

end SerialQuarantine

namespace ManufacturingYield

theorem report_good_and_scrap (planned good scrap remaining goodDelta scrapDelta : Nat)
    (h : good + scrap + remaining = planned)
    (bound : goodDelta + scrapDelta ≤ remaining) :
    good + goodDelta + (scrap + scrapDelta) +
      (remaining - goodDelta - scrapDelta) = planned := by
  omega

theorem completed_within_plan (planned completed quantity : Nat)
    (h : completed + quantity ≤ planned) :
    completed + quantity ≤ planned ∧
    completed + quantity + (planned - completed - quantity) = planned := by
  omega

end ManufacturingYield

namespace PaymentLedger

theorem payment_running_balance (balance amount : Nat)
    (bound : amount ≤ balance) :
    balance - amount + amount = balance := by
  omega

theorem overpayment_clamps_balance (balance amount : Nat)
    (h : balance ≤ amount) :
    balance - amount = 0 := by
  omega

end PaymentLedger

namespace ReturnDisposition

theorem restock_increases_sellable (onHand allocated quantity : Nat)
    (h : allocated ≤ onHand) :
    (onHand + quantity) - allocated = onHand - allocated + quantity := by
  omega

theorem quarantine_preserves_sellable (onHand allocated quantity : Nat)
    (h : allocated ≤ onHand) :
    (onHand + quantity) - (allocated + quantity) = onHand - allocated := by
  omega

end ReturnDisposition
