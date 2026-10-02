---------------------------- MODULE PaymentRefunds ----------------------------
(***************************************************************************)
(* The refund-reservation protocol of a single captured payment, as        *)
(* implemented in crates/stateset-db/src/{sqlite,postgres}/payments.rs,    *)
(* modelled so TLC can check every interleaving of concurrent callers.     *)
(*                                                                         *)
(* A refund is its own record (pending -> processing -> completed, or      *)
(* failed / cancelled). Creating one RESERVES money: its amount must fit   *)
(* within  amount - amount_refunded - (refunds still in flight).  That is  *)
(* a read-check-then-insert, so it is only safe if concurrent creators are *)
(* serialized. The code serializes them -- SQLite with BEGIN IMMEDIATE,    *)
(* Postgres with SELECT ... FROM payments WHERE id = $1 FOR UPDATE -- and  *)
(* the constant `Locked` models exactly that:                              *)
(*                                                                         *)
(*   Locked = TRUE   the check and the insert are one atomic step. This    *)
(*                   is the implementation; every invariant must hold.     *)
(*   Locked = FALSE  the read and the insert are separate steps another    *)
(*                   caller can interleave -- what a code path that forgot *)
(*                   the lock would do. TLC must find an over-refund here; *)
(*                   if it ever stops finding one, the model has stopped   *)
(*                   saying anything and scripts/check.sh fails.           *)
(***************************************************************************)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    Amount,      \* the captured amount, in minor units
    MaxRefunds,  \* bound on refund records, to keep the state space finite
    Workers,     \* concurrent callers of create_refund
    Locked       \* TRUE: reservation check and insert are one serialized step

Statuses ==
    {"pending", "processing", "requires_action", "completed", "failed",
     "cancelled", "refunded", "partially_refunded", "disputed"}

(* PaymentTransactionStatus::can_transition_to, clause for clause.         *)
(* crates/stateset-core/src/models/payment.rs pins the Rust side against   *)
(* can_transition.golden, and scripts/check.sh pins THIS operator against  *)
(* the same file, so the spec and the code cannot drift apart silently.    *)
CanTransition(from, to) ==
    \/ from = to
    \/ from = "pending"            /\ to \in {"processing", "cancelled", "failed"}
    \/ from = "processing"         /\ to \in {"requires_action", "completed", "failed", "cancelled"}
    \/ from = "requires_action"    /\ to \in {"processing", "completed", "failed", "cancelled"}
    \/ from = "completed"          /\ to \in {"refunded", "partially_refunded", "disputed"}
    \/ from = "partially_refunded" /\ to \in {"refunded", "disputed"}
    \/ from = "disputed"           /\ to \in {"completed", "refunded", "cancelled"}

(* PaymentTransactionStatus::is_refundable *)
Refundable(s) == s \in {"completed", "partially_refunded"}

InFlightStatuses == {"pending", "processing"}
RefundStatuses   == {"pending", "processing", "completed", "failed", "cancelled"}

VARIABLES
    pstatus,   \* payments.status
    refunded,  \* payments.amount_refunded
    refunds,   \* the refunds table for this payment: a sequence of [amt, st]
    seen,      \* Locked = FALSE only: the remaining balance a caller read
    pc         \* Locked = FALSE only: "idle" or "insert"

vars == <<pstatus, refunded, refunds, seen, pc>>

RefundIds == 1..Len(refunds)

RECURSIVE SumAmts(_)
SumAmts(ids) ==
    IF ids = {} THEN 0
    ELSE LET i == CHOOSE x \in ids : TRUE IN refunds[i].amt + SumAmts(ids \ {i})

InFlight  == SumAmts({i \in RefundIds : refunds[i].st \in InFlightStatuses})
Remaining == Amount - refunded - InFlight   \* refundable_remaining_in_tx

-----------------------------------------------------------------------------

Init ==
    /\ pstatus  = "pending"
    /\ refunded = 0
    /\ refunds  = <<>>
    /\ seen     = [w \in Workers |-> 0]
    /\ pc       = [w \in Workers |-> "idle"]

