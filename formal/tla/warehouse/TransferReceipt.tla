--------------------------- MODULE TransferReceipt -------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Ordered == 3
VARIABLES shipped, received, status, phase
vars == <<shipped, received, status, phase>>
Init == /\ shipped = 0 /\ received = 0 /\ status = "draft"
        /\ phase = [w \in Workers |-> "idle"]
Ship == /\ status = "draft" /\ shipped' = Ordered /\ status' = "in_transit"
        /\ UNCHANGED <<received, phase>>
Receive(w) == /\ Atomic /\ status \in {"in_transit", "partially_received"}
              /\ phase[w] = "idle" /\ received + 2 <= shipped
              /\ received' = received + 2
              /\ status' = IF received' = shipped THEN "received" ELSE "partially_received"
              /\ phase' = [phase EXCEPT ![w] = "done"]
              /\ UNCHANGED shipped
Read(w) == /\ ~Atomic /\ status \in {"in_transit", "partially_received"}
           /\ phase[w] = "idle" /\ received + 2 <= shipped
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<shipped, received, status>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ received' = received + 2
             /\ status' = IF received' = shipped THEN "received" ELSE "partially_received"
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED shipped
Next == Ship \/ (\E w \in Workers: Receive(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
WithinShipped == received <= shipped
TypeOK == /\ shipped \in Nat /\ received \in Nat
          /\ status \in {"draft", "in_transit", "partially_received", "received"}
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
