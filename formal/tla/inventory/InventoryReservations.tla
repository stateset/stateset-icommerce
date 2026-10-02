------------------------ MODULE InventoryReservations ------------------------
(***************************************************************************)
(* One item/location's reservation ledger, matching the transactional     *)
(* reserve, fulfil, confirm, release and expiry paths in both inventory    *)
(* repositories. Quantity is in exact integral units for model checking.  *)
(*                                                                         *)
(* GuardTerminal = TRUE: a fulfilled row cannot be confirmed or released. *)
(* GuardTerminal = FALSE: the old guards in PR #223. TLC must find drift.  *)
(***************************************************************************)
EXTENDS Integers, FiniteSets, TLC

CONSTANTS Capacity, Units1, Units2, GuardTerminal
Ids == {"r1", "r2"}
InitialUnits(i) == IF i = "r1" THEN Units1 ELSE Units2
Open(s) == s \in {"pending", "confirmed", "allocated"}
Statuses == {"absent", "pending", "confirmed", "allocated",
             "fulfilled", "released", "expired"}

VARIABLES onHand, allocated, status, qty
vars == <<onHand, allocated, status, qty>>

RECURSIVE OpenUnits(_)
OpenUnits(ids) ==
    IF ids = {} THEN 0
    ELSE LET i == CHOOSE x \in ids : TRUE IN
         (IF Open(status[i]) THEN qty[i] ELSE 0) + OpenUnits(ids \ {i})

Init ==
    /\ onHand = Capacity
    /\ allocated = 0
    /\ status = [i \in Ids |-> "absent"]
    /\ qty = [i \in Ids |-> 0]

Reserve(i) ==
    /\ status[i] = "absent"
    /\ 0 < InitialUnits(i)
    /\ allocated + InitialUnits(i) <= onHand
    /\ allocated' = allocated + InitialUnits(i)
    /\ status' = [status EXCEPT ![i] = "pending"]
    /\ qty' = [qty EXCEPT ![i] = InitialUnits(i)]
    /\ UNCHANGED onHand

(* Partial fulfilment reduces the open quantity. Full fulfilment retains   *)
(* the row's recorded quantity, as Rust does, but it no longer holds stock. *)
Fulfil(i, n) ==
    /\ Open(status[i])
    /\ n \in 1..qty[i]
    /\ onHand >= n
    /\ allocated' = allocated - n
    /\ onHand' = onHand - n
    /\ status' = IF n = qty[i]
                 THEN [status EXCEPT ![i] = "fulfilled"] ELSE status
    /\ qty' = IF n = qty[i] THEN qty ELSE [qty EXCEPT ![i] = @ - n]

Confirm(i) ==
    /\ Open(status[i]) \/ (~GuardTerminal /\ status[i] = "fulfilled")
    /\ status' = [status EXCEPT ![i] = "confirmed"]
    /\ UNCHANGED <<onHand, allocated, qty>>

Release(i) ==
    /\ Open(status[i]) \/ (~GuardTerminal /\ status[i] = "fulfilled")
    /\ status' = [status EXCEPT ![i] = "released"]
    /\ allocated' = allocated - qty[i]
    /\ UNCHANGED <<onHand, qty>>

Expire(i) ==
    /\ Open(status[i])
    /\ status' = [status EXCEPT ![i] = "expired"]
    /\ allocated' = allocated - qty[i]
    /\ UNCHANGED <<onHand, qty>>

Next ==
    \/ \E i \in Ids : Reserve(i) \/ Confirm(i) \/ Release(i) \/ Expire(i)
    \/ \E i \in Ids, n \in 1..Capacity : Fulfil(i, n)

Spec == Init /\ [][Next]_vars

AllocatedMatchesOpen == allocated = OpenUnits(Ids)
StockBounds == 0 <= allocated /\ allocated <= onHand /\ onHand <= Capacity
TypeOK ==
    /\ status \in [Ids -> Statuses]
    /\ qty \in [Ids -> 0..Capacity]
    /\ onHand \in 0..Capacity
    /\ allocated \in (-Capacity)..Capacity

=============================================================================
