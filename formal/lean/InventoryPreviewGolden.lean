import InventoryPreview

def main : IO Unit := do
  let statuses : List (String × InventoryReservations.Status) :=
    [("allocated", .allocated), ("cancelled", .cancelled),
     ("confirmed", .confirmed), ("expired", .expired),
     ("fulfilled", .fulfilled), ("pending", .pending), ("released", .released)]
  for (name, status) in statuses do
    for operation in ["confirm", "split", "release"] do
      for due in [false, true] do
        let delta := if InventoryReservations.changesBalance status (operation == "release") due then 1 else 0
        IO.println s!"{name} {operation} {due} {delta}"
