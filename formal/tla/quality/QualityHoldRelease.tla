------------------------- MODULE QualityHoldRelease -------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES active, releases, auditOwner, phase
vars == <<active, releases, auditOwner, phase>>
Init == /\ active = TRUE /\ releases = 0 /\ auditOwner = "none"
        /\ phase = [w \in Workers |-> "idle"]
Release(w) == /\ Atomic /\ active /\ phase[w] = "idle"
              /\ active' = FALSE /\ releases' = releases + 1
              /\ auditOwner' = w /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ active /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<active, releases, auditOwner>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ active' = FALSE /\ releases' = releases + 1
             /\ auditOwner' = w /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Release(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
AtMostOneRelease == releases <= 1
AuditMatchesRelease == (releases = 0) <=> (auditOwner = "none")
TypeOK == /\ active \in BOOLEAN /\ releases \in Nat
          /\ auditOwner \in Workers \cup {"none"}
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
