**To:** Anthropic — MCP / Agent Skills team
**Subject:** Largest commerce MCP server — and a protocol question

Hi [name],

I run StateSet, where we've built what is (to our knowledge) the largest
domain-specific MCP server on the public internet: 700+ commerce tools
across 63 domain modules — orders, inventory, returns, disputes, escrow,
agent-to-agent settlement, the lot. It backs `stateset-icommerce`,
a 250k-LOC Rust engine with 15.8k tests, v1.0.4 shipping today on
crates.io / npm / pypi / gem.

A pattern we keep hitting: MCP gives an agent the tool surface, but the
operational lifecycle of a multi-step commerce transaction (quote, escrow,
fulfillment proof, dispute, settlement receipt) is currently
implementation-defined. Every commerce backend exposes its own verbs.
Agent operators have to learn 10 different escrow models.

We just published ICP-1.0-DRAFT, an open protocol that standardizes the
multi-step commerce lifecycle. CC-BY-4.0 spec, Apache-2.0 schemas,
royalty-free patent grant. It explicitly defines an MCP binding so any
ICP-conformant backend exposes the same six verbs over MCP:
`icp_intent_create`, `icp_quote_review`, `icp_quote_sign`,
`icp_escrow_observe`, `icp_dispute_open`, `icp_settlement_verify`.

**One ask:** would the MCP team be willing to review the MCP-binding
section of the spec (~2 pages) before we freeze it for ICP-1.0 Final?
If you think it's reasonable we'd love to be a documented reference for
"MCP for multi-step commerce" — and if you think it's wrong, we'd much
rather know before ratification than after.

Spec link, draft, and a working `icp_intent_create` MCP tool example
inline if you'd like to see them.

Thanks for shipping MCP. It is the best protocol decision in agentic AI
since OpenAI shipped function-calling.

— Dom Steil
StateSet, Inc.
dom@stateset.com
