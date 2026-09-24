------------------------- MODULE RatePublication -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES current, history, phase
vars == <<current, history, phase>>
Init == /\ current = 0 /\ history = 0 /\ phase = "idle"
Publish == /\ Atomic /\ phase = "idle"
           /\ current' = 1 /\ history' = 1 /\ phase' = "done"
WriteCurrent == /\ ~Atomic /\ phase = "idle"
                /\ current' = 1 /\ phase' = "history"
                /\ UNCHANGED history
WriteHistory == /\ ~Atomic /\ phase = "history"
                /\ history' = 1 /\ phase' = "done"
                /\ UNCHANGED current
Next == Publish \/ WriteCurrent \/ WriteHistory
Spec == Init /\ [][Next]_vars
PublishedHasHistory == current <= history
TypeOK == /\ current \in 0..1 /\ history \in 0..1
          /\ phase \in {"idle", "history", "done"}
=============================================================================
