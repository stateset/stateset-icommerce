-------------------------- MODULE CreditExposure ---------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Limit == 3
VARIABLES balance, holds, phase
vars == <<balance, holds, phase>>
Init == /\ balance = 0 /\ holds = 0
        /\ phase = [w \in Workers |-> "idle"]
Reserve(w) == /\ Atomic /\ phase[w] = "idle" /\ balance + holds + 2 <= Limit
              /\ holds' = holds + 2
              /\ phase' = [phase EXCEPT ![w] = "done"]
              /\ UNCHANGED balance
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ balance + holds + 2 <= Limit
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<balance, holds>>
Insert(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ holds' = holds + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED balance
Next == \E w \in Workers: Reserve(w) \/ Read(w) \/ Insert(w)
Spec == Init /\ [][Next]_vars
WithinLimit == balance + holds <= Limit
TypeOK == /\ balance \in Nat /\ holds \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
