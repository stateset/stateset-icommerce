---------------------------- MODULE LocationMove ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicMove
ASSUME AtomicMove \in BOOLEAN
VARIABLES source, destination, reserved, pending
vars == <<source, destination, reserved, pending>>

Init == /\ source = 3
        /\ destination = 1
        /\ reserved = 1
        /\ pending = FALSE

\* A move of one unit checks unreserved stock and commits both location
\* changes in the same transaction.
Move == /\ source > reserved
        /\ source' = source - 1
        /\ destination' = destination + 1
        /\ UNCHANGED <<reserved, pending>>
DebitSplit == /\ ~AtomicMove /\ ~pending /\ source > reserved
              /\ source' = source - 1
              /\ pending' = TRUE
              /\ UNCHANGED <<destination, reserved>>
CreditSplit == /\ ~AtomicMove /\ pending
               /\ destination' = destination + 1
               /\ pending' = FALSE
               /\ UNCHANGED <<source, reserved>>

Next == (IF AtomicMove THEN Move ELSE DebitSplit \/ CreditSplit)
Spec == Init /\ [][Next]_vars
StockConserved == source + destination = 4
ReservedStockRemains == source >= reserved
TypeOK == /\ source \in 0..3 /\ destination \in 0..4
          /\ reserved = 1 /\ pending \in BOOLEAN
=============================================================================
