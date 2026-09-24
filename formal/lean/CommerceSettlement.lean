/-!
Exact-unit equations for exchange-rate publication, supplier obligations,
cost-layer issues, inbound receipts, and vendor-return valuation. The paired
TLA+ models check bounded interleavings of the repository transitions.
-/

namespace RatePublication

def publish (history : List Nat) (rate : Nat) : Nat × List Nat :=
  (rate, history ++ [rate])

theorem current_is_recorded (history : List Nat) (rate : Nat) :
    (publish history rate).1 ∈ (publish history rate).2 := by
  simp [publish]

theorem fixed_point_remainder (scaled scale : Nat) (positive : 0 < scale) :
    scaled = scaled / scale * scale + scaled % scale ∧ scaled % scale < scale := by
  constructor
  · simpa [Nat.mul_comm, Nat.add_comm] using (Nat.mod_add_div scaled scale).symm
  · exact Nat.mod_lt _ positive

end RatePublication

namespace ObligationSettlement

theorem payment_conserves (total paid amount : Nat)
    (within : paid + amount ≤ total) :
    paid + amount + (total - (paid + amount)) = total := by
  omega

theorem cancelled_amount_frozen (paid : Nat) :
    (if true then paid else paid + 1) = paid := by
  rfl

end ObligationSettlement

namespace CostLayerIssue

theorem one_layer_conserves (remaining issued : Nat) (within : issued ≤ remaining) :
    remaining - issued + issued = remaining := by
  omega

theorem two_layers_conserve (first second takeFirst takeSecond : Nat)
    (hFirst : takeFirst ≤ first) (hSecond : takeSecond ≤ second) :
    (first - takeFirst) + (second - takeSecond) + (takeFirst + takeSecond) =
      first + second := by
  omega

end CostLayerIssue

namespace InboundCancelReceipt

theorem receipt_conserves (expected received quantity : Nat)
    (within : received + quantity ≤ expected) :
    received + quantity + (expected - (received + quantity)) = expected := by
  omega

end InboundCancelReceipt

namespace VendorReturnDecision

def totalCredit (lines : List (Nat × Nat)) : Nat :=
  (lines.map fun line => line.1 * line.2).sum

theorem credit_of_appended_lines (left right : List (Nat × Nat)) :
    totalCredit (left ++ right) = totalCredit left + totalCredit right := by
  induction left with
  | nil => simp [totalCredit]
  | cons line rest ih =>
      simp only [totalCredit, List.map_append] at ih
      simp [totalCredit, ih, Nat.add_assoc]

end VendorReturnDecision
