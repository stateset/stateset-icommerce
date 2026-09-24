------------------------- MODULE ReturnDisposition -------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES disposition, onHand, phase
vars == <<disposition, onHand, phase>>
Init == /\ disposition = "none" /\ onHand = 0 /\ phase = "idle"
Restock == /\ Atomic /\ disposition = "none" /\ phase = "idle"
           /\ disposition' = "restock" /\ onHand' = 2 /\ phase' = "done"
MarkDisposition == /\ ~Atomic /\ disposition = "none" /\ phase = "idle"
                   /\ disposition' = "restock" /\ phase' = "marked"
                   /\ UNCHANGED onHand
AddStock == /\ ~Atomic /\ phase = "marked"
            /\ onHand' = 2 /\ phase' = "done"
            /\ UNCHANGED disposition
Next == Restock \/ MarkDisposition \/ AddStock
Spec == Init /\ [][Next]_vars
StockMatchesDisposition == onHand = IF disposition = "restock" THEN 2 ELSE 0
TypeOK == /\ disposition \in {"none", "restock"}
          /\ onHand \in Nat /\ phase \in {"idle", "marked", "done"}
=============================================================================
