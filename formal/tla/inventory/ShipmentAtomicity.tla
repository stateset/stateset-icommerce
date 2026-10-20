------------------------- MODULE ShipmentAtomicity -------------------------
EXTENDS Naturals
CONSTANTS AtomicPublish, RollbackAll
Workers == {"a", "b"}
Groups == 1..5
(* Groups: stock/reservations, shipment ledger, outbox, order, receipt.
   A group abstracts the batch of all lines in a dispatch. *)
VARIABLES durable, working, owner, progress, receipt
vars == <<durable, working, owner, progress, receipt>>

Init == /\ durable = [i \in Groups |-> 0] /\ working = durable
        /\ owner = "none" /\ progress = 0 /\ receipt = "previewed"

Begin(w) == /\ owner = "none" /\ durable[5] = 0
            /\ owner' = w /\ progress' = 0 /\ working' = durable
            /\ UNCHANGED <<durable, receipt>>

Write == /\ owner \in Workers /\ progress < 5
         /\ progress' = progress + 1
         /\ working' = [working EXCEPT ![progress + 1] = @ + 1]
         /\ durable' = IF AtomicPublish THEN durable ELSE working'
         /\ UNCHANGED <<owner, receipt>>

Commit == /\ owner \in Workers /\ progress = 5
          /\ durable' = working /\ owner' = "none"
          /\ receipt' = "succeeded"
          /\ UNCHANGED <<working, progress>>

(* An exception, cancellation or abstract pre-commit crash discards private
   writes. The persisted preview must survive a failed promotion. *)
Abort == /\ owner \in Workers
         /\ durable' = IF RollbackAll THEN durable ELSE working
         /\ working' = durable' /\ owner' = "none" /\ progress' = 0
         /\ UNCHANGED receipt

Next == (\E w \in Workers: Begin(w)) \/ Write \/ Commit \/ Abort
Spec == Init /\ [][Next]_vars
Coherent == \A i \in Groups: durable[i] = durable[5]
ReceiptBacked == (receipt = "succeeded") = (durable[5] = 1)
AtMostOnce == \A i \in Groups: durable[i] <= 1
TypeOK == /\ durable \in [Groups -> Nat] /\ working \in [Groups -> Nat]
          /\ owner \in Workers \cup {"none"} /\ progress \in 0..5
          /\ receipt \in {"previewed", "succeeded"}
===========================================================================
