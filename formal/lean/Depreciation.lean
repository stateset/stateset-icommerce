import RevenueSchedule

/-!
# Fixed-asset depreciation conservation

Amounts are exact nonnegative minor units. The normal-period amount is an
input that has already been rounded by the caller; the final plug consumes
the rest of the depreciable base without crossing salvage value.
-/

namespace Depreciation

def base (cost salvage : Nat) : Nat := cost - salvage

def schedule (cost salvage per periods : Nat) : List Nat :=
  RevenueSchedule.ratable per periods (base cost salvage)

theorem accumulated_eq_base (cost salvage per periods : Nat)
    (hperiods : 0 < periods) :
    (schedule cost salvage per periods).sum = base cost salvage := by
  exact RevenueSchedule.ratable_sum per (base cost salvage) periods hperiods

theorem final_book_is_salvage (cost salvage per periods : Nat)
    (hcost : salvage ≤ cost) (hperiods : 0 < periods) :
    cost - (schedule cost salvage per periods).sum = salvage := by
  rw [accumulated_eq_base cost salvage per periods hperiods]
  unfold base
  omega

end Depreciation
