/-!
# Captured cash and completed refunds

The payment's amount_refunded is the sum of completed refund records. At each
completion the accepted amount is limited to the remaining captured balance.
The proof is over exact nonnegative minor units, independent of refund count.
-/

namespace CashReconciliation

structure Cash where
  available : Nat
  refunded : Nat
  deriving Repr

def refund (cash : Cash) (request : Nat) : Cash :=
  let accepted := min request cash.available
  { available := cash.available - accepted
    refunded := cash.refunded + accepted }

theorem refund_conserves_capture (cash : Cash) (request : Nat) :
    (refund cash request).available + (refund cash request).refunded =
      cash.available + cash.refunded := by
  unfold refund
  dsimp
  have h := Nat.min_le_right request cash.available
  omega

def refundAll (cash : Cash) (requests : List Nat) : Cash :=
  requests.foldl refund cash

theorem refunds_conserve_capture (cash : Cash) (requests : List Nat) :
    (refundAll cash requests).available + (refundAll cash requests).refunded =
      cash.available + cash.refunded := by
  unfold refundAll
  have aux (xs : List Nat) (current : Cash) :
      (xs.foldl refund current).available + (xs.foldl refund current).refunded =
        current.available + current.refunded := by
    induction xs generalizing current with
    | nil => simp
    | cons x rest ih =>
      simp only [List.foldl_cons]
      rw [ih, refund_conserves_capture]
  exact aux requests cash

end CashReconciliation
