import Lean
import ReservationOwnership

open Lean ReservationOwnership

def rowJson (row : Hold) : Json := Json.mkObj
  [("owner", toJson row.owner), ("quantity", toJson row.quantity)]

def main : IO Unit := do
  for scale in [0, 2, 6] do
    let unit := 10 ^ scale
    for a in [1, 2, 3] do
      for b in [1, 2, 3] do
        let split := [Hold.mk 0 1, Hold.mk 1 1,
          Hold.mk 0 (a * unit - 1), Hold.mk 1 (b * unit - 1)]
          |>.filter (fun row => row.quantity > 0)
        let variants := [
          ("whole", [Hold.mk 0 (a * unit), Hold.mk 1 (b * unit)]),
          ("split", split), ("reversed", split.reverse),
          ("sibling_surplus", [Hold.mk 0 (a * unit - 1), Hold.mk 1 (b * unit + 1)]),
          ("foreign_owner", [Hold.mk 2 (a * unit), Hold.mk 1 (b * unit)])]
        for (variant, rows) in variants do
          for ra in List.range (a + 2) do
            for rb in List.range (b + 2) do
              let accepted := ra + rb > 0 && ra ≤ a && rb ≤ b &&
                held 0 rows == a * unit && held 1 rows == b * unit &&
                rows.all (fun row => row.owner < 2 && row.quantity > 0)
              let request := fun owner => if owner = 0 then ra * unit
                else if owner = 1 then rb * unit else 0
              let amounts := if accepted then (select request rows).map Hold.quantity else []
              IO.println (Json.mkObj [
                ("scale", toJson scale), ("demand", toJson [a, b]),
                ("request", toJson [ra, rb]), ("variant", toJson variant),
                ("rows", Json.arr (rows.map rowJson).toArray),
                ("accepted", toJson accepted), ("amounts", toJson amounts)]).compress
