---------------------------- MODULE LotGenealogy ----------------------------
EXTENDS Naturals, TLC

CONSTANTS RecordAllParents
ASSUME RecordAllParents \in BOOLEAN
VARIABLES stage, edges
vars == <<stage, edges>>

\* A and B are source lots. C is split from A; D merges C and B.
Init == /\ stage = 0
        /\ edges = {}
Split == /\ stage = 0
         /\ stage' = 1
         /\ edges' = edges \cup {<<"A", "C">>}
Merge == /\ stage = 1
         /\ stage' = 2
         /\ edges' = edges \cup {<<"C", "D">>}
                      \cup (IF RecordAllParents THEN {<<"B", "D">>} ELSE {})

Next == Split \/ Merge
Spec == Init /\ [][Next]_vars

TraceComplete == stage = 2 =>
  /\ <<"A", "C">> \in edges
  /\ <<"C", "D">> \in edges
  /\ <<"B", "D">> \in edges
TypeOK == /\ stage \in 0..2
          /\ edges \subseteq {<<"A", "C">>, <<"C", "D">>, <<"B", "D">>}
=============================================================================
