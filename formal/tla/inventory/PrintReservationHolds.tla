------------------------ MODULE PrintReservationHolds ------------------------
\* The same table is emitted by Lean and consumed by the Rust conformance test.
EXTENDS InventoryReservations, Sequences

ASSUME \A s \in Statuses \ {"absent"} :
  PrintT(s \o " -> " \o IF Open(s) THEN "true" ELSE "false")
=============================================================================
