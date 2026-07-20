# Vector 05 — Intent Validation

**Spec sections covered:** ICP-1.0 §6 (intent verbs and their normative JSON
Schemas, `schemas/intent.*.schema.json`), `schemas/error-codes.md`
(`format.*`, `version.*` namespaces).

The second operational-semantics family (after `04-escrow-lifecycle`). Before
any two agents can transact, they must agree on what a *well-formed intent
even is* — an implementation that accepts an intent its counterparty rejects
(or rejects one the counterparty signed in good faith) breaks the handshake.
This family pins that agreement across all seven verbs.

## Scope

**Structural validation only.** Absolute-time checks (`replay.expired`,
`replay.iat_in_future`) and window-timing (`replay.window_too_long`) require
an injected clock and are deferred to a dedicated timing family (roadmap item
1.3). Deep `principal_binding` signature verification is covered by vector
`03-signature-verification`. This family validates the intent envelope's
structure and field formats — the checks every verifier runs on every
received intent regardless of clock or crypto.

## The seven verbs

`purchase.create`, `inventory.query`, `quote.request`, `payout.request`,
`subscription.create`, `subscription.cancel`, `purchase.return`. Each has a
valid fixture (`v01`–`v07`). Note `payout.request` uses `seller`/`platform`
where the others use `buyer`/`merchant`.

## Normative validation precedence

The IUT applies these checks in order; the **first** failure determines the
result. Every negative case violates exactly one rule, but the order is
normative so implementations agree on intents with multiple defects.

1. `v` field absent → `format.missing_field`
2. `v` ≠ `"icp-1.0"` → `version.unsupported`
3. `verb` field absent → `format.missing_field`
4. `verb` not one of the seven → `format.unknown_verb`
5. any other required field for this verb absent → `format.missing_field`
6. any AID-typed field (`buyer`/`merchant` or `seller`/`platform`) fails the
   AID pattern `^aid:v1:z[1-9A-HJ-NP-Za-km-z]{40,60}$` → `format.bad_aid`
7. `settler` fails `^settler:[a-z0-9]+(\.[a-z0-9]+)*$` → `format.bad_settler_id`
8. any top-level Money field's `amount` fails
   `^-?[0-9]+(\.[0-9]{1,18})?$` → `format.bad_money`
9. a verb that requires `items` has an absent or empty `items` array →
   `format.bad_schema`
10. otherwise → valid

## Adapter contract

stdin: `inputs.json` (`{ "cases": [{ "id", "intent" }, ...] }`). stdout:

```json
{ "validations": { "v01_inventory_query_valid": {"valid": true},
                   "n04_unknown_verb": {"error": "format.unknown_verb"}, ... } }
```

Expected outputs (`expected.json`) are generated mechanically from the
schemas — see `_provenance`.
