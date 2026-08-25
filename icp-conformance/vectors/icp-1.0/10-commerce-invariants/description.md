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
| `gl01`–`gl04` | Every posted journal entry balances; every line is single-sided | `commerce.ledger.entry_unbalanced`, `commerce.ledger.line_not_single_sided` |
| `scale01`–`scale06` | No amount carries more decimals than its currency allows | `commerce.money.scale_exceeds_currency` |

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
