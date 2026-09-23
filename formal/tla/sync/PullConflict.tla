---------------------------- MODULE PullConflict ----------------------------
EXTENDS Naturals, TLC

CONSTANTS AtomicPull, RemoteWins
ASSUME /\ AtomicPull \in BOOLEAN /\ RemoteWins \in BOOLEAN
VARIABLES cursor, pending, buffered, checked, resolved
vars == <<cursor, pending, buffered, checked, resolved>>
Init == /\ cursor = 0 /\ pending = TRUE /\ buffered = FALSE
        /\ checked = FALSE /\ resolved = FALSE
\* A cursor advances only after the conflict decision has been applied.
Pull == /\ AtomicPull /\ cursor = 0
        /\ cursor' = 1
        /\ pending' = ~RemoteWins /\ buffered' = RemoteWins
        /\ resolved' = TRUE /\ UNCHANGED checked
CheckSplit == /\ ~AtomicPull /\ cursor = 0 /\ ~checked
              /\ checked' = TRUE
              /\ UNCHANGED <<cursor, pending, buffered, resolved>>
AdvanceFirst == /\ ~AtomicPull /\ checked /\ cursor = 0
                /\ cursor' = 1
                /\ UNCHANGED <<pending, buffered, checked, resolved>>
ResolveLate == /\ ~AtomicPull /\ checked /\ cursor = 1
               /\ pending' = ~RemoteWins /\ buffered' = RemoteWins
               /\ resolved' = TRUE
               /\ UNCHANGED <<cursor, checked>>
Next == Pull \/ CheckSplit \/ AdvanceFirst \/ ResolveLate
Spec == Init /\ [][Next]_vars
CursorHasDecision == cursor = 1 =>
    resolved /\ (IF RemoteWins THEN ~pending /\ buffered ELSE pending /\ ~buffered)
TypeOK == /\ cursor \in 0..1 /\ pending \in BOOLEAN
          /\ buffered \in BOOLEAN /\ checked \in BOOLEAN /\ resolved \in BOOLEAN
=============================================================================
