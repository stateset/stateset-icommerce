**To:** x402 maintainer (find on GH commits)
**Cc:** —
**Subject:** Joint x402 + escrow flow — would you co-author?

Hi [name],

We've been integrating x402 in `stateset-icommerce` for the last few months
and built an end-to-end agent-commerce engine on top of it. One thing we
keep running into: x402 covers the *payment trigger* beautifully, but when
the goods aren't instantly delivered (which is most B2B and most physical
commerce), we end up reinventing escrow + dispute + settlement-receipt
primitives on top.

Rather than fork a snowflake, we wrote it up as an open spec — ICP-1.0,
the Intelligent Commerce Protocol. It explicitly composes with x402 by
naming x402 as a `Settler` rail. Wire format is canonical CBOR + Ed25519
(+ optional ML-DSA-65). Reference impl is the StateSet Rust engine
(250k LOC, 15.8k tests, v1.0.4 shipping).

Spec is here: `github.com/stateset/icp-spec` (forthcoming, draft attached).
The relevant cross-protocol section is in `PROTOCOL-RFC.md` §"With x402".

**One ask:** would you (or someone at Coinbase you'd point us to) co-author
a 1-pager — `ICP × x402` — showing the deposit → escrow → release flow
on Base, signed end-to-end with x402 as the Settler? We'll do the
writing; we just need a reviewer who can speak for x402's design intent
and (ideally) lend a name to the published doc.

If yes, happy to send a draft within a week. If not, who at Coinbase
should I be talking to instead?

Thanks for shipping x402. It is the cleanest agent-payment primitive in
the wild.

— Dom Steil
StateSet, Inc.
dom@stateset.com
