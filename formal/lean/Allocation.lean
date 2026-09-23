/-!
# Allocating rounded money, proved

A model of `stateset_core::allocate_rounded`
(`crates/stateset-core/src/models/tax.rs`), which both tax engines use to
round per line while keeping the lines summing exactly to the rounded total.

Amounts are integers in *fine* units, rounded to *coarse* units by a scale
`f > 0`: rounding a 4-place decimal to 2 places is `f = 100`. That is exact
for the finite decimals the engine stores, so nothing is lost by working in
integers.
-/

namespace Allocation

/-- `rust_decimal::RoundingStrategy`, every variant the tax engine can reach
through `TaxSettings::rounding_strategy`. -/
inductive Strategy where
  | midpointNearestEven
  | midpointAwayFromZero
  | midpointTowardZero
  | toZero
  | awayFromZero
  | toNegativeInfinity
  | toPositiveInfinity
  deriving Repr, DecidableEq

/-- Round `x / f` to an integer under strategy `s`. With `f > 0`, Lean's `/`
and `%` on `Int` are floor division and a remainder in `[0, f)`. -/
def round (s : Strategy) (f x : Int) : Int :=
  let q := x / f
  let r := x % f
  match s with
  | .toNegativeInfinity => q
  | .toPositiveInfinity => if r = 0 then q else q + 1
  | .toZero => if 0 ≤ x then q else if r = 0 then q else q + 1
  | .awayFromZero => if 0 ≤ x then (if r = 0 then q else q + 1) else q
  | .midpointAwayFromZero =>
      if 2 * r < f then q else if f < 2 * r then q + 1 else if 0 ≤ x then q + 1 else q
  | .midpointTowardZero =>
      if 2 * r < f then q else if f < 2 * r then q + 1 else if 0 ≤ x then q else q + 1
  | .midpointNearestEven =>
      if 2 * r < f then q else if f < 2 * r then q + 1 else if q % 2 = 0 then q else q + 1

