**To:** OpenAI — Agents SDK / ChatGPT Commerce
**Subject:** Open commerce-lifecycle spec — would you review before freeze?

Hi [name],

OpenAI's Agents SDK + ChatGPT checkout is the most plausible path to
mainstream agent commerce. We've been building the merchant-side
infrastructure for that world: `stateset-icommerce`, 250k-LOC Rust engine,
v1.0.4 shipping today; reference implementation of the Agentic Commerce
Protocol via `stateset-acp-handler`.

In doing the implementation work we identified an operational seam
between ACP's `checkout.session` and final settlement that lacked a
common protocol — quote-binding, escrow lifecycle, fulfillment proof,
dispute, settlement receipt. We wrote it up as an open spec, ICP-1.0
(Intelligent Commerce Protocol). CC-BY-4.0 + Apache-2.0, royalty-free
patent grant. Composes with ACP rather than competing.

Spec: `github.com/stateset/icp-spec`
RFC: `PROTOCOL-RFC.md`

**One ask:** could the Agents SDK / ChatGPT Commerce team review the
"With ACP" section before we freeze ICP-1.0 (target Q4 2026)? If there
are things on your roadmap that obviate ICP, we'd much rather know now
than have a near-duplicate in the wild.

If the composition story holds, we'd want OpenAI/Stripe quoted on the
joint composition doc.

— Dom Steil
StateSet, Inc.
dom@stateset.com
