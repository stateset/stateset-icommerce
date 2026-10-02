import Allocation

/-!
Golden vectors for `allocate_rounded`, computed by the proved model in
`Allocation.lean`. Run by `formal/lean/check.sh`; the Rust test
`allocate_rounded_matches_the_lean_model` holds the engine to the output.

Parts are 4-place decimals rounded to 2 places (`f = 100`), and every total
is the sum of its parts, the precondition the theorems assume.
-/

open Allocation

def strategies : List (String × Strategy) :=
  [("MidpointNearestEven", .midpointNearestEven),
   ("MidpointAwayFromZero", .midpointAwayFromZero),
   ("MidpointTowardZero", .midpointTowardZero),
   ("ToZero", .toZero),
   ("AwayFromZero", .awayFromZero),
   ("ToNegativeInfinity", .toNegativeInfinity),
   ("ToPositiveInfinity", .toPositiveInfinity)]

/-- Hand-picked cases: the documented three `$1.11` lines at 8.25%, exact
midpoints of both signs, ties on remainder, refunds (negative), and mixed
signs. -/
def fixed : List (List Int) :=
  [[916, 916, 916],
   [50, 50, 50],
   [-50, -50, -50],
   [150, 250, 350, -50],
   [3333, 3333, 3334],
   [-3333, -3333, -3334],
   [49, 49, 49, 49, 49, 49, 49],
   [51, -49, 151, -151, 1],
   [0, 0, 0],
   [12345]]

/-- A small linear congruential generator, so the vectors are reproducible
without any library. -/
def lcg (seed : Nat) : Nat := (seed * 1103515245 + 12345) % 2147483648

def randomCases : Nat → Nat → List (List Int)
  | 0, _ => []
  | n + 1, seed =>
    let s1 := lcg seed
    let len := s1 % 7 + 1
    let rec parts : Nat → Nat → List Int × Nat
      | 0, s => ([], s)
      | k + 1, s =>
        let s' := lcg s
        -- mostly small amounts, a quarter of them negative
        let mag : Int := Int.ofNat (s' / 8 % 20000)
        let v := if s' % 4 = 0 then -mag else mag
        let (rest, s'') := parts k s'
        (v :: rest, s'')
    let (ps, s2) := parts len s1
    ps :: randomCases n s2

/-- Render `x` fine units as a decimal with `dp` places. -/
def dec (dp : Nat) (x : Int) : String :=
  let sign := if x < 0 then "-" else ""
  let a := x.natAbs
  let scale := 10 ^ dp
  let frac := toString (a % scale)
  let pad := String.mk (List.replicate (dp - frac.length) '0')
  sign ++ toString (a / scale) ++ (if dp = 0 then "" else "." ++ pad ++ frac)

def quote (s : String) : String := "\"" ++ s ++ "\""

def jsonList (xs : List String) : String := "[" ++ ", ".intercalate xs ++ "]"

def main : IO Unit := do
  let cases := fixed ++ randomCases 60 20260923
  let mut lines : List String := []
  for (name, s) in strategies do
    for parts in cases do
      let total := parts.sum
      let (rt, rp) := allocate s 100 total parts
      lines := lines ++ [
        "{\"strategy\": " ++ quote name ++
        ", \"total\": " ++ quote (dec 4 total) ++
        ", \"parts\": " ++ jsonList (parts.map (quote ∘ dec 4)) ++
        ", \"expected_total\": " ++ quote (dec 2 rt) ++
        ", \"expected_parts\": " ++ jsonList (rp.map (quote ∘ dec 2)) ++ "}"]
  IO.println ("[\n  " ++ ",\n  ".intercalate lines ++ "\n]")
