--------------------------- MODULE LazyExpiry ---------------------------
EXTENDS Naturals
CONSTANT LockCandidates
Workers == {"a", "b"}
VARIABLES open, owner, phase, added, allocated, events
vars == <<open, owner, phase, added, allocated, events>>

(* One expired two-unit hold and another live three-unit hold. Each caller
   discovers expiry while reserving one new unit. No negative-delta repair
   is involved in the counterexample. *)
Init == /\ open = TRUE /\ owner = "none"
        /\ phase = [w \in Workers |-> "idle"]
        /\ added = [w \in Workers |-> 0]
        /\ allocated = 5 /\ events = 0

Scan(w) == /\ phase[w] = "idle" /\ open
           /\ (~LockCandidates \/ owner = "none")
           /\ owner' = IF LockCandidates THEN w ELSE owner
           /\ phase' = [phase EXCEPT ![w] = "read"]
           /\ UNCHANGED <<open, added, allocated, events>>

ExpireAndReserve(w) == /\ phase[w] = "read" /\ allocated >= 2
                      /\ open' = FALSE /\ owner' = "none"
                      /\ phase' = [phase EXCEPT ![w] = "done"]
                      /\ added' = [added EXCEPT ![w] = 1]
                      /\ allocated' = allocated - 2 + 1
                      /\ events' = events + 1

(* A locking read which resumes after expiry re-evaluates the open predicate. *)
AlreadyExpired(w) == /\ phase[w] = "idle" /\ ~open
                     /\ (~LockCandidates \/ owner = "none")
                     /\ phase' = [phase EXCEPT ![w] = "done"]
                     /\ added' = [added EXCEPT ![w] = 1]
                     /\ allocated' = allocated + 1
                     /\ UNCHANGED <<open, owner, events>>

Next == \E w \in Workers: Scan(w) \/ ExpireAndReserve(w) \/ AlreadyExpired(w)
Spec == Init /\ [][Next]_vars
AllocatedMatchesOpen == allocated = 3 + added["a"] + added["b"] + IF open THEN 2 ELSE 0
ExpiryAtMostOnce == events <= 1
TypeOK == /\ open \in BOOLEAN /\ owner \in Workers \cup {"none"}
          /\ phase \in [Workers -> {"idle", "read", "done"}]
          /\ added \in [Workers -> 0..1] /\ allocated \in 0..7 /\ events \in 0..2
=======================================================================
