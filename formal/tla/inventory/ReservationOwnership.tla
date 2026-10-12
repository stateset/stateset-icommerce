------------------------ MODULE ReservationOwnership ------------------------
EXTENDS Naturals
CONSTANT GuardOwners
Lines == {"a", "b"}
Rows == {"a1", "a2", "b1"}
Owner(r) == IF r = "b1" THEN "b" ELSE "a"
Initial(r) == CASE r = "a1" -> 1 [] r = "a2" -> 2 [] OTHER -> 3
VARIABLES held, shipped, demand
vars == <<held, shipped, demand>>
Init == /\ held = [r \in Rows |-> Initial(r)]
        /\ shipped = [l \in Lines |-> 0]
        /\ demand \in [Lines -> 0..3]

(* Both lines share one SKU. Stock conservation alone cannot detect borrowing
   a sibling line's reservation. Each action is an atomic committed portion. *)
Ship(l, r, n) == /\ (~GuardOwners \/ Owner(r) = l)
                 /\ n \in 1..held[r] /\ shipped[l] + n <= demand[l]
                 /\ held' = [held EXCEPT ![r] = @ - n]
                 /\ shipped' = [shipped EXCEPT ![l] = @ + n]
                 /\ UNCHANGED demand
Next == \E l \in Lines, r \in Rows, n \in 1..3: Ship(l, r, n)
Spec == Init /\ [][Next]_vars
OwnerConservation == /\ shipped["a"] = 3 - held["a1"] - held["a2"]
                     /\ shipped["b"] = 3 - held["b1"]
NoOvership == \A l \in Lines: shipped[l] <= demand[l]
TypeOK == /\ held \in [Rows -> 0..3] /\ shipped \in [Lines -> 0..3]
          /\ demand \in [Lines -> 0..3]
=============================================================================
