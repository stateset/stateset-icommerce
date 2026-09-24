------------------------- MODULE GiftCardExpiry -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES expired, active, balance, charged
vars == <<expired, active, balance, charged>>
Init == /\ expired = FALSE /\ active = TRUE /\ balance = 1 /\ charged = FALSE
Charge == /\ active /\ ~expired /\ balance = 1
          /\ balance' = 0 /\ charged' = TRUE
          /\ UNCHANGED <<expired, active>>
Expire == /\ ~expired /\ expired' = TRUE /\ active' = FALSE
          /\ UNCHANGED <<balance, charged>>
Refund == /\ charged /\ balance = 0
          /\ balance' = 1 /\ charged' = FALSE
          /\ active' = IF Atomic THEN ~expired ELSE TRUE
          /\ UNCHANGED expired
Next == Charge \/ Expire \/ Refund
Spec == Init /\ [][Next]_vars
ExpiredCannotSpend == expired => ~active
TypeOK == /\ expired \in BOOLEAN /\ active \in BOOLEAN
          /\ balance \in 0..1 /\ charged \in BOOLEAN
=============================================================================
