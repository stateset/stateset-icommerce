/-!
# Fulfillment pick quantities

The API may record fewer units than requested (a partial pick), but picked and
short units are nonnegative and may never claim more than requested. `Nat`
represents quantities at a common fixed scale; Rust uses exact Decimal values.
-/

namespace PickQuantity

def valid (requested picked short : Nat) : Prop :=
  picked ≤ requested ∧ short ≤ requested - picked

theorem valid_iff_total_bounded (requested picked short : Nat) :
    valid requested picked short ↔ picked + short ≤ requested := by
  simp only [valid]
  omega

theorem remaining_conserved (requested picked short : Nat)
    (h : valid requested picked short) :
    picked + short + (requested - picked - short) = requested := by
  have htotal := (valid_iff_total_bounded requested picked short).mp h
  omega

end PickQuantity
