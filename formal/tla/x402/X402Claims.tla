----------------------------- MODULE X402Claims -----------------------------
(***************************************************************************)
(* Two creators for the same cart. A created, signed, sequenced, batched   *)
(* or settled intent still claims the cart. Failed, expired and cancelled *)
(* intents release the claim. The repositories serialize creation and a   *)
(* unique claim-key index backs it up. AtomicClaim = FALSE models the old  *)
(* accessor check followed by a separate insert.                         *)
(***************************************************************************)
EXTENDS Integers, FiniteSets, TLC

CONSTANT AtomicClaim
Ids == {"i1", "i2"}
Statuses == {"absent", "created", "signed", "sequenced", "batched",
             "settled", "failed", "expired", "cancelled"}
Claiming(s) == s \in {"created", "signed", "sequenced", "batched", "settled"}

VARIABLES status, pc
vars == <<status, pc>>
Active == {i \in Ids : Claiming(status[i])}

Init ==
    /\ status = [i \in Ids |-> "absent"]
    /\ pc = [i \in Ids |-> "idle"]

CreateAtomic(i) ==
    /\ AtomicClaim
    /\ status[i] = "absent"
    /\ Active = {}
    /\ status' = [status EXCEPT ![i] = "created"]
    /\ UNCHANGED pc

ReadClaim(i) ==
    /\ ~AtomicClaim
    /\ status[i] = "absent"
    /\ pc[i] = "idle"
    /\ Active = {}
    /\ pc' = [pc EXCEPT ![i] = "insert"]
    /\ UNCHANGED status

InsertUnprotected(i) ==
    /\ ~AtomicClaim
    /\ status[i] = "absent"
    /\ pc[i] = "insert"
    /\ status' = [status EXCEPT ![i] = "created"]
    /\ pc' = [pc EXCEPT ![i] = "idle"]

Advance(i) ==
    /\ \E pair \in {<<"created", "signed">>, <<"signed", "sequenced">>,
                    <<"sequenced", "batched">>, <<"batched", "settled">>} :
         /\ status[i] = pair[1]
         /\ status' = [status EXCEPT ![i] = pair[2]]
    /\ UNCHANGED pc

Close(i) ==
    /\ status[i] \in {"created", "signed", "sequenced", "batched"}
    /\ \E target \in {"failed", "expired", "cancelled"} :
         status' = [status EXCEPT ![i] = target]
    /\ UNCHANGED pc

Next == \E i \in Ids :
    CreateAtomic(i) \/ ReadClaim(i) \/ InsertUnprotected(i) \/ Advance(i) \/ Close(i)
Spec == Init /\ [][Next]_vars

NoDuplicateClaim == Cardinality(Active) <= 1
TypeOK == status \in [Ids -> Statuses] /\ pc \in [Ids -> {"idle", "insert"}]

=============================================================================
