------------------------- MODULE PrintTransitions -------------------------
(* Prints the spec's CanTransition relation, one allowed move per line, so *)
(* scripts/check.sh can compare it with can_transition.golden -- the same  *)
(* file the Rust test holds PaymentTransactionStatus::can_transition_to to.*)
EXTENDS PaymentRefunds

ASSUME \A f \in Statuses, t \in Statuses :
          (f # t /\ CanTransition(f, t)) => PrintT(f \o " -> " \o t)
===========================================================================
