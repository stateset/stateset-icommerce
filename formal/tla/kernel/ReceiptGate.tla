---------------------------- MODULE ReceiptGate ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicGuard, EnforcePolicy, WorkerBAllowed
ASSUME AtomicGuard \in BOOLEAN /\ EnforcePolicy \in BOOLEAN /\ WorkerBAllowed \in BOOLEAN
Workers == {"a", "b"}
Fingerprint == [a |-> "same", b |-> "different"]
Authorized == [a |-> TRUE, b |-> WorkerBAllowed]
VARIABLES stored, effects, checked, unauthorized
vars == <<stored, effects, checked, unauthorized>>

Init == /\ stored = "none" /\ effects = 0 /\ unauthorized = 0
        /\ checked = [w \in Workers |-> FALSE]

\* Policy is a fixed operator-owned decision in this model. The receipt,
\* request fingerprint and business mutation commit in one transaction.
Execute(w) == /\ AtomicGuard /\ w \in Workers /\ stored = "none"
              /\ (~EnforcePolicy \/ Authorized[w])
              /\ stored' = Fingerprint[w]
              /\ effects' = effects + 1
              /\ unauthorized' = unauthorized + IF Authorized[w] THEN 0 ELSE 1
              /\ UNCHANGED checked
CheckSplit(w) == /\ ~AtomicGuard /\ w \in Workers /\ stored = "none"
                 /\ (~EnforcePolicy \/ Authorized[w]) /\ ~checked[w]
                 /\ checked' = [checked EXCEPT ![w] = TRUE]
                 /\ UNCHANGED <<stored, effects, unauthorized>>
CommitSplit(w) == /\ ~AtomicGuard /\ w \in Workers /\ checked[w]
                  /\ stored' = Fingerprint[w]
                  /\ effects' = effects + 1
                  /\ unauthorized' = unauthorized + IF Authorized[w] THEN 0 ELSE 1
                  /\ checked' = [checked EXCEPT ![w] = FALSE]
Replay(w) == /\ w \in Workers /\ stored = Fingerprint[w]
             /\ UNCHANGED vars
Conflict(w) == /\ w \in Workers /\ stored # "none"
               /\ stored # Fingerprint[w] /\ UNCHANGED vars

Next == \E w \in Workers:
  Execute(w) \/ CheckSplit(w) \/ CommitSplit(w) \/ Replay(w) \/ Conflict(w)
Spec == Init /\ [][Next]_vars
AtMostOneEffect == effects <= 1
NoUnauthorizedEffect == unauthorized = 0
TypeOK == /\ stored \in {"none", "same", "different"}
          /\ effects \in 0..2 /\ unauthorized \in 0..2
          /\ checked \in [Workers -> BOOLEAN]
=============================================================================
