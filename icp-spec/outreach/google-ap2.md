**To:** Google — AP2 (Agent Payments Protocol) team
**Subject:** AP2 IntentMandate as ICP `principal_binding` — composition note

Hi [name],

We've been studying AP2 closely. The IntentMandate primitive is exactly
the layer of authorization-by-cryptography that the agentic-commerce
stack has been missing.

We've built the operational lifecycle on the merchant side — quote,
escrow, fulfillment proof, dispute, settlement receipt — and just
published it as ICP-1.0-DRAFT (Intelligent Commerce Protocol). CC-BY-4.0
spec, Apache-2.0 schemas, royalty-free patent grant. Reference impl is
the StateSet Rust engine, 250k LOC, v1.0.4 shipping.

ICP and AP2 compose without overlap:

- **AP2** defines the *authorization* (mandate from principal to agent).
- **ICP** defines the *execution lifecycle* of that authorization on the
  merchant side.

In the spec, an AP2 IntentMandate becomes the `principal_binding` field
of an ICP Intent. The AP2 audit trail and the ICP escrow events are
complementary; together they give a regulator-grade end-to-end story.

**One ask:** could the AP2 team review the `principal_binding` schema in
ICP-1.0 (`schemas/intent.purchase.create.schema.json` →
`PrincipalBinding`) to confirm it's a clean superset of the AP2
IntentMandate fields? If it isn't, we'd rather adjust ICP than have two
incompatible authorization shapes in the wild.

If it composes cleanly, would Google want to be a named co-author on a
1-pager `ICP × AP2` showing the joint flow?

— Dom Steil
StateSet, Inc.
dom@stateset.com
