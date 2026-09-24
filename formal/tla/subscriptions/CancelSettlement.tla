------------------------- MODULE CancelSettlement -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES active, nextBill, settled, phase
vars == <<active, nextBill, settled, phase>>
Init == /\ active = TRUE /\ nextBill = TRUE /\ settled = FALSE /\ phase = "idle"
Read == /\ ~Atomic /\ phase = "idle" /\ active
        /\ phase' = "ready" /\ UNCHANGED <<active, nextBill, settled>>
Cancel == /\ active /\ active' = FALSE /\ nextBill' = FALSE
          /\ UNCHANGED <<settled, phase>>
Settle == /\ active /\ phase = "idle"
          /\ settled' = TRUE /\ nextBill' = TRUE
          /\ UNCHANGED active /\ phase' = "done"
Commit == /\ ~Atomic /\ phase = "ready"
          /\ settled' = TRUE /\ nextBill' = TRUE
          /\ UNCHANGED active /\ phase' = "done"
Next == Cancel \/ (IF Atomic THEN Settle ELSE Read \/ Commit)
Spec == Init /\ [][Next]_vars
NoResurrection == ~active => ~nextBill
TypeOK == /\ active \in BOOLEAN /\ nextBill \in BOOLEAN
          /\ settled \in BOOLEAN /\ phase \in {"idle", "ready", "done"}
=============================================================================
