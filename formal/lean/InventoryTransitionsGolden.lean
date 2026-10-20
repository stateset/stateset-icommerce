import Lean
import InventoryReservations
import InventoryFulfilment

open InventoryReservations Lean

def statusName : Status → String
  | .pending => "pending"
  | .allocated => "allocated"
  | .confirmed => "confirmed"
  | .fulfilled => "fulfilled"
  | .released => "released"
  | .expired => "expired"
  | .cancelled => "cancelled"

/-- A consistent target row plus another live hold. Fulfilled units have left stock. -/
def fixture (status : Status) (quantity other : Nat) : State :=
  let target : Row := ⟨status, quantity⟩
  { onHand := if status = .fulfilled then other else quantity + other,
    allocated := target.held + other,
    shipped := if status = .fulfilled then quantity else 0,
    rows := [target, ⟨.pending, other⟩] }

theorem fixture_safe (status : Status) (quantity other : Nat) :
    Safe (quantity + other) (fixture status quantity other) := by
  cases status <;> simp [fixture, Safe, openUnits, Row.held, Status.isOpen, Nat.add_comm]

def rowJson (r : Row) : Json := Json.mkObj
  [("status", toJson (statusName r.status)), ("quantity", toJson r.quantity)]

def stateJson (s : State) : Json := Json.mkObj
  [("on_hand", toJson s.onHand), ("allocated", toJson s.allocated),
   ("shipped", toJson s.shipped), ("rows", Json.arr (s.rows.map rowJson).toArray)]

/-- Expected states come from executable functions proved to refine Step. -/
def main : IO Unit := do
  let statuses : List Status :=
    [.pending, .allocated, .confirmed, .fulfilled, .released, .expired, .cancelled]
  for status in statuses do
    for quantity in [1, 2, 3] do
      for other in [1, 2, 3] do
        for time in ["past", "now", "future", "none"] do
          let due := time == "past"
          let s := fixture status quantity other
          let r : Row := ⟨status, quantity⟩
          let rest : List Row := [⟨.pending, other⟩]
          for operation in ["confirm_quantity", "expire_if_due"] do
            let amounts := if operation == "confirm_quantity" then List.range (quantity + 2) else [0]
            for amount in amounts do
              let t := if operation == "confirm_quantity" then
                confirmQuantity s [] rest r amount due else expireIfDue s [] rest r due
              IO.println (Json.mkObj [
                ("operation", toJson operation), ("amount", toJson amount),
                ("time", toJson time), ("before", stateJson s), ("after", stateJson t)]).compress
  for status in statuses do
    for quantity in [1, 2, 3] do
      for other in [1, 2, 3] do
        let s := fixture status quantity other
        let r : Row := ⟨status, quantity⟩
        for index in List.range (quantity + 3) do
          let amount := (index : Int) - 1
          let decision := match fulfilDecision r amount with
            | .invalidQuantity => "invalid_quantity"
            | .closed => "closed"
            | .excess => "excess"
            | .accepted => "accepted"
          let t := fulfilQuantity s [] [⟨.pending, other⟩] r amount
          IO.println (Json.mkObj [
            ("operation", toJson "fulfil"), ("amount", toJson amount),
            ("decision", toJson decision), ("time", toJson "none"),
            ("before", stateJson s), ("after", stateJson t)]).compress
