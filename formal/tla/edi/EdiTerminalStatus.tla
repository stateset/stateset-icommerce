-------------------------- MODULE EdiTerminalStatus -------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES status, terminalWrites, phase
vars == <<status, terminalWrites, phase>>
Init == /\ status = "pending" /\ terminalWrites = 0
        /\ phase = [w \in Workers |-> "idle"]
Finish(w) == /\ Atomic /\ status = "pending" /\ phase[w] = "idle"
             /\ status' = IF w = "a" THEN "processed" ELSE "acknowledged"
             /\ terminalWrites' = terminalWrites + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ status = "pending" /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<status, terminalWrites>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ status' = IF w = "a" THEN "processed" ELSE "acknowledged"
             /\ terminalWrites' = terminalWrites + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Finish(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
TerminalOnce == terminalWrites <= 1
TypeOK == /\ status \in {"pending", "processed", "acknowledged"}
          /\ terminalWrites \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
