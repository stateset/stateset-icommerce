**To:** Shopify — Commerce Platform / Agent Commerce lead
**Subject:** ICP-conformant agent layer for Shopify merchants

Hi [name],

I run StateSet. We've shipped a 250k-LOC Rust commerce engine
(`stateset-icommerce`, v1.0.4) and just published an open protocol —
ICP-1.0 — for the multi-step agent-commerce lifecycle (quote, escrow,
fulfillment, dispute, settlement). CC-BY-4.0 spec, Apache-2.0 schemas.

The pitch for Shopify specifically:

- A Shopify merchant becomes ICP-conformant via an app — no platform
  changes needed.
- The app exposes Shopify orders, inventory, returns to ICP-speaking
  agents (their own and external) with cryptographic provenance on every
  state transition.
- The merchant gets a signed audit trail of every agent action, which
  closes the "who-told-the-agent-to-do-that" loop that's been blocking
  agent commerce on Shopify.
- Stripe payments stay where they are; ICP just adds the lifecycle layer
  on top.

We can ship the first version of the app in 4–6 weeks. We'd love
Shopify's input on:

1. The right App Store category for an agentic-commerce primitive that
   isn't a sales channel and isn't a payment provider.
2. Whether Shopify's roadmap has anything in this space we should be
   composing with rather than building parallel to.

**One ask:** 30 minutes with whoever owns "agentic commerce" inside
Shopify. If that's not a person yet, who's the closest?

— Dom Steil
StateSet, Inc.
dom@stateset.com
