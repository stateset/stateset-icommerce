---------------------------- MODULE ChargeRetry ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicCharge
ASSUME AtomicCharge \in BOOLEAN
Workers == {"a", "b"}
VARIABLES status, active, attempts, checked
vars == <<status, active, attempts, checked>>

Init == /\ status = "scheduled"
        /\ active = 0
        /\ attempts = 0
        /\ checked = [w \in Workers |-> FALSE]

Charge == /\ AtomicCharge
          /\ status \in {"scheduled", "failed"} /\ attempts < 2
          /\ status' = "processing"
          /\ active' = 1
          /\ attempts' = attempts + 1
          /\ UNCHANGED checked
Fail == /\ status = "processing"
        /\ status' = "failed" /\ active' = 0
        /\ UNCHANGED <<attempts, checked>>
Pay == /\ status = "processing"
       /\ status' = "paid" /\ active' = 0
       /\ UNCHANGED <<attempts, checked>>
\* Replaying the same receipt does not create another payment attempt.
Replay == /\ attempts > 0
          /\ UNCHANGED vars

CheckSplit(w) == /\ ~AtomicCharge /\ w \in Workers
                 /\ status \in {"scheduled", "failed"}
                 /\ ~checked[w] /\ attempts < 2
                 /\ checked' = [checked EXCEPT ![w] = TRUE]
                 /\ UNCHANGED <<status, active, attempts>>
CommitSplit(w) == /\ ~AtomicCharge /\ w \in Workers /\ checked[w]
                  /\ status' = "processing"
                  /\ active' = active + 1
                  /\ attempts' = attempts + 1
                  /\ checked' = [checked EXCEPT ![w] = FALSE]

Next == Charge \/ Fail \/ Pay \/ Replay \/
        (\E w \in Workers: CheckSplit(w) \/ CommitSplit(w))
Spec == Init /\ [][Next]_vars
AtMostOneLivePayment == active <= 1
ProcessingMatchesPayment == status = "processing" => active = 1
TypeOK == /\ status \in {"scheduled", "processing", "failed", "paid"}
          /\ active \in 0..2 /\ attempts \in 0..2
          /\ checked \in [Workers -> BOOLEAN]
=============================================================================
