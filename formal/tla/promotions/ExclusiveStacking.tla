-------------------------- MODULE ExclusiveStacking --------------------------
EXTENDS Naturals, TLC

CONSTANTS GuardStackable
ASSUME GuardStackable \in BOOLEAN
VARIABLES applied, exclusive
vars == <<applied, exclusive>>

Init == /\ applied = 0 /\ exclusive = FALSE
ApplyExclusive == /\ applied = 0
                  /\ applied' = 1 /\ exclusive' = TRUE
ApplyStackable == /\ applied < 2
                  /\ (~GuardStackable \/ ~exclusive)
                  /\ applied' = applied + 1
                  /\ UNCHANGED exclusive
Next == ApplyExclusive \/ ApplyStackable
Spec == Init /\ [][Next]_vars
ExclusiveStandsAlone == exclusive => applied = 1
TypeOK == applied \in 0..2 /\ exclusive \in BOOLEAN
=============================================================================
