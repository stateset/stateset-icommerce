/-!
# Balanced FX revaluation journals

`build_revaluation_journal_lines` converts each signed adjustment to a
single-sided debit or credit and adds one opposite-side FX line for a
nonzero net. This model takes the already-oriented signed adjustments
(positive means debit, negative means credit) in exact minor units.
-/

namespace LedgerRevaluation

abbrev Line := Int × Int

def split (net : Int) : Line :=
  if 0 < net then (net, 0)
  else if net < 0 then (0, -net)
  else (0, 0)

def valid (line : Line) : Prop :=
  (0 < line.1 ∧ line.2 = 0) ∨ (line.1 = 0 ∧ 0 < line.2)

theorem split_delta (net : Int) : (split net).1 - (split net).2 = net := by
  by_cases hp : 0 < net
  · simp [split, hp]
  · by_cases hn : net < 0
    · simp [split, hp, hn]
    · simp [split, hp, hn]; omega

theorem split_valid (net : Int) (h : net ≠ 0) : valid (split net) := by
  by_cases hp : 0 < net
  · simp [split, valid, hp]
  · by_cases hn : net < 0
    · simp [split, valid, hp, hn]; omega
    · omega

/-- Zero adjustments are skipped, as in the Rust builder. -/
def adjustmentLines : List Int → List Line
  | [] => []
  | x :: xs => if x = 0 then adjustmentLines xs else split x :: adjustmentLines xs

theorem adjustment_lines_valid : ∀ (xs : List Int) (line : Line),
    line ∈ adjustmentLines xs → valid line := by
  intro xs
  induction xs with
  | nil => simp [adjustmentLines]
  | cons x tail ih =>
    intro line hmem
    by_cases hx : x = 0
    · simp [adjustmentLines, hx] at hmem
      exact ih line hmem
    · simp [adjustmentLines, hx] at hmem
      rcases hmem with h | h
      · rw [h]; exact split_valid x hx
      · exact ih line h

def debits : List Line → Int
  | [] => 0
  | line :: rest => line.1 + debits rest

def credits : List Line → Int
  | [] => 0
  | line :: rest => line.2 + credits rest

theorem debits_append (xs ys : List Line) :
    debits (xs ++ ys) = debits xs + debits ys := by
  induction xs with
  | nil => simp [debits]
  | cons x tail ih => simp [debits, ih, Int.add_assoc]

theorem credits_append (xs ys : List Line) :
    credits (xs ++ ys) = credits xs + credits ys := by
  induction xs with
  | nil => simp [credits]
  | cons x tail ih => simp [credits, ih, Int.add_assoc]

def addOffset (lines : List Line) : List Line :=
  let offset := credits lines - debits lines
  if offset = 0 then lines else lines ++ [split offset]

def journal (adjustments : List Int) : List Line :=
  addOffset (adjustmentLines adjustments)

theorem offset_balanced (lines : List Line) :
    debits (addOffset lines) = credits (addOffset lines) := by
  by_cases h : credits lines - debits lines = 0
  · simp [addOffset, h]
    omega
  · have hd := split_delta (credits lines - debits lines)
    simp [addOffset, h, debits_append, credits_append, debits, credits]
    omega

theorem journal_balanced (adjustments : List Int) :
    debits (journal adjustments) = credits (journal adjustments) := by
  exact offset_balanced _

theorem offset_lines_valid (lines : List Line)
    (hvalid : ∀ line ∈ lines, valid line) :
    ∀ line ∈ addOffset lines, valid line := by
  intro line hmem
  by_cases h : credits lines - debits lines = 0
  · simp [addOffset, h] at hmem
    exact hvalid line hmem
  · simp [addOffset, h] at hmem
    rcases hmem with hmem | hmem
    · exact hvalid line hmem
    · rw [hmem]; exact split_valid _ h

theorem journal_lines_valid (adjustments : List Int) :
    ∀ line ∈ journal adjustments, valid line := by
  exact offset_lines_valid _ (adjustment_lines_valid adjustments)

def reverse (line : Line) : Line := (line.2, line.1)

theorem debits_reverse (lines : List Line) :
    debits (lines.map reverse) = credits lines := by
  induction lines with
  | nil => rfl
  | cons line rest ih => simp [debits, credits, reverse, ih]

theorem credits_reverse (lines : List Line) :
    credits (lines.map reverse) = debits lines := by
  induction lines with
  | nil => rfl
  | cons line rest ih => simp [debits, credits, reverse, ih]

theorem reversal_balanced (adjustments : List Int) :
    debits ((journal adjustments).map reverse) =
      credits ((journal adjustments).map reverse) := by
  rw [debits_reverse, credits_reverse, journal_balanced]

end LedgerRevaluation
