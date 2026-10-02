---------------------------- MODULE BillingClaim ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicCreate
ASSUME AtomicCreate \in BOOLEAN
Workers == {"a", "b"}
VARIABLES lease, cycles, checked
vars == <<lease, cycles, checked>>

Init == /\ lease = "none"
        /\ cycles = 0
        /\ checked = [w \in Workers |-> FALSE]

Claim(w) == /\ w \in Workers /\ lease = "none"
            /\ lease' = w
            /\ UNCHANGED <<cycles, checked>>
Expire == /\ lease # "none"
          /\ lease' = "none"
          /\ UNCHANGED <<cycles, checked>>

\* The unique (subscription, cycle_number) key is checked at the same
\* serialization point as insert. A lease only gives workers an early guard.
Create(w) == /\ w \in Workers /\ AtomicCreate /\ cycles = 0
             /\ (lease = "none" \/ lease = w)
             /\ cycles' = 1
             /\ UNCHANGED <<lease, checked>>
CheckSplit(w) == /\ w \in Workers /\ ~AtomicCreate
                 /\ cycles = 0 /\ (lease = "none" \/ lease = w)
                 /\ ~checked[w]
                 /\ checked' = [checked EXCEPT ![w] = TRUE]
                 /\ UNCHANGED <<lease, cycles>>
InsertSplit(w) == /\ w \in Workers /\ ~AtomicCreate /\ checked[w]
                  /\ cycles' = cycles + 1
                  /\ checked' = [checked EXCEPT ![w] = FALSE]
                  /\ UNCHANGED lease

Next == (\E w \in Workers: Claim(w) \/ Create(w) \/ CheckSplit(w) \/ InsertSplit(w)) \/ Expire
Spec == Init /\ [][Next]_vars
OneCyclePerKey == cycles <= 1
TypeOK == /\ lease \in Workers \cup {"none"} /\ cycles \in 0..2
          /\ checked \in [Workers -> BOOLEAN]
=============================================================================
