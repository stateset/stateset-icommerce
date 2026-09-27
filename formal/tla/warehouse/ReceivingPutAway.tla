-------------------------- MODULE ReceivingPutAway -------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Received == 2
VARIABLES task, stock, movements, phase
vars == <<task, stock, movements, phase>>
Init == /\ task = "pending" /\ stock = 0 /\ movements = 0
        /\ phase = [w \in Workers |-> "idle"]
Complete(w) == /\ Atomic /\ phase[w] = "idle" /\ task = "pending"
               /\ task' = "completed" /\ stock' = stock + 2
               /\ movements' = movements + 1
               /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ task = "pending"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<task, stock, movements>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ task' = "completed" /\ stock' = stock + 2
             /\ movements' = movements + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Cancel == /\ task = "pending" /\ task' = "cancelled"
          /\ UNCHANGED <<stock, movements, phase>>
Next == Cancel \/ (\E w \in Workers: Complete(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
NoDoublePutAway == stock <= Received
MovementMatchesStock == stock = 2 * movements
TypeOK == /\ task \in {"pending", "completed", "cancelled"}
          /\ stock \in Nat /\ movements \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
