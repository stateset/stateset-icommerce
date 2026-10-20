import ReceiptPromotion

def main : IO Unit := do
  let statuses : List (String × ReceiptPromotion.Status) :=
    [("previewed", .previewed), ("succeeded", .succeeded), ("rejected", .rejected), ("failed", .failed)]
  for (name, status) in statuses do
    for apply in [false, true] do
      for same in [false, true] do
        for sealed in [false, true] do
          let decision := match ReceiptPromotion.decide status apply same sealed with
            | .tampered => "tampered"
            | .conflict => "conflict"
            | .promote => "promote"
            | .replay => "replay"
          IO.println s!"{name} {apply} {same} {sealed} {decision}"
