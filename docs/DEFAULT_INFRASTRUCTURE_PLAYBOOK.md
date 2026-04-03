# Default Infrastructure Playbook

Status: internal go-to-market and ecosystem memo
Last reviewed: 2026-04-02

This document is the distribution playbook for making StateSet the default infrastructure layer for AI-agent commerce.

It is not a product brochure.
It is an execution document.

## Core Goal

Be to AI agents what SQLite is to mobile and local apps:

- embedded
- obvious
- reliable
- free to start
- bundled into the places developers already are

The right question is not:

"How do we prove the architecture is elegant?"

The right question is:

"How do we become the default answer when a developer asks how an agent should do commerce?"

## Why SQLite Is The Right Analogy

SQLite won because it had five properties at once:

- no setup
- embedded by default
- widely bundled
- extremely reliable
- easy to adopt before anyone needed to justify the choice

StateSet needs the same pattern in agent commerce:

- no infrastructure tax to start
- embedded runtime first
- default integrations into agent frameworks
- boring reliability
- early bundling into the platforms that shape developer behavior

## Distribution Thesis

Technical excellence is necessary.
It is not the deciding variable.

The deciding variables are:

- distribution
- developer experience
- standard capture
- visible network effects

StateSet wins only if it becomes easy to adopt before larger platforms ship a good-enough substitute.

## The Five Vectors

### 1. Bundling Into Agent Platforms

The fastest route to ubiquity is inclusion in platforms with existing distribution.

Priority targets:

| Platform | Integration path | Why it matters |
|----------|------------------|----------------|
| Anthropic / Claude ecosystem | official MCP commerce toolkit and reference workflows | strongest immediate fit with current architecture |
| OpenAI ecosystem | function-calling and Responses API toolkit | large developer mindshare and application reach |
| Vercel AI SDK | first-class server-side agent commerce primitives | strong app-builder distribution |
| LangChain / LangGraph | templates, tools, and reference agents | default choice for many agent builders |
| CrewAI and similar orchestration stacks | prebuilt commerce crews and workflows | increases agent-team adoption |
| Hugging Face agent ecosystem | templates and examples | developer awareness and educational reach |

Operating rule:

- every serious agent framework should have an obvious StateSet integration path
- the install story should look trivial
- docs and templates must feel official, not community afterthoughts

### 2. Magical First Five Minutes

StateSet should target:

- under 60 seconds from install to first useful transaction
- no mandatory control-plane setup for the first local success
- one-command scaffolding that yields a runnable demo

The standard to copy is not "good enterprise onboarding."
It is the emotional effect of `create-react-app`, `rails new`, or early Stripe CLI success.

Target experience:

```bash
npx create-commerce-agent my-agent
cd my-agent
npm start
```

The scaffold should include:

- demo catalog and inventory
- default policies
- a visible agent workflow
- sensible local persistence
- attractive terminal output

If the 60-second quickstart cannot be recorded as a compelling demo video, the onboarding still is not good enough.

### 3. Canonical Agent Templates

Most developers do not want primitives.
They want a working reference they can fork.

The first template set should cover the obvious commerce jobs:

| Template | Purpose | Priority |
|----------|---------|----------|
| `storefront-agent` | product Q&A and order creation | highest |
| `fulfillment-agent` | shipment updates and status flows | highest |
| `returns-agent` | returns, refunds, and policy checks | highest |
| `inventory-agent` | stock adjustments and reordering workflows | high |
| `marketplace-agent` | vendor coordination and multi-party flows | high |
| `procurement-agent` | B2B sourcing and purchase-order flows | high |
| `customer-service-agent` | support orchestration across commerce tools | medium |
| `a2a-supplier-agent` | agent-to-agent wholesale and settlements | medium |

Every official template must:

- run out of the box
- expose a real workflow, not just a stub
- explain customization paths clearly
- be deployable with minimal edits

### 4. Standards Before The Market Hardens

If StateSet wants protocol leverage, VES and adjacent surfaces must become the default vocabulary early.

That means:

- standalone specs
- external co-authors or contributors
- independent implementations
- conformance tests
- a visible working group or standards forum

The goal is not merely to publish specs.
The goal is for developers to assume those specs are the starting point for agent commerce interoperability.

Priority surfaces:

- VES
- agent card schema
- A2A interoperability profile
- receipt and conformance artifacts

### 5. Trojan Horse Distribution

Developers often adopt platforms indirectly through useful tools.

StateSet should ship tools that people want immediately, even if they are not explicitly buying "agent commerce infrastructure."

