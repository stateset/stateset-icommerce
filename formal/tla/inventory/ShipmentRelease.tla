-------------------------- MODULE ShipmentRelease --------------------------
EXTENDS Naturals
CONSTANT LockRows
Workers == {"ship", "release"}
VARIABLES phase, owner, cachedOpen, cachedQty, open, quantity,
          allocated, onHand, shipped, events, request
vars == <<phase, owner, cachedOpen, cachedQty, open, quantity,
          allocated, onHand, shipped, events, request>>

(* Two units on the target line, three on a same-SKU sibling. Requests
   cover partial and full shipment. Commits abstract transaction atomicity. *)
Init == /\ phase = [w \in Workers |-> "idle"] /\ owner = "none"
        /\ cachedOpen = [w \in Workers |-> FALSE]
        /\ cachedQty = [w \in Workers |-> 0]
        /\ open = TRUE /\ quantity = 2 /\ allocated = 5
        /\ onHand = 10 /\ shipped = 0 /\ request \in 1..2
        /\ events = [w \in Workers |-> 0]

Read(w) == /\ phase[w] = "idle"
           /\ (~LockRows \/ owner = "none")
           /\ owner' = IF LockRows THEN w ELSE owner
           /\ phase' = [phase EXCEPT ![w] = "read"]
           /\ cachedOpen' = [cachedOpen EXCEPT ![w] = open]
           /\ cachedQty' = [cachedQty EXCEPT ![w] = quantity]
           /\ UNCHANGED <<open, quantity, allocated, onHand, shipped, events, request>>

Commit(w) ==
    /\ phase[w] = "read"
    /\ phase' = [phase EXCEPT ![w] = "done"] /\ owner' = "none"
    /\ IF ~cachedOpen[w]
          THEN UNCHANGED <<open, quantity, allocated, onHand, shipped, events>>
          ELSE /\ events' = [events EXCEPT ![w] = @ + 1]
               /\ IF w = "ship"
                     THEN /\ onHand' = onHand - request /\ shipped' = shipped + request
                          /\ allocated' = allocated - request
                          /\ open' = (request < cachedQty[w])
                          /\ quantity' = IF open' THEN cachedQty[w] - request ELSE cachedQty[w]
                     ELSE /\ open' = FALSE /\ quantity' = cachedQty[w]
                          /\ allocated' = allocated - cachedQty[w]
                          /\ UNCHANGED <<onHand, shipped>>
    /\ UNCHANGED <<cachedOpen, cachedQty, request>>

Next == \E w \in Workers: Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
OwnerConservation == allocated = 3 + IF open THEN quantity ELSE 0
StockConservation == /\ onHand + shipped = 10 /\ allocated <= onHand
AtMostOnce == \A w \in Workers: events[w] <= 1
TypeOK == /\ phase \in [Workers -> {"idle", "read", "done"}]
          /\ owner \in Workers \cup {"none"}
          /\ cachedOpen \in [Workers -> BOOLEAN] /\ cachedQty \in [Workers -> 0..2]
          /\ open \in BOOLEAN /\ quantity \in 0..2 /\ allocated \in 0..5
          /\ onHand \in 8..10 /\ shipped \in 0..2 /\ request \in 1..2
          /\ events \in [Workers -> 0..1]
=============================================================================
