----------------------------- MODULE LoyaltyPoints --------------------------
EXTENDS Integers, Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
VARIABLES balance, spent, records, phase
vars == <<balance, spent, records, phase>>
Init == /\ balance = 3 /\ spent = 0 /\ records = 0
        /\ phase = [w \in Workers |-> "idle"]
Redeem(w) == /\ Atomic /\ phase[w] = "idle" /\ balance >= 2
             /\ balance' = balance - 2 /\ spent' = spent + 2
             /\ records' = records + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ balance >= 2
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<balance, spent, records>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ balance' = balance - 2 /\ spent' = spent + 2
             /\ records' = records + 1
             /\ phase' = [phase EXCEPT ![w] = "done"]
Next == \E w \in Workers: Redeem(w) \/ Read(w) \/ Commit(w)
Spec == Init /\ [][Next]_vars
NoNegativeBalance == balance >= 0
LedgerConserved == balance + spent = 3 /\ spent = 2 * records
TypeOK == /\ balance \in Int /\ spent \in Nat /\ records \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
