-------------------------- MODULE YieldAccounting --------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Planned == 3
VARIABLES good, scrap, phase
vars == <<good, scrap, phase>>
Init == /\ good = 0 /\ scrap = 0
        /\ phase = [w \in Workers |-> "idle"]
Report(w) == /\ Atomic /\ phase[w] = "idle" /\ good + scrap + 2 <= Planned
             /\ good' = good + 1 /\ scrap' = scrap + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ good + scrap + 2 <= Planned
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<good, scrap>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ good' = good + 1 /\ scrap' = scrap + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Report(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
WithinPlan == good + scrap <= Planned
TypeOK == /\ good \in Nat /\ scrap \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
