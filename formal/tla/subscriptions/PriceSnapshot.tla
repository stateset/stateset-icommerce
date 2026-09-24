--------------------------- MODULE PriceSnapshot ---------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicSnapshot
ASSUME AtomicSnapshot \in BOOLEAN
VARIABLES price, cycle, cyclePrice, priceAtInsert, readPrice
vars == <<price, cycle, cyclePrice, priceAtInsert, readPrice>>
Init == /\ price = 1 /\ cycle = "absent" /\ cyclePrice = 0
        /\ priceAtInsert = 0 /\ readPrice = 0
ChangePlan == /\ price = 1 /\ price' = 2
              /\ UNCHANGED <<cycle, cyclePrice, priceAtInsert, readPrice>>
SeedAtomic == /\ AtomicSnapshot /\ cycle = "absent"
              /\ cycle' = "scheduled"
              /\ cyclePrice' = price /\ priceAtInsert' = price
              /\ UNCHANGED <<price, readPrice>>
ReadSplit == /\ ~AtomicSnapshot /\ cycle = "absent" /\ readPrice = 0
             /\ readPrice' = price
             /\ UNCHANGED <<price, cycle, cyclePrice, priceAtInsert>>
InsertSplit == /\ ~AtomicSnapshot /\ cycle = "absent" /\ readPrice > 0
               /\ cycle' = "scheduled"
               /\ cyclePrice' = readPrice /\ priceAtInsert' = price
               /\ UNCHANGED <<price, readPrice>>
Next == ChangePlan \/ SeedAtomic \/ ReadSplit \/ InsertSplit
Spec == Init /\ [][Next]_vars
NoStaleCyclePrice == cycle = "scheduled" => cyclePrice = priceAtInsert
TypeOK == /\ price \in 1..2 /\ cycle \in {"absent", "scheduled"}
          /\ cyclePrice \in 0..2 /\ priceAtInsert \in 0..2 /\ readPrice \in 0..2
=============================================================================
