----------------------- MODULE CycleCountCompletion ------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES status, stock, applications, phase
vars == <<status, stock, applications, phase>>
Init == /\ status = "in_progress" /\ stock = 3 /\ applications = 0
        /\ phase = [w \in Workers |-> "idle"]
Complete(w) == /\ Atomic /\ status = "in_progress" /\ phase[w] = "idle"
               /\ status' = "completed" /\ stock' = stock + 1
               /\ applications' = applications + 1
               /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ status = "in_progress" /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<status, stock, applications>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ status' = "completed" /\ stock' = stock + 1
             /\ applications' = applications + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Complete(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
AtMostOneAdjustment == applications <= 1
StockMatchesAdjustments == stock = 3 + applications
TypeOK == /\ status \in {"in_progress", "completed"}
          /\ stock \in Nat /\ applications \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
