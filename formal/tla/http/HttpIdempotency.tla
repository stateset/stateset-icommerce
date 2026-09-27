-------------------------- MODULE HttpIdempotency ---------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES live, generation, response, first, phase
vars == <<live, generation, response, first, phase>>
Init == /\ live = FALSE /\ generation = 0 /\ response = "none"
        /\ first = "none" /\ phase = [w \in Workers |-> "idle"]
Put(w) == /\ Atomic /\ ~live /\ phase[w] = "idle"
          /\ live' = TRUE /\ response' = w /\ first' = w
          /\ phase' = [phase EXCEPT ![w] = "done"]
          /\ UNCHANGED generation
ReadVacant(w) == /\ ~Atomic /\ ~live /\ phase[w] = "idle"
                /\ phase' = [phase EXCEPT ![w] = "ready"]
                /\ UNCHANGED <<live, generation, response, first>>
CommitStale(w) == /\ ~Atomic /\ phase[w] = "ready"
                 /\ live' = TRUE /\ response' = w
                 /\ first' = (IF first = "none" THEN w ELSE first)
                 /\ phase' = [phase EXCEPT ![w] = "done"]
                 /\ UNCHANGED generation
Expire == /\ live /\ generation = 0
          /\ live' = FALSE /\ generation' = 1
          /\ response' = "none" /\ first' = "none"
          /\ phase' = [w \in Workers |-> "idle"]
Next == Expire \/ (\E w \in Workers: Put(w) \/ ReadVacant(w) \/ CommitStale(w))
Spec == Init /\ [][Next]_vars
FirstWriterWins == live => response = first
TypeOK == /\ live \in BOOLEAN /\ generation \in 0..1
          /\ response \in Workers \cup {"none"}
          /\ first \in Workers \cup {"none"}
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
