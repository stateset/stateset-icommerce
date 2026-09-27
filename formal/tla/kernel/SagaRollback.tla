---------------------------- MODULE SagaRollback ----------------------------
EXTENDS Naturals, TLC

CONSTANTS ReverseOrder, SkipDone
ASSUME ReverseOrder \in BOOLEAN /\ SkipDone \in BOOLEAN
VARIABLES count1, count2
vars == <<count1, count2>>

\* Both forward steps have completed. Rollback must compensate step 2 before
\* step 1, and a recorded completion must be skipped on retry.
Init == /\ count1 = 0 /\ count2 = 0
Compensate2 == /\ count2 < 2
               /\ (~SkipDone \/ count2 = 0)
               /\ count2' = count2 + 1
               /\ UNCHANGED count1
Compensate1 == /\ count1 < 2
               /\ (~SkipDone \/ count1 = 0)
               /\ (~ReverseOrder \/ count2 = 1)
               /\ count1' = count1 + 1
               /\ UNCHANGED count2
Next == Compensate2 \/ Compensate1
Spec == Init /\ [][Next]_vars
AtMostOnceRecorded == count1 <= 1 /\ count2 <= 1
ReverseCompensation == count1 > 0 => count2 = 1
TypeOK == count1 \in 0..2 /\ count2 \in 0..2
=============================================================================
