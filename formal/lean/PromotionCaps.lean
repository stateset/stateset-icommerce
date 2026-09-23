/-!
# Promotion budget conservation

The cart's item-discount budget is its subtotal in exact minor units. Each
accepted item promotion consumes no more than its request or remaining budget.
This models the final caps after promotion eligibility and rounding have been
resolved; shipping discounts have a separate budget.
-/

namespace PromotionCaps

def applyOne (budget used request : Nat) : Nat :=
  used + min request (budget - used)

theorem step_bounded (budget used request : Nat) (h : used ≤ budget) :
    applyOne budget used request ≤ budget := by
  unfold applyOne
  have hm := Nat.min_le_right request (budget - used)
  omega

def stack (budget : Nat) (requests : List Nat) : Nat :=
  requests.foldl (applyOne budget) 0

theorem stack_bounded (budget : Nat) (requests : List Nat) :
    stack budget requests ≤ budget := by
  unfold stack
  have aux (xs : List Nat) (used : Nat) (h : used ≤ budget) :
      xs.foldl (applyOne budget) used ≤ budget := by
    induction xs generalizing used with
    | nil => simpa using h
    | cons x rest ih =>
      simp only [List.foldl_cons]
      exact ih (applyOne budget used x) (step_bounded budget used x h)
  exact aux requests 0 (Nat.zero_le budget)

theorem step_never_reverses (budget used request : Nat) :
    used ≤ applyOne budget used request := by
  unfold applyOne
  omega

theorem step_request_bound (budget used request : Nat) :
    applyOne budget used request - used ≤ request := by
  unfold applyOne
  have hm := Nat.min_le_left request (budget - used)
  omega

end PromotionCaps
