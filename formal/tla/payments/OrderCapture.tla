---------------------------- MODULE OrderCapture ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicCapture
ASSUME AtomicCapture \in BOOLEAN
Workers == {"a", "b"}
VARIABLES captured, phase
vars == <<captured, phase>>

Init == /\ captured = 0
        /\ phase = [w \in Workers |-> "idle"]
\* Each worker requests two units against an order totaling three. The
\* capacity check and payment insert share one write transaction.
Capture(w) == /\ AtomicCapture /\ w \in Workers /\ phase[w] = "idle"
              /\ captured + 2 <= 3
              /\ captured' = captured + 2
              /\ phase' = [phase EXCEPT ![w] = "done"]
CheckSplit(w) == /\ ~AtomicCapture /\ w \in Workers /\ phase[w] = "idle"
                 /\ captured + 2 <= 3
                 /\ phase' = [phase EXCEPT ![w] = "checked"]
                 /\ UNCHANGED captured
CommitSplit(w) == /\ ~AtomicCapture /\ w \in Workers /\ phase[w] = "checked"
                  /\ captured' = captured + 2
                  /\ phase' = [phase EXCEPT ![w] = "done"]

Next == \E w \in Workers: Capture(w) \/ CheckSplit(w) \/ CommitSplit(w)
Spec == Init /\ [][Next]_vars
NoOverCapture == captured <= 3
TypeOK == /\ captured \in 0..4
          /\ phase \in [Workers -> {"idle", "checked", "done"}]
=============================================================================
