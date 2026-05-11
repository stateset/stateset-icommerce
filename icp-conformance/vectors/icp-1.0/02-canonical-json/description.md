# Vector 02 — Canonical JSON

**Spec sections covered:** `icp-spec/schemas/canonicalization.md` (the
load-bearing serialization rules over which all ICP signatures are
computed).

This vector exercises every canonicalization rule defined in §2 of the
canonicalization spec. Each sub-case targets a specific rule; together
they cover the surface that determines whether two implementations'
signatures interoperate.

If an implementation diverges on **any** sub-case, every cross-IUT
signature in production will fail verification. There is no "almost
canonical" — either the bytes match or the protocol breaks.

## Sub-cases

| # | Rule under test                                  | Input shape                                            |
|---|--------------------------------------------------|--------------------------------------------------------|
| 1 | Empty object                                     | `{}`                                                    |
| 2 | Single-key object                                | `{"a":"hi"}`                                            |
| 3 | Key reordering (lexicographic by JS sort)        | `{"b":2,"a":1}` → `{"a":1,"b":2}`                       |
| 4 | Nested object key reordering                     | `{"x":{"b":2,"a":1}}` → `{"x":{"a":1,"b":2}}`           |
| 5 | Array order preserved (NOT sorted)               | `{"x":[3,1,2]}` → `{"x":[3,1,2]}`                       |
| 6 | String escapes (\n, \t, \")                      | `{"x":"a\nb"}` → `{"x":"a\\nb"}`                        |
| 7 | Booleans + null                                  | `{"a":true,"b":false,"c":null}`                         |
| 8 | Integer and decimal as JSON numbers              | `{"i":42,"d":3.14}` (NOT for monetary; decimals as Number for the test only) |
| 9 | Monetary amount as string (spec rule)            | `{"price":{"amount":"29.99","currency":"USDC"}}`        |
| 10 | Empty array                                     | `{"x":[]}`                                              |
| 11 | Full purchase.create Intent (regression)        | Re-runs the canonicalization from vector 01            |

## Pass criteria

For each sub-case, the IUT MUST produce a canonical-string output
byte-identical to `expected.canonical_strings[i]`. Any divergence is a
FAIL with the diverging sub-case index reported by the runner.

## Why this matters

Vector 01 (AID derivation) signs ONE payload. Vector 02 verifies that
the canonicalization underneath any signature path produces consistent
bytes across all the shapes that appear in real ICP payloads. Without
this guarantee, vector 01 passing is a coincidence.

## Implementation hint

The reference IUT canonicalizes via either:
- The simplified subset described in `schemas/canonicalization.md` §2
  (sufficient for ICP-1.0 payload shapes), or
- A full RFC 8785 JCS implementation (e.g. the Rust `serde_jcs` crate).

Both produce identical output on the sub-cases below.
