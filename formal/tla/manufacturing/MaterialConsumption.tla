------------------------ MODULE MaterialConsumption ------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Reserved == 3
VARIABLES consumed, booked, phase
vars == <<consumed, booked, phase>>
Init == /\ consumed = 0
        /\ booked = [w \in Workers |-> 0]
        /\ phase = [w \in Workers |-> "idle"]
Consume(w) == /\ Atomic /\ phase[w] = "idle" /\ consumed + 2 <= Reserved
              /\ consumed' = consumed + 2
              /\ booked' = [booked EXCEPT ![w] = 2]
              /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ consumed + 2 <= Reserved
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<consumed, booked>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ consumed' = consumed + 2
             /\ booked' = [booked EXCEPT ![w] = 2]
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Consume(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
WithinReservation == consumed <= Reserved
BookedEqualsConsumed == consumed = booked["a"] + booked["b"]
TypeOK == /\ consumed \in Nat
          /\ booked \in [Workers -> Nat]
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
