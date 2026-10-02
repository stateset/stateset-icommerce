------------------------- MODULE InboundCancelReceipt -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES status, received, cancelledEver, phase
vars == <<status, received, cancelledEver, phase>>
Init == /\ status = "open" /\ received = 0 /\ cancelledEver = FALSE
        /\ phase = "idle"
Read == /\ ~Atomic /\ status = "open" /\ phase = "idle"
        /\ phase' = "ready" /\ UNCHANGED <<status, received, cancelledEver>>
Receive == /\ Atomic /\ status = "open" /\ phase = "idle"
           /\ status' = "received" /\ received' = 1 /\ phase' = "done"
           /\ UNCHANGED cancelledEver
Commit == /\ ~Atomic /\ phase = "ready"
          /\ status' = "received" /\ received' = 1 /\ phase' = "done"
          /\ UNCHANGED cancelledEver
Cancel == /\ status = "open" /\ status' = "cancelled"
          /\ cancelledEver' = TRUE /\ UNCHANGED <<received, phase>>
Next == Cancel \/ (IF Atomic THEN Receive ELSE Read \/ Commit)
Spec == Init /\ [][Next]_vars
CancelledHasNoReceipt == cancelledEver => /\ status = "cancelled" /\ received = 0
OneTerminalDecision == status = "received" => received = 1
TypeOK == /\ status \in {"open", "received", "cancelled"}
          /\ received \in 0..1 /\ cancelledEver \in BOOLEAN
          /\ phase \in {"idle", "ready", "done"}
=============================================================================
