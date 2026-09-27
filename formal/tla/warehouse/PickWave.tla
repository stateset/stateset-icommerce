----------------------------- MODULE PickWave -----------------------------
EXTENDS Naturals, TLC

CONSTANTS GuardCompletion
ASSUME GuardCompletion \in BOOLEAN
Picks == {"a", "b"}
VARIABLES wave, pick
vars == <<wave, pick>>
Init == /\ wave = "released"
        /\ pick = [p \in Picks |-> "open"]
Finish(p) == /\ p \in Picks /\ pick[p] = "open"
             /\ pick' = [pick EXCEPT ![p] = "done"]
             /\ UNCHANGED wave
CancelPick(p) == /\ p \in Picks /\ pick[p] = "open"
                 /\ pick' = [pick EXCEPT ![p] = "cancelled"]
                 /\ UNCHANGED wave
Complete == /\ wave = "released"
            /\ (GuardCompletion => \A p \in Picks: pick[p] # "open")
            /\ wave' = "completed"
            /\ UNCHANGED pick
Next == Complete \/ (\E p \in Picks: Finish(p) \/ CancelPick(p))
Spec == Init /\ [][Next]_vars
CompletedOnlyWhenFinal == wave = "completed" => \A p \in Picks: pick[p] # "open"
TypeOK == /\ wave \in {"released", "completed"}
          /\ pick \in [Picks -> {"open", "done", "cancelled"}]
=============================================================================
