---------------------------- MODULE PeriodClose ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicPost
ASSUME AtomicPost \in BOOLEAN

VARIABLES status, posted, closing, snapshot, pending
vars == <<status, posted, closing, snapshot, pending>>

Init == /\ status = "open"
        /\ posted = 0
        /\ closing = 0
        /\ snapshot = 0
        /\ pending = FALSE

\* Two ordinary entries can race with close. A posting is visible only after
\* its period-status check and balance update commit together.
Post == /\ status = "open"
        /\ posted < 2
        /\ posted' = posted + 1
        /\ UNCHANGED <<status, closing, snapshot, pending>>

BeginSplitPost == /\ ~AtomicPost /\ status = "open" /\ ~pending
                  /\ pending' = TRUE
                  /\ UNCHANGED <<status, posted, closing, snapshot>>
CommitSplitPost == /\ ~AtomicPost /\ pending /\ posted < 2
                   /\ posted' = posted + 1
                   /\ pending' = FALSE
                   /\ UNCHANGED <<status, closing, snapshot>>

Close == /\ status = "open" /\ closing = 0
         /\ status' = "closed"
         /\ closing' = 1
         /\ snapshot' = posted
         /\ UNCHANGED <<posted, pending>>
Lock == /\ status = "closed"
        /\ status' = "locked"
        /\ UNCHANGED <<posted, closing, snapshot, pending>>

Next == Post \/ BeginSplitPost \/ CommitSplitPost \/ Close \/ Lock
Spec == Init /\ [][Next]_vars

ClosedBalanceFrozen == status \in {"closed", "locked"} => posted = snapshot
ClosingEntryUnique == closing <= 1
TypeOK == /\ status \in {"open", "closed", "locked"}
          /\ posted \in 0..2 /\ closing \in 0..1 /\ snapshot \in 0..2
          /\ pending \in BOOLEAN
=============================================================================
