# Vector 04 — Escrow Lifecycle State Machine

**Spec sections covered:** ICP-1.0 §8 (Escrow state machine),
`schemas/error-codes.md` `escrow.*` namespace.

The first conformance family to exercise ICP's *operational* semantics
rather than its cryptographic plumbing. Every Settler and every SDK that
tracks escrow state runs this machine; two implementations that disagree
on a single transition will disagree about who holds the money.

## Part 1 — Transition matrix (`transition_cases`, t01–t30)

The full cross product of the 5 escrow states × 6 normative triggers.
Exactly 6 combinations are valid per the §8 table; every other combination
MUST be rejected:

- `funded` × `payment_confirmed` → `escrow.already_funded` (re-fund attempt)
- all other invalid combinations → `escrow.wrong_state`

Trigger identifiers map 1:1 to the §8 table rows:

| Trigger token | §8 table row |
|---|---|
| `payment_confirmed` | Buyer payment confirmed by Settler |
| `fulfillment_confirmed_window_elapsed` | Fulfillment confirmed AND dispute window elapsed |
| `dispute_raised` | Buyer or Merchant raises Dispute |
| `resolution_favors_merchant` | Dispute resolution favors Merchant |
| `resolution_favors_buyer` | Dispute resolution favors Buyer |
| `merchant_cancel_or_expiry` | Merchant cancels OR fulfillment expires |

## Part 2 — Event replay (`replay_cases`, r01–r10)

§8: implementations **MUST** be able to reconstruct escrow state by
replaying EscrowEvents from `seq=0`. Each case supplies an event list;
the IUT replays from the initial `pending` state and reports the final
state, or the first error encountered:

- `r01` — empty log → `pending` (creation state)
- `r02`–`r05` — the four terminal paths (release, dispute→refund,
  dispute→release, cancel)
- `r06` — gap in `seq` (0, 2) → `escrow.seq_out_of_order`
- `r07` — duplicate `seq` → `escrow.seq_out_of_order`
- `r08` — log starting at `seq=1` → `escrow.seq_out_of_order`
- `r09`, `r10` — structurally ordered logs containing an invalid
  transition → the transition's error code

## Adapter contract

stdin: `inputs.json`. stdout:

```json
{
  "transitions": { "t01": {"state": "funded"} , "t02": {"error": "escrow.wrong_state"}, ... },
  "replays":     { "r01": {"final_state": "pending"}, "r06": {"error": "escrow.seq_out_of_order"}, ... }
}
```

Expected outputs are generated mechanically from the §8 table — see
`_provenance` in `expected.json`.
