**To:** Stripe — ACP team / @ patrickc, @ brockm if introduceable
**Subject:** ACP-conformant escrow + dispute layer — open spec, RI shipping

Hi [name],

We ship `stateset-acp-handler`, a reference implementation of the Agentic
Commerce Protocol for merchants. Behind it is `stateset-icommerce`, a
250k-LOC Rust commerce engine with 15.8k tests (v1.0.4 on crates.io /
npm / pypi today).

Working on real merchant flows over the last few months made one gap
sharp: ACP's `checkout.session` is great for the moment-of-purchase, but
agentic commerce has many cases where the merchant needs a *signed
escrow + fulfillment + dispute* lifecycle on top — not because ACP is
missing anything, but because that lifecycle has always lived in
merchant-side software and now needs to be agent-readable too.

We wrote an open protocol for the seam — ICP-1.0 (Intelligent Commerce
Protocol). CC-BY-4.0 spec, Apache-2.0 schemas, royalty-free patent grant.
It explicitly composes with ACP: an ACP merchant becomes ICP-conformant
by adding **one HTTP endpoint** (`POST /icp/v1/escrow/events`) and **one
signing key**. Existing ACP integrations are not invalidated.

The composition section of the RFC is here:
[PROTOCOL-RFC.md §"With ACP"]

**One ask:** could someone on the ACP team spend 30 minutes with me on
the composition design before we freeze ICP-1.0? Specifically I want to
make sure the EscrowEvent payload doesn't shadow or conflict with anything
on ACP's roadmap. If the answer is "we have something coming that does
this," we'd much rather defer to it than ship a near-duplicate.

If ACP and ICP can be shown to compose cleanly in a public 1-pager, we'd
like Stripe to be the named co-author.

Happy to come to SF if a face-to-face is faster.

— Dom Steil
StateSet, Inc.
dom@stateset.com
