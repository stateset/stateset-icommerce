---------------------------- MODULE ShippedReturns ---------------------------
(***************************************************************************)
(* One order line, partially shipped in steps, with two callers creating  *)
(* returns. A live or completed return claims shipped units. Rejected or  *)
(* cancelled returns release their claim only before any disposition.     *)
(* AtomicCreate models SQLite IMMEDIATE / Postgres row locking; splitting  *)
(* its check and insert permits concurrent over-return.                   *)
(***************************************************************************)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS Ordered, MaxReturns, Workers, AtomicCreate, GuardDisposition
Statuses == {"requested", "approved", "received", "completed", "rejected", "cancelled"}
Claiming(s) == s \notin {"rejected", "cancelled"}

VARIABLES shipped, returns, seen, pc
vars == <<shipped, returns, seen, pc>>
ReturnIds == 1..Len(returns)

RECURSIVE SumClaims(_)
SumClaims(ids) ==
    IF ids = {} THEN 0
    ELSE LET i == CHOOSE x \in ids : TRUE IN
         (IF Claiming(returns[i].st) THEN returns[i].qty ELSE 0)
           + SumClaims(ids \ {i})

Claimed == SumClaims(ReturnIds)
Remaining == shipped - Claimed

Init ==
    /\ shipped = 0
    /\ returns = <<>>
    /\ seen = [w \in Workers |-> 0]
    /\ pc = [w \in Workers |-> "idle"]

Ship(n) ==
    /\ n \in 1..(Ordered - shipped)
    /\ shipped' = shipped + n
    /\ UNCHANGED <<returns, seen, pc>>

NewReturn(n) == [qty |-> n, st |-> "requested", dispositioned |-> FALSE]

CreateAtomic(w, n) ==
    /\ AtomicCreate
    /\ Len(returns) < MaxReturns
    /\ n \in 1..Remaining
    /\ returns' = Append(returns, NewReturn(n))
    /\ UNCHANGED <<shipped, seen, pc>>

ReadRemaining(w) ==
    /\ ~AtomicCreate
    /\ pc[w] = "idle"
    /\ Remaining > 0
    /\ seen' = [seen EXCEPT ![w] = Remaining]
    /\ pc' = [pc EXCEPT ![w] = "insert"]
    /\ UNCHANGED <<shipped, returns>>

InsertUnprotected(w, n) ==
    /\ ~AtomicCreate
    /\ pc[w] = "insert"
    /\ Len(returns) < MaxReturns
    /\ n \in 1..seen[w]
    /\ returns' = Append(returns, NewReturn(n))
    /\ pc' = [pc EXCEPT ![w] = "idle"]
    /\ UNCHANGED <<shipped, seen>>

Approve(i) ==
    /\ returns[i].st = "requested"
    /\ returns' = [returns EXCEPT ![i].st = "approved"]
    /\ UNCHANGED <<shipped, seen, pc>>

Receive(i) ==
    /\ returns[i].st = "approved"
    /\ returns' = [returns EXCEPT ![i].st = "received"]
    /\ UNCHANGED <<shipped, seen, pc>>

Disposition(i) ==
    /\ returns[i].st = "received"
    /\ ~returns[i].dispositioned
    /\ returns' = [returns EXCEPT ![i].dispositioned = TRUE]
    /\ UNCHANGED <<shipped, seen, pc>>

Complete(i) ==
    /\ returns[i].st = "received"
    /\ returns[i].dispositioned
    /\ returns' = [returns EXCEPT ![i].st = "completed"]
    /\ UNCHANGED <<shipped, seen, pc>>

Close(i) ==
    /\ returns[i].st \in {"requested", "approved", "received"}
    /\ (~GuardDisposition \/ ~returns[i].dispositioned)
    /\ \E target \in {"rejected", "cancelled"} :
         returns' = [returns EXCEPT ![i].st = target]
    /\ UNCHANGED <<shipped, seen, pc>>

Next ==
    \/ \E n \in 1..Ordered : Ship(n)
    \/ \E w \in Workers, n \in 1..Ordered : CreateAtomic(w, n) \/ InsertUnprotected(w, n)
    \/ \E w \in Workers : ReadRemaining(w)
    \/ \E i \in ReturnIds :
         Approve(i) \/ Receive(i) \/ Disposition(i) \/ Complete(i) \/ Close(i)

Spec == Init /\ [][Next]_vars

NoOverReturn == Claimed <= shipped
DispositionClaimsPersist ==
    \A i \in ReturnIds : returns[i].dispositioned => Claiming(returns[i].st)
TypeOK ==
    /\ shipped \in 0..Ordered
    /\ Len(returns) <= MaxReturns
    /\ \A i \in ReturnIds :
         returns[i].qty \in 1..Ordered /\ returns[i].st \in Statuses
         /\ returns[i].dispositioned \in BOOLEAN

=============================================================================