(* The payment's own lifecycle: authorization, capture, dispute. Moves     *)
(* into refunded / partially_refunded happen ONLY through CompleteRefund,  *)
(* as in the code. (A chargeback that lands a dispute on `refunded`        *)
(* without a refund record is out of scope for this model.)                *)
LifecycleTargets ==
    {"processing", "requires_action", "completed", "failed", "cancelled", "disputed"}

PaymentStep ==
    /\ \E to \in LifecycleTargets :
          /\ to # pstatus
          /\ CanTransition(pstatus, to)
          /\ pstatus' = to
    /\ UNCHANGED <<refunded, refunds, seen, pc>>

(* create_refund with the lock held: check and insert are atomic.          *)
CreateLocked(w, a) ==
    /\ Locked
    /\ Len(refunds) < MaxRefunds
    /\ Refundable(pstatus)
    /\ a <= Remaining
    /\ refunds' = Append(refunds, [amt |-> a, st |-> "pending"])
    /\ UNCHANGED <<pstatus, refunded, seen, pc>>

(* create_refund WITHOUT the lock, split where an interleaving can land.   *)
ReadRemaining(w) ==
    /\ ~Locked
    /\ pc[w] = "idle"
    /\ Refundable(pstatus)
    /\ seen' = [seen EXCEPT ![w] = Remaining]
    /\ pc'   = [pc EXCEPT ![w] = "insert"]
    /\ UNCHANGED <<pstatus, refunded, refunds>>

InsertRefund(w, a) ==
    /\ ~Locked
    /\ pc[w] = "insert"
    /\ Len(refunds) < MaxRefunds
    /\ a <= seen[w]                          \* checked against a stale read
    /\ refunds' = Append(refunds, [amt |-> a, st |-> "pending"])
    /\ pc'      = [pc EXCEPT ![w] = "idle"]
    /\ UNCHANGED <<pstatus, refunded, seen>>

StartRefund(i) ==
    /\ refunds[i].st = "pending"
    /\ refunds' = [refunds EXCEPT ![i].st = "processing"]
    /\ UNCHANGED <<pstatus, refunded, seen, pc>>

(* complete_refund: the refund row is locked and completion is idempotent  *)
(* (an already-completed refund is a no-op, so it cannot double-count).    *)
(* The payment write is guarded by                                          *)
(*     UPDATE payments ... WHERE status IN (statuses allowed to reach new) *)
(* and a zero-row update rolls the whole transaction back -- modelled here *)
(* as the action simply not being enabled.                                 *)
NewStatus(r) == IF r >= Amount THEN "refunded" ELSE "partially_refunded"

CompleteRefund(i) ==
    /\ refunds[i].st \in InFlightStatuses
    /\ LET nr == refunded + refunds[i].amt
           ns == NewStatus(nr)
       IN  /\ CanTransition(pstatus, ns)
           /\ refunded' = nr
           /\ pstatus'  = ns
           /\ refunds'  = [refunds EXCEPT ![i].st = "completed"]
    /\ UNCHANGED <<seen, pc>>

FailRefund(i) ==
    /\ refunds[i].st \in InFlightStatuses
    /\ refunds' = [refunds EXCEPT ![i].st = "failed"]
    /\ UNCHANGED <<pstatus, refunded, seen, pc>>

CancelRefund(i) ==
    /\ refunds[i].st = "pending"
    /\ refunds' = [refunds EXCEPT ![i].st = "cancelled"]
    /\ UNCHANGED <<pstatus, refunded, seen, pc>>

Next ==
    \/ PaymentStep
    \/ \E w \in Workers, a \in 1..Amount : CreateLocked(w, a) \/ InsertRefund(w, a)
    \/ \E w \in Workers : ReadRemaining(w)
    \/ \E i \in RefundIds :
          StartRefund(i) \/ CompleteRefund(i) \/ FailRefund(i) \/ CancelRefund(i)

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Properties                                                              *)

TypeOK ==
    /\ pstatus \in Statuses
    /\ refunded \in 0..(Amount * MaxRefunds)   \* loose on purpose: an over-refund must be representable to be caught
    /\ \A i \in RefundIds : refunds[i].st \in RefundStatuses /\ refunds[i].amt \in 1..Amount

(* The money invariant: completed plus reserved never exceeds the capture. *)
NoOverRefund == refunded + InFlight <= Amount

(* amount_refunded is exactly the sum of completed refunds -- no refund is *)
(* counted twice and none is lost.                                         *)
RefundedIsSumOfCompleted ==
    refunded = SumAmts({i \in RefundIds : refunds[i].st = "completed"})

(* The status agrees with the balance.                                     *)
StatusMatchesBalance ==
    /\ pstatus = "refunded"           => refunded = Amount
    /\ pstatus = "partially_refunded" => 0 < refunded /\ refunded < Amount

(* Every status change the model makes is one the state machine allows.   *)
TransitionsLegal == [][pstatus' # pstatus => CanTransition(pstatus, pstatus')]_vars

=============================================================================
