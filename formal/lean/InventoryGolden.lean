import InventoryReservations

/-- Shared with TLC and the Rust reservation predicate conformance test. -/
def main : IO Unit := do
  let statuses : List (String × InventoryReservations.Status) :=
    [("allocated", .allocated), ("cancelled", .cancelled),
     ("confirmed", .confirmed), ("expired", .expired),
     ("fulfilled", .fulfilled), ("pending", .pending), ("released", .released)]
  for (name, status) in statuses do
    IO.println s!"{name} -> {status.isOpen}"
