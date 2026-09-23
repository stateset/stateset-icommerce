------------------------- MODULE WarrantyClaimSlots ------------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
MaxClaims == 1
VARIABLES used, claims, phase
vars == <<used, claims, phase>>
Init == /\ used = 0 /\ claims = [w \in Workers |-> FALSE]
        /\ phase = [w \in Workers |-> "idle"]
Claim(w) == /\ Atomic /\ phase[w] = "idle" /\ used < MaxClaims
            /\ used' = used + 1
            /\ claims' = [claims EXCEPT ![w] = TRUE]
            /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ used < MaxClaims
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<used, claims>>
Insert(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ used' = used + 1
             /\ claims' = [claims EXCEPT ![w] = TRUE]
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Claim(w) \/ Read(w) \/ Insert(w)
Spec == Init /\ [][Next]_vars
WithinClaimLimit == used <= MaxClaims
ClaimsMatchSlots == used = Cardinality({w \in Workers: claims[w]})
TypeOK == /\ used \in Nat /\ claims \in [Workers -> BOOLEAN]
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
