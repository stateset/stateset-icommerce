/-! Durable preview promotion for one idempotency key. Atomic execution,
collision-free semantic fingerprints and trustworthy seal verification are
assumptions. Guards abstract current policy, deadlines and optimistic versions.
No external side effects or crash-recovery implementation are proved here. -/
namespace ReceiptPromotion

inductive Status where
  | previewed | succeeded | rejected | failed
  deriving DecidableEq, Repr

inductive Decision where
  | tampered | conflict | promote | replay
  deriving DecidableEq, Repr

/-- The ordering in `kernel::replay::resolve_replay`: authenticate first. -/
def decide (status : Status) (apply same sealed : Bool) : Decision :=
  if !sealed then .tampered
  else if !same then .conflict
  else if status == .previewed && apply then .promote
  else .replay

theorem promotion_requires (status : Status) (apply same sealed : Bool)
    (h : decide status apply same sealed = .promote) :
    status = .previewed ∧ apply = true ∧ same = true ∧ sealed = true := by
  cases status <;> cases apply <;> cases same <;> cases sealed <;> simp_all [decide]

theorem tamper_precedes_conflict (status : Status) (apply same : Bool) :
    decide status apply same false = .tampered := by simp [decide]

structure State where
  receipt : Option Status
  fingerprint : Nat
  effects : Nat
  deriving DecidableEq, Repr

def initial : State := ⟨none, 0, 0⟩

def Safe (s : State) : Prop :=
  s.effects = if s.receipt = some .succeeded then 1 else 0

/-- Policy/version guards are evaluated again on every fresh/promoted apply. -/
def execute (s : State) (fingerprint : Nat) (apply allowed : Bool) : State :=
  if allowed then
    if apply then ⟨some .succeeded, fingerprint, s.effects + 1⟩
    else ⟨some .previewed, fingerprint, s.effects⟩
  else ⟨some .rejected, fingerprint, s.effects⟩

def invoke (s : State) (fingerprint : Nat) (apply allowed sealed : Bool) : State :=
  match s.receipt with
  | none => execute s fingerprint apply allowed
  | some status =>
    if decide status apply (s.fingerprint == fingerprint) sealed = .promote
    then execute s fingerprint apply allowed
    else s

theorem execute_safe (s : State) (fingerprint : Nat) (apply allowed : Bool)
    (empty : s.effects = 0) : Safe (execute s fingerprint apply allowed) := by
  cases apply <;> cases allowed <;> simp [execute, Safe, empty]

theorem invoke_safe (s : State) (fingerprint : Nat) (apply allowed sealed : Bool)
    (safe : Safe s) : Safe (invoke s fingerprint apply allowed sealed) := by
  unfold invoke
  split
  · next absent =>
      apply execute_safe
      simpa [Safe, absent] using safe
  · next status present =>
      split
      · next promoted =>
          have h := promotion_requires status apply (s.fingerprint == fingerprint) sealed promoted
          apply execute_safe
          simpa [Safe, present, h.1] using safe
      · exact safe

inductive Reachable : State → Prop where
  | init : Reachable initial
  | next {s : State} (fingerprint : Nat) (apply allowed sealed : Bool) :
      Reachable s → Reachable (invoke s fingerprint apply allowed sealed)

theorem reachable_safe (s : State) (h : Reachable s) : Safe s := by
  induction h with
  | init => simp [Safe, initial]
  | next fingerprint apply allowed sealed _ ih => exact invoke_safe _ _ _ _ _ ih

theorem at_most_once (s : State) (h : Reachable s) : s.effects ≤ 1 := by
  have safe := reachable_safe s h
  unfold Safe at safe
  split at safe <;> omega

theorem denial_has_no_effect (s : State) (fingerprint : Nat) (apply sealed : Bool) :
    (invoke s fingerprint apply false sealed).effects = s.effects := by
  unfold invoke
  split
  · simp [execute]
  · split <;> simp [execute]

theorem terminal_is_stable (s : State) (status : Status) (fingerprint : Nat)
    (apply allowed sealed : Bool) (present : s.receipt = some status)
    (terminal : status ≠ .previewed) :
    invoke s fingerprint apply allowed sealed = s := by
  simp only [invoke, present]
  have noPromotion : decide status apply (s.fingerprint == fingerprint) sealed ≠ .promote := by
    intro h
    exact terminal (promotion_requires _ _ _ _ h).1
  simp [noPromotion]

end ReceiptPromotion
