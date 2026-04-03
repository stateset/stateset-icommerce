# Outcomes Model

Status: internal scenario-planning memo
Last reviewed: 2026-04-02

This document is a strategic outcomes model for StateSet.

It is not a forecast.
It is not fundraising advice.
It is not a claim about current fair market value.

It is a way to think about how market creation, standard capture, and revenue design combine into enterprise value.

## Core Thesis

StateSet outcomes are driven far more by market reality and protocol position than by code quality alone.

The key question is not:

"Is the code good?"

The key questions are:

- Does agent commerce become real?
- Does StateSet become part of the default control plane for it?
- Does StateSet capture enough of the economics to matter?

That creates an unusually wide outcome range:

- downside: small acquisition or failure to find a durable market
- upside: category-defining infrastructure if agent commerce becomes large and StateSet becomes standard

## Value Equation

The simple model is:

`enterprise value ~= agent-commerce GMV x StateSet share x revenue capture x revenue multiple`

That is still too simple, but it gives the right intuition.

### Variables

| Variable | Bear Case | Base Case | Bull Case |
|----------|-----------|-----------|-----------|
| Agent-commerce TAM | niche | meaningful infra market | very large commerce category |
| StateSet market share | low single digits | meaningful control-plane share | standard-like position |
| Revenue capture | thin software/service take | moderate infrastructure take | strong protocol/control-plane take |
| Revenue multiple | infrastructure discount | venture-scale infra multiple | standard/platform premium |

## Scenario Bands

### 1. Talent / IP acquisition

**Indicative outcome:** `$20M-$75M`

What it means:

- the company does not become a large standalone platform
- the team, protocol work, or cryptography stack is still valuable to an acquirer
- likely acquirer set includes payment, stablecoin, exchange, or infrastructure players

What would drive this outcome:

- strong technical differentiation
- weak distribution or weak market timing
- agent commerce remains early or fragmented

### 2. Venture-scale infrastructure company

**Indicative outcome:** `$150M-$500M`

What it means:

- StateSet becomes a real infrastructure company with enterprise revenue
- value comes from software, hosted control plane, or sequencer/compliance services
- this does not require StateSet to become the global standard

What would drive this outcome:

- real enterprise customers
- stable recurring revenue
- credible security, compliance, and support posture
- useful but not dominant ecosystem position

### 3. Protocol standard capture

**Indicative outcome:** `$2B-$8B`

What it means:

- StateSet becomes one of the default standards for verifiable agent commerce
- the market sees StateSet as infrastructure, not just a product
- value comes from standard capture, ecosystem gravity, and trusted control-plane status

What would drive this outcome:

- multiple external implementations of core protocols
- meaningful third-party adoption beyond first-party apps
- network effects in discovery, reputation, sequencing, or compliance
- clear monetization tied to the control plane

### 4. Category-defining infrastructure

**Indicative outcome:** `$15B-$50B+`

What it means:

- agent commerce becomes a very large market
- StateSet becomes default infrastructure for a meaningful portion of it
- the company is valued as foundational rails, not as a point solution

What would drive this outcome:

- agent commerce reaches substantial share of real commercial flows
- StateSet owns or co-owns a default protocol/control-plane layer
- distribution and trust keep pace with technical lead over many years

## Illustrative Math

These numbers are not predictions.
They are scenario illustrations.

### Bull-style illustration

```
Global commerce base:                 very large
Agent-intermediated share:            meaningful and rising
StateSet share of agent commerce:     meaningful but not dominant
Average revenue capture:              infrastructure-level take
Result:                               hundreds of millions of revenue
At a strong multiple:                 multi-billion valuation
```

### Base-style illustration

```
Agent commerce becomes real, but not dominant
StateSet wins a meaningful slice of the control plane
Revenue becomes durable but not category-defining
Result:                               tens of millions of revenue
At normal infra multiples:            hundreds of millions in value
```

### Bear-style illustration

