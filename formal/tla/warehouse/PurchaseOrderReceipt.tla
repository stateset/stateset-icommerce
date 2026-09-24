------------------------- MODULE PurchaseOrderReceipt -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES open, received, frozen, phase
vars == <<open, received, frozen, phase>>
Init == /\ open = TRUE /\ received = 0 /\ frozen = 0
        /\ phase = [w \in Workers |-> "idle"]
Receive(w) == /\ Atomic /\ open /\ received + 1 <= 1
              /\ phase[w] = "idle" /\ received' = received + 1
              /\ phase' = [phase EXCEPT ![w] = "done"] /\ UNCHANGED <<open, frozen>>
Read(w) == /\ ~Atomic /\ open /\ received + 1 <= 1
           /\ phase[w] = "idle" /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<open, received, frozen>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ received' = received + 1
             /\ phase' = [phase EXCEPT ![w] = "done"] /\ UNCHANGED <<open, frozen>>
Cancel == /\ open /\ open' = FALSE /\ frozen' = received
          /\ UNCHANGED <<received, phase>>
Next == Cancel \/ (\E w \in Workers: Receive(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
WithinOrder == received <= 1
ClosedFrozen == ~open => received = frozen
TypeOK == /\ open \in BOOLEAN /\ received \in Nat /\ frozen \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
