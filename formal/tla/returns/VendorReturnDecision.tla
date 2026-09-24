------------------------- MODULE VendorReturnDecision -------------------------
EXTENDS Naturals, TLC
CONSTANT Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES status, decisions, phase
vars == <<status, decisions, phase>>
Init == /\ status = "pending" /\ decisions = 0
        /\ phase = [w \in {"process", "cancel"} |-> "idle"]
Process == /\ Atomic /\ status = "pending" /\ phase["process"] = "idle"
           /\ status' = "processed" /\ decisions' = decisions + 1
           /\ phase' = [phase EXCEPT !["process"] = "done"]
ReadProcess == /\ ~Atomic /\ status = "pending" /\ phase["process"] = "idle"
               /\ phase' = [phase EXCEPT !["process"] = "ready"]
               /\ UNCHANGED <<status, decisions>>
CommitProcess == /\ ~Atomic /\ phase["process"] = "ready"
                 /\ status' = "processed" /\ decisions' = decisions + 1
                 /\ phase' = [phase EXCEPT !["process"] = "done"]
Cancel == /\ Atomic /\ status = "pending" /\ phase["cancel"] = "idle"
          /\ status' = "cancelled" /\ decisions' = decisions + 1
          /\ phase' = [phase EXCEPT !["cancel"] = "done"]
ReadCancel == /\ ~Atomic /\ status = "pending" /\ phase["cancel"] = "idle"
              /\ phase' = [phase EXCEPT !["cancel"] = "ready"]
              /\ UNCHANGED <<status, decisions>>
CommitCancel == /\ ~Atomic /\ phase["cancel"] = "ready"
                /\ status' = "cancelled" /\ decisions' = decisions + 1
                /\ phase' = [phase EXCEPT !["cancel"] = "done"]
Next == Process \/ ReadProcess \/ CommitProcess \/ Cancel \/ ReadCancel \/ CommitCancel
Spec == Init /\ [][Next]_vars
TerminalOnce == decisions <= 1
TypeOK == /\ status \in {"pending", "processed", "cancelled"}
          /\ decisions \in Nat
          /\ phase \in [{"process", "cancel"} -> {"idle", "ready", "done"}]
=============================================================================
