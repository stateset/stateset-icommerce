-------------------------- MODULE ShipmentExpiry --------------------------
EXTENDS Naturals
CONSTANTS CheckDeadline, RespectLocks
VARIABLES due, owner, shipPhase, sweepPhase, skipped, cachedQty,
          open, quantity, allocated, onHand, shipped, expired,
          admittedDue, request
vars == <<due, owner, shipPhase, sweepPhase, skipped, cachedQty,
          open, quantity, allocated, onHand, shipped, expired,
          admittedDue, request>>

(* The target owns two units; a same-SKU sibling owns three. Admission
   linearizes after acquiring the row lock, before the shipment commits. *)
Init == /\ due = FALSE /\ owner = "none"
        /\ shipPhase = "queued" /\ sweepPhase = "idle" /\ skipped = FALSE
        /\ cachedQty = 0 /\ open = TRUE /\ quantity = 2
        /\ allocated = 5 /\ onHand = 10 /\ shipped = 0 /\ expired = 0
        /\ admittedDue = FALSE /\ request \in 1..2

DeadlinePasses == /\ ~due /\ due' = TRUE
                  /\ UNCHANGED <<owner, shipPhase, sweepPhase, skipped, cachedQty,
                       open, quantity, allocated, onHand, shipped, expired, admittedDue, request>>

AdmitShip == /\ shipPhase = "queued" /\ owner = "none" /\ open
             /\ (~CheckDeadline \/ ~due)
             /\ shipPhase' = "admitted" /\ owner' = "ship" /\ admittedDue' = due
             /\ UNCHANGED <<due, sweepPhase, skipped, cachedQty, open, quantity,
                  allocated, onHand, shipped, expired, request>>

RejectShip == /\ shipPhase = "queued" /\ owner = "none"
              /\ (~open \/ (CheckDeadline /\ due))
              /\ shipPhase' = "done"
              /\ UNCHANGED <<due, owner, sweepPhase, skipped, cachedQty, open, quantity,
                   allocated, onHand, shipped, expired, admittedDue, request>>

CommitShip == /\ shipPhase = "admitted"
              /\ shipPhase' = "done" /\ owner' = "none"
              /\ onHand' = onHand - request /\ allocated' = allocated - request
              /\ shipped' = request /\ open' = (request < 2)
              /\ quantity' = IF request < 2 THEN 2 - request ELSE 2
              /\ UNCHANGED <<due, sweepPhase, skipped, cachedQty, expired, admittedDue, request>>

SkipLocked == /\ sweepPhase = "idle" /\ due /\ open /\ owner = "ship"
              /\ ~skipped /\ skipped' = TRUE
              /\ UNCHANGED <<due, owner, shipPhase, sweepPhase, cachedQty, open, quantity,
                   allocated, onHand, shipped, expired, admittedDue, request>>

ClaimExpiry == /\ sweepPhase = "idle" /\ due /\ open
               /\ (~RespectLocks \/ owner = "none")
               /\ sweepPhase' = "claimed" /\ owner' = "expiry" /\ cachedQty' = quantity
               /\ UNCHANGED <<due, shipPhase, skipped, open, quantity, allocated,
                    onHand, shipped, expired, admittedDue, request>>

CommitExpiry == /\ sweepPhase = "claimed"
                /\ sweepPhase' = "done" /\ owner' = "none"
                /\ open' = FALSE /\ allocated' = allocated - cachedQty
                /\ expired' = cachedQty
                /\ UNCHANGED <<due, shipPhase, skipped, cachedQty, quantity,
                     onHand, shipped, admittedDue, request>>

Next == DeadlinePasses \/ AdmitShip \/ RejectShip \/ CommitShip \/ SkipLocked \/ ClaimExpiry \/ CommitExpiry
Spec == Init /\ [][Next]_vars
LiveAtAdmission == ~admittedDue
OwnerConservation == /\ allocated = 3 + IF open THEN quantity ELSE 0
                     /\ shipped + expired + (IF open THEN quantity ELSE 0) = 2
StockConservation == /\ onHand + shipped = 10 /\ allocated <= onHand
TypeOK == /\ due \in BOOLEAN /\ admittedDue \in BOOLEAN /\ skipped \in BOOLEAN
          /\ owner \in {"none", "ship", "expiry"}
          /\ shipPhase \in {"queued", "admitted", "done"}
          /\ sweepPhase \in {"idle", "claimed", "done"}
          /\ open \in BOOLEAN /\ quantity \in 0..2 /\ cachedQty \in 0..2
          /\ allocated \in 0..5 /\ onHand \in 8..10
          /\ shipped \in 0..2 /\ expired \in 0..2 /\ request \in 1..2
=============================================================================
