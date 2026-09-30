--------------------------- MODULE EscrowSettlement --------------------------
(***************************************************************************)
(* One embedded A2A escrow. Buyer/seller amounts are settlement allocations *)
(* recorded by the kernel, not claims about an external payment rail.      *)
(* A terminal status gets exactly one allocation of the escrow amount.     *)
(* AtomicSettle models the write transaction and status-conditional UPDATE; *)
(* its split alternative lets release and refund commit stale reads.       *)
(***************************************************************************)
EXTENDS Integers, FiniteSets, TLC

CONSTANTS Amount, AtomicSettle, ValidSplit
Ops == {"release", "refund"}
Statuses == {"created", "active", "disputed", "released", "refunded", "resolved"}
Terminal(s) == s \in {"released", "refunded", "resolved"}

VARIABLES status, buyer, seller, expired, pc
vars == <<status, buyer, seller, expired, pc>>

Init ==
    /\ status = "created"
    /\ buyer = 0
    /\ seller = 0
    /\ expired = FALSE
    /\ pc = [op \in Ops |-> "idle"]

Fund ==
    /\ status = "created"
    /\ ~expired
    /\ status' = "active"
    /\ UNCHANGED <<buyer, seller, expired, pc>>

Dispute ==
    /\ status = "active"
    /\ status' = "disputed"
    /\ UNCHANGED <<buyer, seller, expired, pc>>

AdvanceTime ==
    /\ ~expired
    /\ expired' = TRUE
    /\ UNCHANGED <<status, buyer, seller, pc>>

ReleaseAtomic ==
    /\ AtomicSettle
    /\ status = "active"
    /\ ~expired
    /\ status' = "released"
    /\ seller' = Amount
    /\ UNCHANGED <<buyer, expired, pc>>

RefundAtomic ==
    /\ AtomicSettle
    /\ status \in {"created", "active", "disputed"}
    /\ status' = "refunded"
    /\ buyer' = Amount
    /\ UNCHANGED <<seller, expired, pc>>

ReadRelease ==
    /\ ~AtomicSettle
    /\ pc["release"] = "idle"
    /\ status = "active"
    /\ ~expired
    /\ pc' = [pc EXCEPT !["release"] = "commit"]
    /\ UNCHANGED <<status, buyer, seller, expired>>

ReadRefund ==
    /\ ~AtomicSettle
    /\ pc["refund"] = "idle"
    /\ status \in {"created", "active", "disputed"}
    /\ pc' = [pc EXCEPT !["refund"] = "commit"]
    /\ UNCHANGED <<status, buyer, seller, expired>>

CommitRelease ==
    /\ ~AtomicSettle
    /\ pc["release"] = "commit"
    /\ ~expired
    /\ status' = "released"
    /\ seller' = Amount
    /\ pc' = [pc EXCEPT !["release"] = "idle"]
    /\ UNCHANGED <<buyer, expired>>

CommitRefund ==
    /\ ~AtomicSettle
    /\ pc["refund"] = "commit"
    /\ status' = "refunded"
    /\ buyer' = Amount
    /\ pc' = [pc EXCEPT !["refund"] = "idle"]
    /\ UNCHANGED <<seller, expired>>

ResolveSplit(b, s) ==
    /\ status = "disputed"
    /\ b \in 0..Amount
    /\ s \in 0..Amount
    /\ IF ValidSplit THEN b + s = Amount ELSE b + s >= Amount
    /\ status' = "resolved"
    /\ buyer' = b
    /\ seller' = s
    /\ UNCHANGED <<expired, pc>>

Next ==
    \/ Fund \/ Dispute \/ AdvanceTime
    \/ ReleaseAtomic \/ RefundAtomic
    \/ ReadRelease \/ ReadRefund \/ CommitRelease \/ CommitRefund
    \/ \E b, s \in 0..Amount : ResolveSplit(b, s)

Spec == Init /\ [][Next]_vars

NoDoubleSettlement == buyer + seller <= Amount
TerminalAllocated == Terminal(status) => buyer + seller = Amount
TypeOK ==
    /\ status \in Statuses
    /\ buyer \in 0..Amount
    /\ seller \in 0..Amount
    /\ expired \in BOOLEAN
    /\ pc \in [Ops -> {"idle", "commit"}]

=============================================================================
