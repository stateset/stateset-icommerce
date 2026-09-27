/-!
Pure equations for HTTP idempotency generations, loyalty points, prepayments,
quality hold release, and EDI terminal transitions. Concurrency is checked by
the matching bounded TLA+ models; these theorems cover arbitrary natural units.
-/

namespace HttpIdempotency

def put (stored : Option Nat) (response : Nat) : Option Nat :=
  match stored with
  | none => some response
  | some first => some first

theorem first_write_wins (first second : Nat) :
    put (put none first) second = some first := by
  rfl

theorem expired_generation_restarts (old new : Nat) :
    put none new = some new ∧ put (some old) new = some old := by
  constructor <;> rfl

end HttpIdempotency

namespace LoyaltyPoints

theorem redeem_conserves (opening balance spent amount : Nat)
    (h : balance + spent = opening) (enough : amount ≤ balance) :
    balance - amount + (spent + amount) = opening := by
  omega

theorem earn_conserves (opening earned spent balance amount : Nat)
    (h : balance + spent = opening + earned) :
    balance + amount + spent = opening + (earned + amount) := by
  omega

end LoyaltyPoints

namespace PrepaymentSettlement

theorem application_conserves (original remaining applied refunded amount : Nat)
    (h : remaining + applied + refunded = original)
    (enough : amount ≤ remaining) :
    remaining - amount + (applied + amount) + refunded = original := by
  omega

theorem refund_conserves (original remaining applied refunded : Nat)
    (h : remaining + applied + refunded = original) :
    0 + applied + (refunded + remaining) = original := by
  omega

end PrepaymentSettlement

namespace QualityHoldRelease

def release (owner : Option Nat) (actor : Nat) : Option Nat :=
  match owner with
  | none => some actor
  | some first => some first

theorem release_once (first second : Nat) :
    release (release none first) second = some first := by
  rfl

end QualityHoldRelease

namespace EdiTerminalStatus

def finish (current requested : Nat) : Nat :=
  if current = 0 then requested else current

theorem terminal_write_once (first second : Nat) (terminal : first ≠ 0) :
    finish (finish 0 first) second = first := by
  simp [finish, terminal]

end EdiTerminalStatus
