------------------------- MODULE SerialQuarantine --------------------------
EXTENDS Naturals, TLC
CONSTANTS Atomic
ASSUME Atomic \in BOOLEAN
VARIABLES lot, serial, phase
vars == <<lot, serial, phase>>
Init == /\ lot = "active" /\ serial = "available" /\ phase = "idle"
Quarantine == /\ Atomic /\ phase = "idle" /\ lot = "active"
              /\ lot' = "quarantine" /\ serial' = "quarantined" /\ phase' = "done"
MarkLot == /\ ~Atomic /\ phase = "idle" /\ lot = "active"
           /\ lot' = "quarantine" /\ phase' = "marked"
           /\ UNCHANGED serial
MarkSerial == /\ ~Atomic /\ phase = "marked"
              /\ serial' = "quarantined" /\ phase' = "done"
              /\ UNCHANGED lot
Next == Quarantine \/ MarkLot \/ MarkSerial
Spec == Init /\ [][Next]_vars
QuarantineCascades == lot = "quarantine" => serial = "quarantined"
TypeOK == /\ lot \in {"active", "quarantine"}
          /\ serial \in {"available", "quarantined"}
          /\ phase \in {"idle", "marked", "done"}
=============================================================================
