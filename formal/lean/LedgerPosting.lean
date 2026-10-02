/-!
# Posting balanced journal entries

Each line is represented by debit and credit minor units. A journal's net
change is the sum of debit-minus-credit; posting it to an already balanced
trial balance cannot change the trial balance's net when its debit and credit
totals match. This models arithmetic after line validation, not SQL posting.
-/

namespace LedgerPosting

abbrev Line := Nat × Nat

def debits (lines : List Line) : Nat := (lines.map Prod.fst).sum
def credits (lines : List Line) : Nat := (lines.map Prod.snd).sum
def net (lines : List Line) : Int :=
  (debits lines : Int) - (credits lines : Int)
def post (opening : Int) (lines : List Line) : Int := opening + net lines

private theorem sum_append_nat (xs ys : List Nat) :
    (xs ++ ys).sum = xs.sum + ys.sum := by
  induction xs with
  | nil => simp
  | cons x rest ih => simp [ih, Nat.add_assoc]

theorem balanced_net_zero (lines : List Line)
    (h : debits lines = credits lines) : net lines = 0 := by
  simp [net, h]

theorem posting_preserves_trial_balance (opening : Int) (lines : List Line)
    (h : debits lines = credits lines) : post opening lines = opening := by
  simp [post, balanced_net_zero lines h]

theorem append_balanced (left right : List Line)
    (hl : debits left = credits left)
    (hr : debits right = credits right) :
    debits (left ++ right) = credits (left ++ right) := by
  simp [debits, credits, List.map_append, sum_append_nat] at *
  omega

theorem reversal_balanced (lines : List Line)
    (h : debits lines = credits lines) :
    debits (lines.map Prod.swap) = credits (lines.map Prod.swap) := by
  simp [debits, credits, List.map_map, Function.comp_def] at *
  exact h.symm

end LedgerPosting
