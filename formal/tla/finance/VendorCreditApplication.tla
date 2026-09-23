--------------------- MODULE VendorCreditApplication -----------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Original == 3
VARIABLES remaining, applied, phase
vars == <<remaining, applied, phase>>
Init == /\ remaining = Original /\ applied = 0
        /\ phase = [w \in Workers |-> "idle"]
Apply(w) == /\ Atomic /\ phase[w] = "idle" /\ remaining >= 2
            /\ remaining' = remaining - 2 /\ applied' = applied + 2
            /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ remaining >= 2
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<remaining, applied>>
Insert(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ remaining' = remaining - 2 /\ applied' = applied + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
Reverse(w) == /\ phase[w] = "done"
              /\ remaining' = remaining + 2 /\ applied' = applied - 2
              /\ phase' = [phase EXCEPT ![w] = "reversed"]
Next == \E w \in Workers: Apply(w) \/ Read(w) \/ Insert(w) \/ Reverse(w)
Spec == Init /\ [][Next]_vars
CreditConserved == remaining + applied = Original
NonnegativeRemaining == remaining >= 0
TypeOK == /\ remaining \in Nat /\ applied \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done", "reversed"}]
=============================================================================
