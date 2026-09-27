------------------------ MODULE ReceivableSettlement -----------------------
EXTENDS Integers, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
Workers == {"a", "b"}
Invoice == 3
VARIABLES balance, paid, credited, written, phase
vars == <<balance, paid, credited, written, phase>>
Init == /\ balance = Invoice /\ paid = 0 /\ credited = 0 /\ written = 0
        /\ phase = [w \in Workers |-> "idle"]
Pay(w) == /\ Atomic /\ phase[w] = "idle" /\ balance >= 2
          /\ balance' = balance - 2 /\ paid' = paid + 2
          /\ phase' = [phase EXCEPT ![w] = "done"]
          /\ UNCHANGED <<credited, written>>
Read(w) == /\ ~Atomic /\ phase[w] = "idle" /\ balance >= 2
           /\ phase' = [phase EXCEPT ![w] = "ready"]
           /\ UNCHANGED <<balance, paid, credited, written>>
Commit(w) == /\ ~Atomic /\ phase[w] = "ready"
             /\ balance' = balance - 2 /\ paid' = paid + 2
             /\ phase' = [phase EXCEPT ![w] = "done"]
             /\ UNCHANGED <<credited, written>>
Credit == /\ balance >= 1 /\ balance' = balance - 1
          /\ credited' = credited + 1
          /\ UNCHANGED <<paid, written, phase>>
WriteOff == /\ balance > 0 /\ written' = written + balance
            /\ balance' = 0
            /\ UNCHANGED <<paid, credited, phase>>
Next == Credit \/ WriteOff \/ (\E w \in Workers: Pay(w) \/ Read(w) \/ Commit(w))
Spec == Init /\ [][Next]_vars
NoOverSettlement == paid + credited + written <= Invoice
InvoiceConserved == balance + paid + credited + written = Invoice
TypeOK == /\ balance \in Int /\ paid \in Nat /\ credited \in Nat
          /\ written \in Nat
          /\ phase \in [Workers -> {"idle", "ready", "done"}]
=============================================================================