```
Agent commerce stays niche
StateSet captures a small footprint
Revenue remains modest or services-heavy
Result:                               low revenue and limited strategic leverage
Outcome:                              small company, acqui-hire, or reset
```

## Variance Drivers

The variance is dominated by a small number of questions:

| Question | Impact on outcome |
|----------|-------------------|
| Does agent commerce actually happen at scale? | very high |
| Does StateSet capture a protocol or control-plane standard? | high |
| Is the business model economically correct? | medium |
| Does the team execute well enough to compound advantages? | medium, but downstream of market reality |

The market-existence question is still the biggest one.

## Milestone-Gated View

The best way to reason about outcomes is by milestone progression, not abstract TAM alone.

### Stage A: Technical credibility

Signals:

- `1.0` release
- published trust assumptions
- audits
- benchmark evidence
- early design partners

This moves StateSet from "interesting code" toward "credible infrastructure."

### Stage B: Market proof

Signals:

- 10 production design partners or equivalent
- repeat transactions across multiple agent-driven workflows
- evidence that agents are handling real commerce tasks, not just demos
- measurable retention and expansion

This answers whether agent commerce is real for an identifiable segment.

### Stage C: Ecosystem proof

Signals:

- third parties implement VES or adjacent protocol surfaces
- external agents or platforms integrate without StateSet owning the full stack
- reputation, discovery, or sequencing starts to show network effects

This is the transition from product to standard candidate.

### Stage D: Economic proof

Signals:

- clear take-rate or subscription logic
- low-friction pricing for embedded adoption
- higher-margin hosted or control-plane services
- expansion path from local runtime to managed infrastructure

This determines whether adoption can compound into large enterprise value.

## What Maximizes Outcome

### 1. Prove that agent commerce is real

The most valuable near-term signal is not another subsystem.
It is real transaction volume and repeated use by real agents.

The internal target should be concrete:

- get real agents transacting
- get repeated workflows, not one-off demos
- measure retention, failure modes, and policy outcomes

### 2. Own the standard layer

Feature count does not create large infrastructure value by itself.
Standard capture does.

That means:

- publish the protocol surfaces clearly
- make third-party implementation possible
- create conformance tests and reference implementations

### 3. Align upward where protocol gravity already exists

If payment-rail gravity belongs elsewhere, StateSet should sit above it rather than fight it directly.

The winning posture is:

- partner where possible on rails
- own the commerce control plane above the rails

### 4. Ship trust artifacts, not just features

For enterprise adoption, the next large unlocks are:

- `1.0`
- audits
- operational evidence
- stability guarantees
- accurate and disciplined docs

### 5. Make monetization follow adoption naturally

Likely durable monetization layers include:

- enterprise licensing
- hosted sequencing or compliance services
- control-plane features with high switching cost
- ecosystem services around standards and conformance

The business model should not require immediate protocol rent extraction to work.

## Practical Range

The honest practical range is still very wide.

If agent commerce remains niche, the likely outcomes cluster around:

- small acquisition
- modest infrastructure company
- strategic but limited exit

If agent commerce becomes a large category and StateSet captures standard-like position, the upside expands dramatically.

The key point is:

- upside is real
- variance is enormous
- most of the variance comes from market creation and standard capture, not from incremental code quality

## Operating Conclusion

The right internal conclusion is:

1. Treat market proof as the first priority.
2. Treat protocol standardization as the second priority.
3. Treat trust artifacts as the gating function for enterprise value.
4. Treat monetization as a design problem that should follow adoption, not choke it.

StateSet can become a large infrastructure company only if it answers two questions with evidence:

- autonomous agents really will run meaningful commerce
- StateSet is the default control plane they run it on

## Related Docs

- [Competitive Landscape](./COMPETITIVE_LANDSCAPE.md)
- [Agentic Commerce Baseline](./AGENTIC_COMMERCE_BASELINE.md)
- [Trust Foundation](../TRUST_FOUNDATION.md)
