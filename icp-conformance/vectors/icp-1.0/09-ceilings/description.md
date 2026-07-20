# Vector 09 — Refund & Payout Ceilings

**Spec sections covered:** ICP-1.0 §6.2 (`purchase.return` `max_refund`),
§6.6 (`payout.request` `max_per_payout`), `schemas/error-codes.md`
(`policy.return.exceeds_max_refund`, `policy.payout.exceeds_max_per_payout`).

The economic-safety story completed. Family `06-quote-binding` pinned the
`max_total` ceiling; the protocol has the same authoritative-ceiling pattern
in two more places, each with its own `policy.*` code:

- A Merchant `ReturnAuthorization` with `refund.amount > max_refund` MUST be
  rejected → `policy.return.exceeds_max_refund`.
- A `payout.request` whose `amount` exceeds the PrincipalBinding's
  `max_per_payout` cap MUST be rejected → `policy.payout.exceeds_max_per_payout`.

Both are the **same exact-decimal comparison** as `06` (the spec bans float
money everywhere), so this family reuses each IUT's comparator and only
dispatches the error code on `kind`. It exists to lock the two additional
codes and prove the comparator generalizes — the decimal traps
(`ret04`: `9.9 ≤ 10.0`; `pay04`: `> 2⁵³`) are carried over.

## Contract

Each case supplies a `kind` (`"return"` or `"payout"`) and two same-currency
non-negative Money values: `value` (the requested refund/payout) and
`ceiling` (`max_refund` / `max_per_payout`). The IUT returns, per case:

- `{"valid": true}` when `value ≤ ceiling`
- `{"error": "<kind-specific code>"}` when `value > ceiling`

stdin: `inputs.json`. stdout: `{ "decisions": { "ret01_under": {"valid": true},
"pay03_over": {"error": "policy.payout.exceeds_max_per_payout"}, ... } }`.

Expected outputs are generated with the same comparator as `06` — see
`_provenance`. The subscription `max_total_per_period` ceiling (§6.2, the same
rule again) and currency-mismatch handling are deferred.
