--------------------------- MODULE CommitCheckout ---------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicCommit
ASSUME AtomicCommit \in BOOLEAN
VARIABLES cart, orders, holds, payments
vars == <<cart, orders, holds, payments>>

Init == /\ cart = "active" /\ orders = 0 /\ holds = 0 /\ payments = 0
Commit == /\ AtomicCommit /\ cart = "active"
          /\ cart' = "completed" /\ orders' = 1
          /\ holds' = 1 /\ payments' = 1
\* A split implementation can expose an order before the stock and pending
\* payment rows have been committed. Failed stock then strands the order.
CreateOrderSplit == /\ ~AtomicCommit /\ cart = "active"
                    /\ cart' = "completed" /\ orders' = 1
                    /\ UNCHANGED <<holds, payments>>
FinishSplit == /\ ~AtomicCommit /\ cart = "completed" /\ holds = 0
               /\ holds' = 1 /\ payments' = 1
               /\ UNCHANGED <<cart, orders>>
Next == IF AtomicCommit THEN Commit ELSE CreateOrderSplit \/ FinishSplit
Spec == Init /\ [][Next]_vars
OrderBacked == orders <= holds /\ orders <= payments
OneCheckout == orders <= 1
TypeOK == /\ cart \in {"active", "completed"}
          /\ orders \in 0..1 /\ holds \in 0..1 /\ payments \in 0..1
=============================================================================
