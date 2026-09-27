------------------------- MODULE BackorderAllocation -----------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Ordered == 3
VARIABLES remaining, fulfilled, allocated, phase
vars == <<remaining, fulfilled, allocated, phase>>
Init == /\ remaining = Ordered /\ fulfilled = 0 /\ allocated = 0
        /\ phase = [w \in Workers |-> "idle"]
Allocate(w) == /\ Atomic /\ phase[w] = "idle"
               /\ allocated + 2 <= remaining
               /\ allocated' = allocated + 2
               /\ phase' = [phase EXCEPT ![w] = "done"]
               /\ UNCHANGED <<remaining, fulfilled>>
Read(w) == /\ ~Atomic /\ phase[w] = "idle"
           /\ allocated + 2 <= remaining
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<remaining, fulfilled, allocated>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ allocated' = allocated + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED <<remaining, fulfilled>>
Fulfill == /\ allocated > 0 /\ allocated <= remaining
           /\ fulfilled' = fulfilled + allocated
           /\ remaining' = remaining - allocated
           /\ allocated' = 0
           /\ UNCHANGED phase
Next == Fulfill \/ (\E w \in Workers: Allocate(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
NoOverAllocation == allocated <= remaining
OrderConserved == fulfilled + remaining = Ordered
TypeOK == /\ remaining \in Nat /\ fulfilled \in Nat /\ allocated \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
