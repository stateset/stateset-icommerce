------------------------ MODULE ReservationLockOrder ------------------------
EXTENDS Naturals, Sequences
CONSTANTS PhasedShip, CloseOrder
Workers == {"ship", "close"}
Resources == {"r1", "r2", "balance"}
Plan(w) == IF w = "ship"
              THEN IF PhasedShip THEN <<"r1", "r2", "balance">>
                                ELSE <<"r1", "balance", "r2">>
              ELSE IF CloseOrder = "single" THEN <<"r2", "balance">>
                   ELSE IF CloseOrder = "identity" THEN <<"r1", "r2", "balance">>
                   ELSE <<"r2", "r1", "balance">>
VARIABLES pc, owner
vars == <<pc, owner>>
Init == /\ pc = [w \in Workers |-> 1]
        /\ owner = [r \in Resources |-> "none"]

Acquiring(w) == pc[w] <= Len(Plan(w))
Requested(w) == Plan(w)[pc[w]]
Acquire(w) == /\ Acquiring(w) /\ owner[Requested(w)] = "none"
              /\ owner' = [owner EXCEPT ![Requested(w)] = w]
              /\ pc' = [pc EXCEPT ![w] = @ + 1]
Commit(w) == /\ pc[w] = Len(Plan(w)) + 1
             /\ owner' = [r \in Resources |-> IF owner[r] = w THEN "none" ELSE owner[r]]
             /\ pc' = [pc EXCEPT ![w] = @ + 1]
Next == \E w \in Workers: Acquire(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
BlockedBy(w, other) == IF Acquiring(w) THEN owner[Requested(w)] = other ELSE FALSE
NoCircularWait == ~(BlockedBy("ship", "close") /\ BlockedBy("close", "ship"))
TypeOK == /\ pc["ship"] \in 1..5 /\ pc["close"] \in 1..(Len(Plan("close")) + 2)
          /\ owner \in [Resources -> Workers \cup {"none"}]
=============================================================================