Strong candidates:

| Tool | User-facing value | Strategic effect |
|------|-------------------|------------------|
| `agent-checkout` | gives agents a drop-in commerce action | normalizes StateSet runtime underneath |
| `agent-simulator` | tests and replays commerce flows | makes StateSet the default testing substrate |
| `commerce-debugger` | explains failed transactions and policy denials | makes StateSet artifacts the debug standard |
| `agent-analytics` | measures agent commerce outcomes | ties observability to StateSet semantics |
| `policy-sandbox` | simulates agent actions safely | makes policy-first execution feel native |

These tools should create adoption paths even for teams that are not yet ready to commit to the full stack.

## The Real Flywheel

The flywheel is:

1. More developers embed StateSet.
2. More visible agents use StateSet semantics.
3. More agent-to-agent interactions become naturally compatible.
4. More templates, tooling, and third-party references appear.
5. StateSet becomes the default choice for the next developer.

The critical mass point is not a philosophical milestone.
It is a visibility milestone.

Internal planning target:

- `1,000` publicly visible agents or workloads is the first meaningful network-effect threshold

This is not a law of nature.
It is a practical marker for when ecosystem gravity may begin to matter more than pure feature comparison.

## 90-Day Sprint

This is the highest-leverage 90-day sequence if the goal is default status rather than pure feature expansion.

| Window | Focus | Deliverable |
|--------|-------|-------------|
| Weeks 1-2 | onboarding polish | 60-second quickstart that can be recorded cleanly |
| Weeks 3-4 | official templates | three production-grade starter agents |
| Weeks 5-6 | framework integrations | LangChain and one additional orchestration stack shipped |
| Weeks 7-8 | standards surface | standalone VES and agent-card docs ready for external review |
| Weeks 9-10 | platform integrations | OpenAI and Vercel AI SDK reference integrations |
| Weeks 11-12 | visibility push | launch package, tutorials, demo videos, public examples |

## Success Metrics

Use simple leading indicators.

Near-term distribution indicators:

- successful quickstart completion rate
- weekly package downloads
- template forks or clones
- number of external examples built on StateSet
- integration usage by framework

Visibility indicators:

- public demo agents
- documentation traffic to quickstart and templates
- GitHub stars and contributor growth
- tutorial and reference-example adoption

Ecosystem indicators:

- third-party implementations of protocol surfaces
- external adapters or plugins
- public mentions in framework docs or marketplaces

Initial 90-day target band:

- meaningful lift in package installs
- visible template adoption
- at least two serious framework integrations
- at least one external team using the protocol surfaces publicly

## Hard Truths

The technically superior product often loses.

StateSet should assume the following:

- larger platforms can ship a weaker version faster once they care
- developer mindshare compounds faster than architectural purity
- default status is won through convenience and visibility before it is defended by depth

That means:

- do not hide behind technical sophistication
- do not confuse protocol elegance with adoption
- do not let onboarding friction waste the embedded advantage

## Open Source And Governance

If StateSet wants standard status, governance matters.

The strongest long-term standards are difficult for one company to monopolize.

That does not mean giving everything away immediately.
It does mean planning for legitimacy beyond a single vendor.

Possible directions:

- foundation-hosted protocol stewardship
- independent working group for VES and A2A surfaces
- published conformance program not controlled solely by one commercial offering

The test is simple:

- if adoption grows, will outside companies feel safe building on the protocol layer?

If the answer is no, standard capture gets harder.

## Non-Negotiable Constraints

Distribution only compounds if trust keeps pace.

The following remain non-negotiable:

- do not over-claim shipped capabilities
- do not hard-code misleading metrics in public docs
- do not turn protocol marketing into trust debt
- do not ship ecosystem promises without a real adoption path

The embedded and verifiable story is strong enough.
It does not need exaggeration.

## Operating Conclusion

The practical objective for the next phase is:

- make StateSet trivial to adopt
- make StateSet visible in every important agent framework
- make StateSet easy to copy through templates
- make StateSet harder to replace by standardizing early

The winning question is not:

"Can StateSet become technically complete?"

The winning question is:

"Can StateSet become the default before a bigger player becomes the default with a worse product?"

## Related Docs

- [Competitive Landscape](./COMPETITIVE_LANDSCAPE.md)
- [Outcomes Model](./OUTCOMES_MODEL.md)
- [Agentic Commerce Baseline](./AGENTIC_COMMERCE_BASELINE.md)
- [Trust Foundation](../TRUST_FOUNDATION.md)
