---------------------- MODULE ReservationPreview ----------------------
EXTENDS Naturals

(* Preview/apply parity for an unchanged snapshot and expiry decision.
   Balance versions change on allocation deltas, not on row-only changes.
   Rejected commands are outside this state machine. *)
CONSTANT GuardPrediction
VARIABLES status, operation, due, phase, predicted, actual
vars == <<status, operation, due, phase, predicted, actual>>

Statuses == {"pending", "allocated", "confirmed", "fulfilled",
             "released", "expired", "cancelled"}
Operations == {"confirm", "split", "release"}
Open == {"pending", "allocated", "confirmed"}
Accepted == ~(operation # "release" /\ status \in {"released", "cancelled"})
            /\ ~(operation = "split" /\ status = "confirmed")

Init == /\ status \in Statuses
        /\ operation \in Operations
        /\ due \in BOOLEAN
        /\ Accepted
        /\ phase = "initial"
        /\ predicted = 10
        /\ actual = 10

Preview == /\ phase = "initial"
           /\ predicted' = 10 + IF GuardPrediction
                 THEN IF status \in Open /\
                         (operation = "release" \/ (status # "confirmed" /\ due))
                      THEN 1 ELSE 0
                 ELSE IF status \notin {"released", "cancelled", "expired"} /\
                         (due \/ operation = "release")
                      THEN 1 ELSE 0
           /\ phase' = "previewed"
           /\ UNCHANGED <<status, operation, due, actual>>

Apply == /\ phase = "previewed"
         /\ actual' = IF operation = "release" /\ status \in Open
                       THEN actual + 1
                       ELSE IF operation \in {"confirm", "split"} /\ due /\
                               status \in {"pending", "allocated"}
                            THEN actual + 1 ELSE actual
         /\ phase' = "applied"
         /\ UNCHANGED <<status, operation, due, predicted>>

Next == Preview \/ Apply
Spec == Init /\ [][Next]_vars
TypeOK == /\ status \in Statuses /\ operation \in Operations /\ due \in BOOLEAN
          /\ phase \in {"initial", "previewed", "applied"}
          /\ predicted \in 10..11 /\ actual \in 10..11
PreviewDoesNotWrite == phase = "previewed" => actual = 10
PredictionMatchesApply == phase = "applied" => predicted = actual
=======================================================================
