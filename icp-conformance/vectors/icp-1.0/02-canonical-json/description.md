# Vector 02 — Canonical JSON

**Spec sections covered:** `icp-spec/schemas/canonicalization.md` (the
load-bearing serialization rules over which all ICP signatures are
computed).

This vector exercises every canonicalization rule defined in §1 of the
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
| 12 | No HTML-safety escaping of `<`, `>`, `&` (RFC 8785 §3.2.2.2) | `{"a":"<&>"}` → `{"a":"<&>"}` (NOT `\\u003c&\\u003e`) |
| 13 | URL with query params survives byte-for-byte    | `{"url":"https://x.test/a?b=1&c=<2>"}` → unchanged      |
| 14 | Non-minimal number forms minimized (RFC 8785 §3.2.2.3) | `{"a":1.50,"b":10.0,"c":0.500}` → `{"a":1.5,"b":10,"c":0.5}` |
| 15 | Exponent boundaries (ES Number::toString)       | `{"a":1e21,"b":1e-6,"c":1e-7}` → `{"a":1e+21,"b":0.000001,"c":1e-7}` |
| 16 | Largest exact float64 integer, no decimal point | `{"n":9007199254740991}` → `{"n":9007199254740991}`     |
| 17 | Negative zero serializes as `0`                 | `{"n":-0}` → `{"n":0}`                                  |
| 18 | U+2028/U+2029 stay raw (no JSONP-safety escape) | `{"a":"x\u2028y\u2029z"}` → raw separators in output    |
| 19 | `\b`/`\f` two-char escapes (NOT `\u0008`/`\u000c`) | `{"a":"x\by\fz"}` → `{"a":"x\by\fz"}` with two-char escapes |
| 20 | Control-char sweep: `\u00xx` only below U+0020, raw U+007F | `{"a":"\u0000\u0001\u001f\u007f"}` → `\u0000\u0001\u001f` escapes + raw DEL |

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
- The simplified subset described in `schemas/canonicalization.md` §1
  (sufficient for ICP-1.0 payload shapes), or
- A full RFC 8785 JCS implementation (e.g. the Rust `serde_jcs` crate).

Both produce identical output on the sub-cases below.
