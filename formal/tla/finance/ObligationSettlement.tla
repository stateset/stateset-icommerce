------------------------- MODULE ObligationSettlement -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES paid, status, cancelledEver, phase
vars == <<paid, status, cancelledEver, phase>>
Init == /\ paid = 0 /\ status = "open" /\ cancelledEver = FALSE
        /\ phase = [w \in Workers |-> "idle"]
Pay(w) == /\ Atomic /\ status = "open" /\ paid + 2 <= 3
          /\ phase[w] = "idle" /\ paid' = paid + 2
          /\ phase' = [phase EXCEPT ![w] = "done"]
          /\ UNCHANGED <<status, cancelledEver>>
Read(w) == /\ ~Atomic /\ status = "open" /\ paid + 2 <= 3
           /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<paid, status, cancelledEver>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ paid' = paid + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED <<status, cancelledEver>>
Cancel == /\ status = "open" /\ status' = "cancelled"
          /\ cancelledEver' = TRUE /\ UNCHANGED <<paid, phase>>
Reopen == /\ ~Atomic /\ status = "cancelled"
          /\ status' = "open" /\ UNCHANGED <<paid, cancelledEver, phase>>
Next == Cancel \/ Reopen \/ (\E w \in Workers: Pay(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
WithinObligation == paid <= 3
TerminalNeverReopens == cancelledEver => status = "cancelled"
TypeOK == /\ paid \in Nat /\ status \in {"open", "cancelled"}
          /\ cancelledEver \in BOOLEAN
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
