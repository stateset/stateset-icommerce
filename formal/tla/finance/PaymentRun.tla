---------------------------- MODULE PaymentRun ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicProcess
ASSUME AtomicProcess \in BOOLEAN
Workers == {"a", "b"}
VARIABLES status, due, paid, checks
vars == <<status, due, paid, checks>>

Init == /\ status = "approved"
        /\ due = 2
        /\ paid = 0
        /\ checks = [w \in Workers |-> FALSE]

\* The approved->completed claim, bill re-read, payment insert, and bill
\* recalculation share one write transaction in process_payment_run.
Process == /\ AtomicProcess /\ status = "approved"
           /\ status' = "completed" /\ paid' = paid + due /\ due' = 0
           /\ UNCHANGED checks
\* A separate direct payment may settle the bill before the run processes;
\* the run then re-reads due=0 and skips it.
DirectPay == /\ status = "approved" /\ due > 0
             /\ paid' = paid + due /\ due' = 0
             /\ UNCHANGED <<status, checks>>
Cancel == /\ status = "approved"
          /\ status' = "cancelled"
          /\ UNCHANGED <<due, paid, checks>>
CheckSplit(w) == /\ ~AtomicProcess /\ w \in Workers
                 /\ status = "approved" /\ ~checks[w]
                 /\ checks' = [checks EXCEPT ![w] = TRUE]
                 /\ UNCHANGED <<status, due, paid>>
CommitSplit(w) == /\ ~AtomicProcess /\ w \in Workers /\ checks[w]
                  /\ status' = "completed" /\ paid' = paid + 2
                  /\ due' = 0 /\ checks' = [checks EXCEPT ![w] = FALSE]

Next == Process \/ DirectPay \/ Cancel \/
        (\E w \in Workers: CheckSplit(w) \/ CommitSplit(w))
Spec == Init /\ [][Next]_vars
NoDoubleDisbursement == paid <= 2
BillConserved == paid + due = 2
TypeOK == /\ status \in {"approved", "completed", "cancelled"}
          /\ due \in 0..2 /\ paid \in 0..4
          /\ checks \in [Workers -> BOOLEAN]
=============================================================================
