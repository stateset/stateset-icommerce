------------------------- MODULE AssetDisposal -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES live, accumulated, disposalAccumulated, phase
vars == <<live, accumulated, disposalAccumulated, phase>>
Init == /\ live = TRUE /\ accumulated = 0
        /\ disposalAccumulated = 0 /\ phase = "idle"
Read == /\ ~Atomic /\ live /\ accumulated = 0 /\ phase = "idle"
        /\ phase' = "ready"
        /\ UNCHANGED <<live, accumulated, disposalAccumulated>>
Post == /\ live /\ accumulated = 0 /\ phase = "idle"
        /\ accumulated' = 1 /\ phase' = "done"
        /\ UNCHANGED <<live, disposalAccumulated>>
Commit == /\ ~Atomic /\ phase = "ready"
          /\ accumulated' = accumulated + 1 /\ phase' = "done"
          /\ UNCHANGED <<live, disposalAccumulated>>
Dispose == /\ live /\ live' = FALSE
           /\ disposalAccumulated' = accumulated
           /\ UNCHANGED <<accumulated, phase>>
Next == Dispose \/ (IF Atomic THEN Post ELSE Read \/ Commit)
Spec == Init /\ [][Next]_vars
DisposalFrozen == ~live => accumulated = disposalAccumulated
TypeOK == /\ live \in BOOLEAN /\ accumulated \in Nat
          /\ disposalAccumulated \in Nat
          /\ phase \in {"idle", "ready", "done"}
=============================================================================
