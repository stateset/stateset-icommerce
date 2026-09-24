-------------------------- MODULE StoredValueSpend -------------------------
EXTENDS Integers, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Opening == 3
VARIABLES balance, charged, refunded, phase
vars == <<balance, charged, refunded, phase>>
Init == /\ balance = Opening
        /\ charged = [w \in Workers |-> 0]
        /\ refunded = [w \in Workers |-> 0]
        /\ phase = [w \in Workers |-> "idle"]
Charge(w) == /\ Atomic /\ phase[w] = "idle" /\ balance >= 2
             /\ balance' = balance - 2
             /\ charged' = [charged EXCEPT ![w] = 2]
             /\ phase' = [phase EXCEPT ![w] = "charged"]
             /\ UNCHANGED refunded
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ balance >= 2
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<balance, charged, refunded>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ balance' = balance - 2
             /\ charged' = [charged EXCEPT ![w] = 2]
             /\ phase' = [phase EXCEPT ![w] = "charged"]
             /\ UNCHANGED refunded
Refund(w) == /\ phase[w] = "charged"
             /\ balance' = balance + charged[w]
             /\ refunded' = [refunded EXCEPT ![w] = charged[w]]
             /\ phase' = [phase EXCEPT ![w] = "refunded"]
             /\ UNCHANGED charged
Next == \E w \in Workers: Charge(w) \/ Read(w) \/ Commit(w) \/ Refund(w)
Spec == Init /\ [][Next]_vars
NonnegativeBalance == balance >= 0
Conserved == balance + charged["a"] + charged["b"]
                       - refunded["a"] - refunded["b"] = Opening
TypeOK == /\ balance \in Int
          /\ charged \in [Workers -> Nat]
          /\ refunded \in [Workers -> Nat]
          /\ phase \in [Workers -> {"idle", "ready", "charged", "refunded"}]
=============================================================================
