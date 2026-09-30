------------------------- MODULE CostLayerIssue -------------------------
EXTENDS Integers, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES remaining, issued, phase
vars == <<remaining, issued, phase>>
Init == /\ remaining = 3 /\ issued = 0
        /\ phase = [w \in Workers |-> "idle"]
Issue(w) == /\ Atomic /\ remaining >= 2 /\ phase[w] = "idle"
            /\ remaining' = remaining - 2 /\ issued' = issued + 2
            /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ remaining >= 2 /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<remaining, issued>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ remaining' = remaining - 2 /\ issued' = issued + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Issue(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
NoOverIssue == remaining >= 0
Conserved == remaining + issued = 3
TypeOK == /\ remaining \in Int /\ issued \in Int
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