/-- Every strategy lands on the floor, or on the floor plus one when there is a
remainder to round away. -/
theorem round_floor_or_next (s : Strategy) (f x : Int) (hf : 0 < f) :
    round s f x = x / f ∨ (round s f x = x / f + 1 ∧ x % f ≠ 0) := by
  have hr0 : 0 ≤ x % f := Int.emod_nonneg x (by omega)
  cases s <;> simp only [round] <;> (repeat' split) <;> first | omega | simp

/-- So every strategy lands strictly within one coarse unit of the exact
value: `|round(x) * f - x| < f`. This is the only fact about rounding the
rest of the file needs, which is why its theorems hold for all seven
strategies at once. -/
theorem round_within (s : Strategy) (f x : Int) (hf : 0 < f) :
    x - f < round s f x * f ∧ round s f x * f < x + f := by
  have hd := Int.ediv_add_emod x f
  have hr0 : 0 ≤ x % f := Int.emod_nonneg x (by omega)
  have hr1 : x % f < f := Int.emod_lt_of_pos x hf
  rcases round_floor_or_next s f x hf with h | ⟨h, hne⟩
  · rw [h, Int.mul_comm]; omega
  · rw [h, Int.add_mul, Int.mul_comm]; omega

/-- How far the sum of the individually rounded parts is from the rounded
total. `allocate_rounded` must hand out exactly this many coarse units. -/
def residue (s : Strategy) (f total : Int) (parts : List Int) : Int :=
  round s f total - (parts.map (round s f)).sum

/-- Summed over `n` parts, rounding moves the total by at most `n * (f - 1)`
fine units: each part contributes at most `f - 1`. -/
theorem rounding_error_sum (s : Strategy) (f : Int) (hf : 0 < f) (parts : List Int) :
    -((parts.length : Int) * f) + parts.length
        ≤ (parts.map (round s f)).sum * f - parts.sum ∧
    (parts.map (round s f)).sum * f - parts.sum
        ≤ (parts.length : Int) * f - parts.length := by
  induction parts with
  | nil => simp
  | cons p ps ih =>
    have hp := round_within s f p hf
    have hlen : ((ps.length + 1 : Nat) : Int) * f = (ps.length : Int) * f + f := by
      rw [Int.ofNat_add, Int.add_mul, Int.ofNat_one, Int.one_mul]
    simp only [List.map_cons, List.sum_cons, List.length_cons, Int.add_mul, hlen]
    omega

/-- **The residue is at most the number of parts**, whenever the total is the
exact sum of the parts -- which is how the tax engine calls it
(`raw_total = raw.iter().sum()`).

`allocate_rounded` hands out the residue one coarse unit at a time, cycling
through the parts, and stops after `parts.len() * 1000` steps. This theorem
is why that cap is safe: the loop needs at most `n` steps, so for any
`n ≥ 1` it finishes long before the cap and never exits with residue left
over. A caller passing a total that is NOT the sum of its parts has no such
guarantee -- the function is `pub`, so that precondition is the caller's to
keep. -/
theorem residue_le_length (s : Strategy) (f : Int) (hf : 0 < f)
    (parts : List Int) (total : Int) (htotal : total = parts.sum) :
    -(parts.length : Int) ≤ residue s f total parts ∧
    residue s f total parts ≤ parts.length := by
  have ht := round_within s f total hf
  have he := rounding_error_sum s f hf parts
  have hmul : residue s f total parts * f
      = round s f total * f - (parts.map (round s f)).sum * f := by
    simp only [residue, Int.sub_mul]
  have hlen : ((parts.length : Int) + 1) * f = (parts.length : Int) * f + f := by
    rw [Int.add_mul, Int.one_mul]
  have hneg : (-((parts.length : Int) + 1)) * f = -((parts.length : Int) * f) - f := by
    rw [Int.neg_mul, hlen, Int.neg_add]; omega
  constructor
  · have : (-((parts.length : Int) + 1)) * f < residue s f total parts * f := by
      rw [hneg, hmul]; omega
    have := Int.lt_of_mul_lt_mul_right this (by omega)
    omega
  · have : residue s f total parts * f < ((parts.length : Int) + 1) * f := by
      rw [hlen, hmul]; omega
    have := Int.lt_of_mul_lt_mul_right this (by omega)
    omega

/-! ## Handing out the residue

`allocate_rounded` orders the parts by how much rounding took from them
(largest first when the residue is positive, smallest first when negative)
with a stable sort, then nudges them one coarse unit each. Repeatedly marking
the first unmarked part that holds the best remainder picks exactly the same
parts -- best remainder first, lowest index on ties -- and keeps every proof
free of index arithmetic. -/

/-- How many positions are marked. -/
def marked : List Bool → Nat
  | [] => 0
  | b :: bs => (if b then 1 else 0) + marked bs

/-- The best remainder among unmarked positions: largest when `desc`,
otherwise smallest. `none` when every position is marked. -/
def bestVal (desc : Bool) : List Int → List Bool → Option Int
  | v :: vs, b :: bs =>
    if b then bestVal desc vs bs
    else match bestVal desc vs bs with
      | none => some v
      | some w => some (if desc then max v w else min v w)
  | _, _ => none

/-- Mark the first unmarked position that holds `t`. -/
def markFirst (t : Int) : List Int → List Bool → List Bool
  | v :: vs, b :: bs => if b = false ∧ v = t then true :: bs else b :: markFirst t vs bs
  | _, bs => bs

/-- Mark `k` positions, best remainder first. -/
def select (desc : Bool) (rem : List Int) : Nat → List Bool → List Bool
  | 0, flags => flags
  | k + 1, flags =>
    match bestVal desc rem flags with
    | none => flags
    | some t => select desc rem k (markFirst t rem flags)

/-- Add `step` to every marked part. -/
def nudge (step : Int) : List Int → List Bool → List Int
  | r :: rs, b :: bs => (if b then r + step else r) :: nudge step rs bs
  | rs, [] => rs
  | [], _ :: _ => []

theorem marked_le_length : ∀ bs : List Bool, marked bs ≤ bs.length
  | [] => by simp [marked]
  | b :: bs => by
    have := marked_le_length bs
    cases b <;> simp [marked] <;> omega

theorem markFirst_length (t : Int) :
    ∀ (vs : List Int) (bs : List Bool), (markFirst t vs bs).length = bs.length
  | v :: vs, b :: bs => by
    simp only [markFirst]; split
    · simp
    · simp [markFirst_length t vs bs]
  | [], bs => by simp [markFirst]
  | _ :: _, [] => by simp [markFirst]

/-- Whatever `bestVal` returns, some unmarked position holds it, so
`markFirst` marks exactly one more position. -/
theorem markFirst_marks_one (desc : Bool) :
    ∀ (vs : List Int) (bs : List Bool) (t : Int),
      bestVal desc vs bs = some t → marked (markFirst t vs bs) = marked bs + 1
  | v :: vs, b :: bs, t, h => by
    cases b with
    | true =>
      simp only [bestVal, if_true] at h
      have := markFirst_marks_one desc vs bs t h
      simp [markFirst, marked, this]; omega
    | false =>
      cases hrest : bestVal desc vs bs with
      | none =>
        -- nothing unmarked further on: t is this position's value
        simp [bestVal, hrest] at h; subst h; simp [markFirst, marked]; omega
      | some w =>
        simp [bestVal, hrest] at h
        by_cases hv : v = t
        · subst hv; simp [markFirst, marked]; omega
        · -- the best is further on, so it is the tail's best
          have hwt : w = t := by
            cases desc <;> simp [Int.max_def, Int.min_def] at h <;> split at h <;> omega
          subst hwt
          have := markFirst_marks_one desc vs bs w hrest
          simp [markFirst, marked, hv, this]
  | [], bs, t, h => by cases bs <;> simp [bestVal] at h
  | _ :: _, [], t, h => by simp [bestVal] at h

/-- While something is unmarked, `bestVal` finds it. -/
theorem bestVal_some (desc : Bool) :
    ∀ (vs : List Int) (bs : List Bool), vs.length = bs.length →
      marked bs < bs.length → ∃ t, bestVal desc vs bs = some t
  | v :: vs, b :: bs, hl, hm => by
    simp only [List.length_cons] at hl
    cases b with
    | true =>
      simp [marked] at hm
      obtain ⟨t, ht⟩ := bestVal_some desc vs bs (by omega) (by omega)
      exact ⟨t, by simp [bestVal, ht]⟩
    | false =>
      cases hrest : bestVal desc vs bs with
      | none => exact ⟨v, by simp [bestVal, hrest]⟩
      | some w => exact ⟨if desc then max v w else min v w, by simp [bestVal, hrest]⟩
  | [], [], _, hm => by simp [marked] at hm
  | [], _ :: _, hl, _ => by simp at hl
  | _ :: _, [], hl, _ => by simp at hl

/-- Selecting `k` marks exactly `k` more positions, as long as that many are
still unmarked. -/
theorem select_marks (desc : Bool) (rem : List Int) :
    ∀ (k : Nat) (flags : List Bool), rem.length = flags.length →
      marked flags + k ≤ flags.length →
      marked (select desc rem k flags) = marked flags + k ∧
      (select desc rem k flags).length = flags.length
  | 0, flags, _, _ => by simp [select]
  | k + 1, flags, hl, hk => by
    obtain ⟨t, ht⟩ := bestVal_some desc rem flags hl (by omega)
    have hm := markFirst_marks_one desc rem flags t ht
    have hlen := markFirst_length t rem flags
    have ih := select_marks desc rem k (markFirst t rem flags) (by omega) (by omega)
    simp only [select, ht]
    omega

theorem nudge_sum (step : Int) :
    ∀ (rs : List Int) (bs : List Bool), rs.length = bs.length →
      (nudge step rs bs).sum = rs.sum + step * marked bs
  | r :: rs, b :: bs, hl => by
    simp only [List.length_cons] at hl
    have ih := nudge_sum step rs bs (by omega)
    cases b <;> simp [nudge, marked, ih, Int.mul_add] <;> omega
  | [], [], _ => by simp [nudge, marked]
  | [], _ :: _, hl => by simp at hl
  | _ :: _, [], hl => by simp at hl

/-! ## `allocate_rounded`, and what it guarantees -/

/-- `stateset_core::allocate_rounded`, step for step: round the total and each
part, then hand the residue out one coarse unit at a time, best remainder
first, lowest index on ties. (The Rust loop also stops after `n * 1000`
steps; `residue_le_length` shows it never gets there, so the model omits it.) -/
def allocate (s : Strategy) (f total : Int) (parts : List Int) : Int × List Int :=
  let rt := round s f total
  let rp := parts.map (round s f)
  let d := rt - rp.sum
  if d = 0 then (rt, rp)
  else
    let rem := List.zipWith (fun p r => p - r * f) parts rp
    let flags := select (decide (0 < d)) rem d.natAbs (rp.map fun _ => false)
    (rt, nudge (if 0 < d then 1 else -1) rp flags)

theorem marked_all_false : ∀ rs : List Int, marked (rs.map fun _ => false) = 0
  | [] => rfl
  | _ :: rs => by simp [marked, marked_all_false rs]

/-- **The allocated parts sum exactly to the rounded total** -- the contract
`allocate_rounded` documents, now proved for every rounding strategy
whenever the total is the sum of its parts. -/
theorem allocate_sum (s : Strategy) (f : Int) (hf : 0 < f)
    (parts : List Int) (total : Int) (htotal : total = parts.sum) :
    (allocate s f total parts).2.sum = (allocate s f total parts).1 := by
  have hb := residue_le_length s f hf parts total htotal
  have hlen : (parts.map (round s f)).length = parts.length := by simp
  have hreml : (List.zipWith (fun p r => p - r * f) parts (parts.map (round s f))).length
      = ((parts.map (round s f)).map fun _ => false).length := by
    simp [List.length_zipWith]
  simp only [residue] at hb
  simp only [allocate]
  generalize List.zipWith (fun p r => p - r * f) parts (parts.map (round s f)) = rem at *
  generalize parts.map (round s f) = rp at *
  generalize round s f total = rt at *
  split
  · simp; omega
  · rename_i hd
    have hsel := select_marks (decide (0 < rt - rp.sum)) rem (rt - rp.sum).natAbs
      (rp.map fun _ => false) hreml (by rw [marked_all_false]; simp; omega)
    rw [marked_all_false] at hsel
    have hsum := nudge_sum (if 0 < rt - rp.sum then 1 else -1) rp
      (select (decide (0 < rt - rp.sum)) rem (rt - rp.sum).natAbs (rp.map fun _ => false))
      (by rw [hsel.2]; simp)
    rw [hsum, hsel.1]
    split <;> omega

/-- Each output is its own rounding, or that moved by one coarse unit. -/
def Near : List Int → List Int → Prop
  | o :: os, r :: rs => (r - 1 ≤ o ∧ o ≤ r + 1) ∧ Near os rs
  | [], [] => True
  | _, _ => False

theorem nudge_near (step : Int) (hs : step = 1 ∨ step = -1) :
    ∀ (rs : List Int) (bs : List Bool), rs.length = bs.length → Near (nudge step rs bs) rs
  | r :: rs, b :: bs, hl => by
    simp only [List.length_cons] at hl
    have ih := nudge_near step hs rs bs (by omega)
    cases b <;> simp [nudge, Near, ih] <;> omega
  | [], [], _ => by simp [nudge, Near]
  | [], _ :: _, hl => by simp at hl
  | _ :: _, [], hl => by simp at hl

theorem near_refl : ∀ rs : List Int, Near rs rs
  | [] => by simp [Near]
  | r :: rs => by simp [Near, near_refl rs]; omega

/-- **No part moves more than one coarse unit from its own rounding.** The
residue is spread so thinly that every line's tax stays within a cent of
what rounding that line alone would give. -/
theorem allocate_near (s : Strategy) (f : Int) (hf : 0 < f)
    (parts : List Int) (total : Int) (htotal : total = parts.sum) :
    Near (allocate s f total parts).2 (parts.map (round s f)) := by
  have hb := residue_le_length s f hf parts total htotal
  have hlen : (parts.map (round s f)).length = parts.length := by simp
  have hreml : (List.zipWith (fun p r => p - r * f) parts (parts.map (round s f))).length
      = ((parts.map (round s f)).map fun _ => false).length := by
    simp [List.length_zipWith]
  simp only [residue] at hb
  simp only [allocate]
  generalize List.zipWith (fun p r => p - r * f) parts (parts.map (round s f)) = rem at *
  generalize parts.map (round s f) = rp at *
  generalize round s f total = rt at *
  split
  · exact near_refl _
  · have hsel := select_marks (decide (0 < rt - rp.sum)) rem (rt - rp.sum).natAbs
      (rp.map fun _ => false) hreml (by rw [marked_all_false]; simp; omega)
    exact nudge_near _ (by split <;> simp) rp _ (by rw [hsel.2]; simp)

end Allocation
