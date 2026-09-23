---------------------------- MODULE OutboxLease ----------------------------
EXTENDS Naturals, TLC

CONSTANTS GuardAck
ASSUME GuardAck \in BOOLEAN
Workers == {"a", "b"}
VARIABLES owner, expired, publishedBy, attempts, dead
vars == <<owner, expired, publishedBy, attempts, dead>>

Init == /\ owner = "none" /\ expired = FALSE
        /\ publishedBy = "none" /\ attempts = 0 /\ dead = FALSE
Claim(w) == /\ w \in Workers /\ publishedBy = "none" /\ ~dead
            /\ (owner = "none" \/ expired)
            /\ owner' = w /\ expired' = FALSE
            /\ UNCHANGED <<publishedBy, attempts, dead>>
Expire == /\ owner # "none" /\ ~expired /\ publishedBy = "none"
          /\ expired' = TRUE
          /\ UNCHANGED <<owner, publishedBy, attempts, dead>>
Ack(w) == /\ w \in Workers /\ publishedBy = "none" /\ ~dead
          /\ (~GuardAck \/ owner = w)
          /\ publishedBy' = w
          /\ UNCHANGED <<owner, expired, attempts, dead>>
Fail(w) == /\ w \in Workers /\ owner = w /\ publishedBy = "none"
           /\ attempts < 2
           /\ attempts' = attempts + 1
           /\ dead' = (attempts + 1 = 2)
           /\ owner' = "none" /\ expired' = FALSE
           /\ UNCHANGED publishedBy
Redrive == /\ dead
           /\ dead' = FALSE /\ attempts' = 0
           /\ UNCHANGED <<owner, expired, publishedBy>>

Next == (\E w \in Workers: Claim(w) \/ Ack(w) \/ Fail(w)) \/ Expire \/ Redrive
Spec == Init /\ [][Next]_vars
AckOwned == publishedBy = "none" \/ publishedBy = owner
NoDeadPublication == dead => publishedBy = "none"
TypeOK == /\ owner \in Workers \cup {"none"}
          /\ publishedBy \in Workers \cup {"none"}
          /\ expired \in BOOLEAN /\ dead \in BOOLEAN /\ attempts \in 0..2
=============================================================================
