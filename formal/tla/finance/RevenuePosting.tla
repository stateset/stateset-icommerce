------------------------- MODULE RevenuePosting -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES active, recognized, journal, frozen, phase
vars == <<active, recognized, journal, frozen, phase>>
Init == /\ active = TRUE /\ recognized = 0 /\ journal = 0 /\ frozen = 0
        /\ phase = [w \in Workers |-> "idle"]
Recognize(w) == /\ Atomic /\ active /\ recognized = 0
                /\ phase[w] = "idle"
                /\ recognized' = 1 /\ journal' = journal + 1
                /\ phase' = [phase EXCEPT ![w] = "done"] /\ UNCHANGED <<active, frozen>>
Read(w) == /\ ~Atomic /\ active /\ recognized = 0
           /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<active, recognized, journal, frozen>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ recognized' = recognized + 1 /\ journal' = journal + 1
             /\ phase' = [phase EXCEPT ![w] = "done"] /\ UNCHANGED <<active, frozen>>
Cancel == /\ active /\ active' = FALSE /\ frozen' = recognized
          /\ UNCHANGED <<recognized, journal, phase>>
Next == Cancel \/ (\E w \in Workers: Recognize(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
RecognizedOnce == recognized <= 1
JournalMatches == journal = recognized
CancelledFrozen == ~active => recognized = frozen
TypeOK == /\ active \in BOOLEAN /\ recognized \in Nat /\ journal \in Nat
          /\ frozen \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
