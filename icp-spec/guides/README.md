# ICP Operator Guides

Practical walkthroughs for the three operator roles in ICP. The
normative specs live alongside in [`../`](../); these are the
"I have a thing, how do I plug it into ICP" docs.

| Guide | For | Length |
|---|---|---|
| [`merchant-integration.md`](./merchant-integration.md) | You sell things and want to accept ICP-signed Intents | ~15 min |
| [`settler-implementation.md`](./settler-implementation.md) | You custody value and want to be a named Settler | ~20 min |
| [`icpip-0005-quickstart.md`](./icpip-0005-quickstart.md) | You want webhook delivery instead of polling | ~5 min |

## Reading order

If you're new to ICP, read in this order:

1. [`../PACKET.md`](../PACKET.md) — 8-minute decision-grade summary.
2. The guide for your role above.
3. [`../ICP-1.0-DRAFT.md`](../ICP-1.0-DRAFT.md) — the normative spec.
4. The runnable demo:
   [`../examples/02-end-to-end-flow/`](../examples/02-end-to-end-flow/).

## Where to ask

File an issue against
[`stateset/stateset-icommerce`](https://github.com/stateset/stateset-icommerce)
with label `icp-spec` (spec questions) or `icp-conformance` (conformance
failures).
