-------------------------- MODULE ReservationSplits --------------------------
\* A two-unit source reservation, another two-unit hold, and one split child.
\* Abstracts confirm_reservation_quantity_in_tx_with_now and expiry-by-ID.
\* The clock may cross expiry between calls; each guarded action is atomic.
EXTENDS Naturals, TLC
CONSTANTS GuardSplit, GuardExpiry, AtomicSplit
ASSUME GuardSplit \in BOOLEAN /\ GuardExpiry \in BOOLEAN /\ AtomicSplit \in BOOLEAN
VARIABLES status, quantity, child, onHand, allocated, shipped, due, inserting
vars == <<status, quantity, child, onHand, allocated, shipped, due, inserting>>
Open == status \in {"pending", "confirmed"}
Init == /\ status = "pending" /\ quantity = 2 /\ child = 0
        /\ onHand = 4 /\ allocated = 4 /\ shipped = 0
        /\ due = FALSE /\ inserting = FALSE

Tick == /\ ~due /\ due' = TRUE
        /\ UNCHANGED <<status, quantity, child, onHand, allocated, shipped, inserting>>
Confirm == /\ status = "pending" /\ ~due /\ ~inserting
           /\ status' = "confirmed"
           /\ UNCHANGED <<quantity, child, onHand, allocated, shipped, due, inserting>>
Split == /\ (status = "pending" \/ (~GuardSplit /\ status = "fulfilled"))
         /\ ~due /\ quantity = 2 /\ child = 0 /\ ~inserting
         /\ quantity' = 1
         /\ child' = IF AtomicSplit THEN 1 ELSE 0
         /\ inserting' = ~AtomicSplit
         /\ UNCHANGED <<status, onHand, allocated, shipped, due>>
FinishInsert == /\ inserting /\ child' = 1 /\ inserting' = FALSE
                /\ UNCHANGED <<status, quantity, onHand, allocated, shipped, due>>
Fulfil(n) == /\ Open /\ ~inserting /\ n \in 1..quantity
             /\ onHand' = onHand - n /\ allocated' = allocated - n
             /\ shipped' = shipped + n
             /\ status' = IF n = quantity THEN "fulfilled" ELSE status
             /\ quantity' = IF n = quantity THEN quantity ELSE quantity - n
             /\ UNCHANGED <<child, due, inserting>>
Expire == /\ due /\ ~inserting
          /\ (Open \/ (~GuardExpiry /\ status = "fulfilled"))
          \* Require the ordinary, nonnegative allocation path, not drift repair.
          /\ allocated >= quantity /\ allocated' = allocated - quantity
          /\ status' = "expired"
          /\ UNCHANGED <<quantity, child, onHand, shipped, due, inserting>>
Next == Tick \/ Confirm \/ Split \/ FinishInsert \/ Expire \/ (\E n \in 1..2 : Fulfil(n))
Spec == Init /\ [][Next]_vars
AllocatedMatchesOpen == allocated = 2 + child + IF Open THEN quantity ELSE 0
StockBounds == allocated <= onHand /\ onHand <= 4
StockConserved == onHand + shipped = 4
TypeOK == /\ status \in {"pending", "confirmed", "fulfilled", "expired"}
          /\ quantity \in 1..2 /\ child \in 0..1
          /\ onHand \in 0..4 /\ allocated \in 0..4 /\ shipped \in 0..4
          /\ due \in BOOLEAN /\ inserting \in BOOLEAN
=============================================================================
