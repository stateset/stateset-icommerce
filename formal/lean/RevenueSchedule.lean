/-!
# Revenue schedule conservation

`generate_revenue_schedule` uses a capped per-period amount and a final
period plug. This model represents exact minor units as natural numbers;
`per` is the already-rounded amount for a normal period. The proof is
independent of how many periods there are or how `per` was rounded.
-/

namespace RevenueSchedule

/-- The amounts emitted by the ratable branch. `remaining` is the Rust
`amount - accumulated`; on the last period, emit all of it. -/
def ratable (per : Nat) : Nat → Nat → List Nat
  | 0, _ => []
  | n + 1, remaining =>
      if n = 0 then [remaining]
      else
        let first := min per remaining
        first :: ratable per n (remaining - first)

/-- The final plug conserves the exact amount, even if `per` is large enough
to exhaust it before the final period. -/
theorem ratable_sum (per remaining n : Nat) (hn : 0 < n) :
    (ratable per n remaining).sum = remaining := by
  induction n generalizing remaining with
  | zero => omega
  | succ n ih =>
    cases n with
    | zero => simp [ratable]
    | succ k =>
      have hstep : min per remaining ≤ remaining := Nat.min_le_right per remaining
      have hrec := ih (remaining - min per remaining) (by omega)
      change min per remaining +
        (ratable per (k + 1) (remaining - min per remaining)).sum = remaining
      rw [hrec]
      omega

/-- Recognized and deferred entries are a partition of the same schedule. -/
def recognized : List (Nat × Bool) → Nat
  | [] => 0
  | (amount, true) :: rest => amount + recognized rest
  | (_, false) :: rest => recognized rest

def deferred : List (Nat × Bool) → Nat
  | [] => 0
  | (_, true) :: rest => deferred rest
  | (amount, false) :: rest => amount + deferred rest

def amountSum (entries : List (Nat × Bool)) : Nat :=
  (entries.map Prod.fst).sum

/-- Moving an entry from deferred to recognized cannot create or lose money. -/
theorem recognized_add_deferred (entries : List (Nat × Bool)) :
    recognized entries + deferred entries = amountSum entries := by
  induction entries with
  | nil => simp [recognized, deferred, amountSum]
  | cons head tail ih =>
    rcases head with ⟨amount, flag⟩
    cases flag <;> simp [recognized, deferred, amountSum] at * <;> omega

end RevenueSchedule
