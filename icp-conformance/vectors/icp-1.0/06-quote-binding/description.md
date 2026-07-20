# Vector 06 — Quote Binding (§11.4 max_total ceiling)

**Spec sections covered:** ICP-1.0 §6.1 (`max_total`), §11.4 (quote-binding
attack), `schemas/error-codes.md` (`policy.quote.exceeds_max_total`).

The third operational-semantics family, and the first to test an **economic
safety rule**. §11.4 names `max_total` the protocol-level mitigation against
the quote-binding attack: *"a merchant could return an inflated Quote and rely
on a sloppy Buyer Agent to auto-accept."* The Intent's `max_total` is a
**MUST NOT** ceiling — a Merchant Quote with `total > max_total` MUST be
rejected. If two implementations disagree on that comparison, one of them
auto-accepts an overcharge the other blocks.

## Why this is a real conformance risk

The comparison is over **decimal money strings**, and the spec forbids floats
("banker's rounding errors are unacceptable in settlement",
`intent.*.schema.json` Money). An implementation that compares amounts as
IEEE-754 doubles, or — worse — as raw strings, diverges:

- Case `c09` (`quote_total = "9.9"`, `max_total = "10.0"`) is **valid**: 9.9 ≤
  10.0. A naive string comparison reports `"9.9" > "10.0"` (because `'9' >
  '1'`) and wrongly rejects it.
- Case `c07` (`"1000000000000.00"` vs `"999999999999.99"`) exceeds — the
  integer part is beyond 2⁵³, so a double-based comparison risks precision loss.
- Cases `c04`/`c05`/`c12`/`c14` (`"65.0"` vs `"65.00"`, `"65"` vs `"65.00"`,
  `"0065.00"` vs `"65.0"`) are all **equal** — trailing zeros, missing
  fraction, and leading zeros must not change the value.

## Contract

Each case supplies same-currency non-negative `intent_max_total` and
`quote_total` Money values. The IUT returns, per case:

- `{"valid": true}` when `quote_total ≤ intent_max_total`
- `{"error": "policy.quote.exceeds_max_total"}` when `quote_total > intent_max_total`

Comparison MUST be exact decimal. stdin: `inputs.json`. stdout:

```json
{ "decisions": { "c01_under": {"valid": true},
                 "c03_over_by_a_cent": {"error": "policy.quote.exceeds_max_total"}, ... } }
```

Expected outputs (`expected.json`) are generated from an exact
integer-and-fraction comparator — see `_provenance`. Currency mismatch and
subscription `max_total_per_period` (§6.2, the same rule generalized) are
deferred to later families.
