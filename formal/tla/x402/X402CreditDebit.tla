-------------------------- MODULE X402CreditDebit --------------------------
EXTENDS Integers, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Opening == 3
VARIABLES balance, credited, debited, creditDone, phase
vars == <<balance, credited, debited, creditDone, phase>>
Init == /\ balance = Opening /\ credited = 0 /\ debited = 0
        /\ creditDone = FALSE
        /\ phase = [w \in Workers |-> "idle"]
Credit == /\ ~creditDone /\ balance' = balance + 1
          /\ credited' = credited + 1 /\ creditDone' = TRUE
          /\ UNCHANGED <<debited, phase>>
Debit(w) == /\ Atomic /\ phase[w] = "idle" /\ balance >= 2
            /\ balance' = balance - 2 /\ debited' = debited + 2
            /\ phase' = [phase EXCEPT ![w] = "done"]
            /\ UNCHANGED <<credited, creditDone>>
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ balance >= 2
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<balance, credited, debited, creditDone>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ balance' = balance - 2 /\ debited' = debited + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED <<credited, creditDone>>
Next == Credit \/ (\E w \in Workers: Debit(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
NonnegativeBalance == balance >= 0
LedgerConserved == balance + debited = Opening + credited
TypeOK == /\ balance \in Int /\ credited \in Nat /\ debited \in Nat
          /\ creditDone \in BOOLEAN
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
