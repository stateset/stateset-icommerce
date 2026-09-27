--------------------------- MODULE PaymentLedger ---------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES accountBalance, ledgerBalance, phase
vars == <<accountBalance, ledgerBalance, phase>>
Init == /\ accountBalance = 3 /\ ledgerBalance = 3 /\ phase = "idle"
ApplyPayment == /\ Atomic /\ phase = "idle"
                /\ accountBalance' = 1 /\ ledgerBalance' = 1 /\ phase' = "done"
RecordPayment == /\ ~Atomic /\ phase = "idle"
                 /\ accountBalance' = 1 /\ phase' = "payment_written"
                 /\ UNCHANGED ledgerBalance
PostJournal == /\ ~Atomic /\ phase = "payment_written"
               /\ ledgerBalance' = 1 /\ phase' = "done"
               /\ UNCHANGED accountBalance
Next == ApplyPayment \/ RecordPayment \/ PostJournal
Spec == Init /\ [][Next]_vars
Reconciled == accountBalance = ledgerBalance
TypeOK == /\ accountBalance \in Nat /\ ledgerBalance \in Nat
          /\ phase \in {"idle", "payment_written", "done"}
=============================================================================
