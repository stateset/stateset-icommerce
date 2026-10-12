------------------------ MODULE InventoryReservations ------------------------
(***************************************************************************)
(* One item/location's reservation ledger, matching the transactional     *)
(* reserve, fulfil, confirm, release and expiry paths in both inventory    *)
(* repositories. Quantity is in exact integral units for model checking.  *)
(*                                                                         *)
(* All guards TRUE: a fulfilled row cannot be reopened or consumed again. *)
(* Each guard has its own negative control; TLC must find ledger drift.  *)
(***************************************************************************)
EXTENDS Integers, FiniteSets, TLC

CONSTANTS Capacity, Units1, Units2, GuardConfirm, GuardRelease, GuardFulfil
ASSUME /\ Capacity \in Nat /\ Units1 \in Nat /\ Units2 \in Nat
       /\ GuardConfirm \in BOOLEAN /\ GuardRelease \in BOOLEAN
       /\ GuardFulfil \in BOOLEAN
Ids == {"r1", "r2"}
InitialUnits(i) == IF i = "r1" THEN Units1 ELSE Units2
Open(s) == s \in {"pending", "confirmed", "allocated"}
Statuses == {"absent", "pending", "confirmed", "allocated",
             "fulfilled", "released", "expired", "cancelled"}

\* shipped is a history variable: only Fulfil advances it.
VARIABLES onHand, allocated, status, qty, shipped
vars == <<onHand, allocated, status, qty, shipped>>

RECURSIVE OpenUnits(_)
OpenUnits(ids) ==
    IF ids = {} THEN 0
    ELSE LET i == CHOOSE x \in ids : TRUE IN
         (IF Open(status[i]) THEN qty[i] ELSE 0) + OpenUnits(ids \ {i})

Init ==
    /\ onHand = Capacity
    /\ allocated = 0
    /\ shipped = 0
    /\ status = [i \in Ids |-> "absent"]
    /\ qty = [i \in Ids |-> 0]

Reserve(i) ==
    /\ status[i] = "absent"
    /\ 0 < InitialUnits(i)
    /\ allocated + InitialUnits(i) <= onHand
    /\ allocated' = allocated + InitialUnits(i)
    /\ status' = [status EXCEPT ![i] = "pending"]
    /\ qty' = [qty EXCEPT ![i] = InitialUnits(i)]
    /\ UNCHANGED <<onHand, shipped>>

(* Partial fulfilment reduces the open quantity. Full fulfilment retains   *)
(* the row's recorded quantity, as Rust does, but it no longer holds stock. *)
Fulfil(i, n) ==
    /\ Open(status[i]) \/ (~GuardFulfil /\ status[i] = "fulfilled")
    /\ n \in 1..qty[i]
    /\ onHand >= n
    \* Stay on the ordinary subtraction path, excluding the runtime clamp.
    /\ allocated >= n
    /\ allocated' = allocated - n
    /\ onHand' = onHand - n
    /\ shipped' = shipped + n
    /\ status' = IF n = qty[i]
                 THEN [status EXCEPT ![i] = "fulfilled"] ELSE status
    /\ qty' = IF n = qty[i] THEN qty ELSE [qty EXCEPT ![i] = @ - n]

Confirm(i) ==
    /\ Open(status[i]) \/ (~GuardConfirm /\ status[i] = "fulfilled")
    /\ status' = [status EXCEPT ![i] = "confirmed"]
    /\ UNCHANGED <<onHand, allocated, qty, shipped>>

Release(i) ==
    /\ Open(status[i]) \/ (~GuardRelease /\ status[i] = "fulfilled")
    \* Restrict to the ordinary arithmetic path. The repositories repair
    \* negative allocation deltas from the ledger; that repair is omitted.
    \* The broken case must steal another hold without entering repair.
    /\ allocated >= qty[i]
    /\ status' = [status EXCEPT ![i] = "released"]
    /\ allocated' = allocated - qty[i]
    /\ UNCHANGED <<onHand, qty, shipped>>

Expire(i) ==
    /\ Open(status[i])
    /\ status' = [status EXCEPT ![i] = "expired"]
    /\ allocated' = allocated - qty[i]
    /\ UNCHANGED <<onHand, qty, shipped>>

Next ==
    \/ \E i \in Ids : Reserve(i) \/ Confirm(i) \/ Release(i) \/ Expire(i)
    \/ \E i \in Ids, n \in 1..Capacity : Fulfil(i, n)

Spec == Init /\ [][Next]_vars

AllocatedMatchesOpen == allocated = OpenUnits(Ids)
StockBounds == 0 <= allocated /\ allocated <= onHand /\ onHand <= Capacity
StockConserved == onHand + shipped = Capacity
TypeOK ==
    /\ status \in [Ids -> Statuses]
    /\ qty \in [Ids -> 0..Capacity]
    /\ onHand \in 0..Capacity
    /\ shipped \in 0..Capacity
    /\ allocated \in (-Capacity)..Capacity

=============================================================================
