------------------------ MODULE PrepaymentSettlement ------------------------
EXTENDS Integers, Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES remaining, applied, refunded, status, phase
vars == <<remaining, applied, refunded, status, phase>>
Init == /\ remaining = 3 /\ applied = 0 /\ refunded = 0
        /\ status = "open" /\ phase = [w \in Workers |-> "idle"]
Apply(w) == /\ Atomic /\ status = "open" /\ remaining >= 2
            /\ phase[w] = "idle"
            /\ remaining' = remaining - 2 /\ applied' = applied + 2
            /\ phase' = [phase EXCEPT ![w] = "done"]
            /\ UNCHANGED <<refunded, status>>
Read(w) == /\ ~Atomic /\ status = "open" /\ remaining >= 2
           /\ phase[w] = "idle"
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<remaining, applied, refunded, status>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ remaining' = remaining - 2 /\ applied' = applied + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED <<refunded, status>>
Refund == /\ status = "open" /\ remaining > 0
          /\ refunded' = refunded + remaining /\ remaining' = 0
          /\ status' = "refunded"
          /\ UNCHANGED <<applied, phase>>
Next == Refund \/ (\E w \in Workers: Apply(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
NonnegativeRemaining == remaining >= 0
Conserved == remaining + applied + refunded = 3
TypeOK == /\ remaining \in Int /\ applied \in Nat /\ refunded \in Nat
          /\ status \in {"open", "refunded"}
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
