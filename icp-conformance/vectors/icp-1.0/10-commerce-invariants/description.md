# Vector 10 — Commerce Invariants

**Profile:** `icp-1.0-commerce` (layered on `icp-1.0-core`)
**Companion documentation:** `docs/src/advanced/invariants.md`

Vectors 01–09 establish that two implementations agree about *identity,
canonicalisation, signatures, and protocol state*. They do not establish that
either one keeps correct books.

This vector covers the other half: the economic invariants an implementation
MUST uphold before it can claim to execute commerce. An implementation that
passes `icp-1.0-core` and fails this vector is cryptographically interoperable
and financially unsafe.

Every case is a pure decision — given a state and a request, does the
implementation allow it or reject it with the stated error code? No case
depends on wall-clock time, key material, or storage engine.

## Families

| Cases | Invariant | Error code on violation |
|---|---|---|
| `ref01`–`ref07` | Σ refunds ≤ amount captured, **counting in-flight refunds** | `commerce.refund.exceeds_captured` |
| `cap01`–`cap04` | Σ captures ≤ order total, **counting in-flight captures** | `commerce.capture.exceeds_order_total` |
| `retq01`–`retq06` | Returned quantity ≤ shipped quantity, and only after shipment | `commerce.return.exceeds_shipped`, `commerce.return.order_not_shipped` |
| `inv01`–`inv04` | A reservation never exceeds `on_hand − allocated` | `commerce.inventory.insufficient_available` |
| `gl01`–`gl06` | Every posted journal entry balances; every line is single-sided | `commerce.ledger.entry_unbalanced`, `commerce.ledger.line_not_single_sided` |
| `scale01`–`scale10` | No amount carries more *significant* decimals than its currency allows | `commerce.money.scale_exceeds_currency` |

## Why in-flight amounts are in the vector

`ref04`/`ref05` and `cap04` are the load-bearing cases. An implementation that
compares only *completed* refunds against the captured amount passes every
single-threaded test and still lets two concurrent refunds both succeed — the
classic double-refund race. The reference implementation was deliberately
broken this way while developing this vector, and the vector caught it.

The same applies to captures: a payment authorised for $1,500 must not accept
two concurrent $1,000 captures.

## Exact decimal arithmetic

`ref06` (`100.0` vs `100.000`) and `scale06` (six-decimal USDC) exist to catch
implementations that parse money into binary floating point. Amounts are
compared as exact decimals; trailing zeros are numerically insignificant but
must not change a verdict.

`scale04`/`scale05` use JPY, which has **zero** minor units — an implementation
that hard-codes two decimal places fails.

## Claiming conformance

Add `icp-1.0-commerce` to your entry's `supports` array in
`iut-adapters/registry.json` and run:

```bash
node runner/run.mjs --profile icp-1.0-commerce --iut <your-iut>
```

## Normative details

These were left implicit in the first draft of this vector. Four independent
implementations (JS, Python, Go, Rust) were written against it; they diverged on
the trailing-zero rule below, which is exactly the class of latent interop bug
this suite exists to prevent. Everything here is now binding.

### Response shape

An adapter reads `inputs.json` on stdin, receives the vector name as `argv[1]`,
and writes to stdout:

```json
{ "decisions": { "<case id>": { "valid": true } | { "error": "<code>" } } }
```

One entry per case in `cases`, keyed by `id`. Exactly one of `valid` or `error`.

### Currency minor units

| Currency | Minor units |
|---|---|
| `USD`, `EUR` | 2 |
| `JPY` | 0 |
| `USDC` | 6 |

Only these appear in this vector. An implementation MAY support more; behaviour
for an unlisted currency is not constrained by this vector.

### Significant scale — trailing zeros

The scale check counts decimal places **after trimming insignificant trailing
zeros**. `10.9900` is two-scale and valid USD; `10.9901` is four-scale and is
not. `1000.0` is zero-scale and valid JPY.

This follows from the rule that trailing zeros must not change a verdict: the
invariant bounds *precision*, not the number of characters after the point.
`scale07`–`scale10` pin both directions.

### Error precedence within a journal entry

The single-sided check runs **first**, per line. An entry that is both
unbalanced and contains a two-sided line reports
`commerce.ledger.line_not_single_sided` (`gl05`). A line with zero on both
sides is legal — zero is neither a debit nor a credit (`gl06`).

### Returns against an unshipped order

When `shipped` is `0`, both "not shipped" and "exceeds shipped" are literally
true. `commerce.return.order_not_shipped` takes precedence: it is the more
specific diagnosis, and the one an agent can act on.

### The `ordered` field

`return_quantity` cases carry `ordered` for context. No invariant here uses it —
`shipped` can never exceed `ordered`, so bounding a return by `shipped` already
bounds it by `ordered`. It is informational.

### Quantity encoding

Inventory quantities (`inv0x`) are decimal **strings** because stock can be
fractional for weight- or volume-priced goods. Return quantities (`retq0x`) are
JSON **numbers** because return lines are whole units. Implementations should
route both through exact-decimal comparison rather than binary floating point.
