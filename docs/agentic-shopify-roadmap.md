# Agentic Shopify Roadmap Implementation

This document tracks concrete implementation status for the multi-phase roadmap to make
StateSet an agentic-first commerce control plane.

## Phase 1: Trust + Interop Foundation

- [x] Deterministic mutation simulation tool (`agentic_simulate_mutation`)
- [x] Deterministic mutation replay tool (`agentic_replay_mutation`)
- [x] Signed policy decision bundles (approval stages, rollback contract, audit artifact)
- [x] Replay event signatures and event-id filtering support
- [x] Shopify shadow adapter registered (`shopify-shadow`)
- [x] Shopify shadow import tool (`import_shopify_shadow_data`)
- [x] Shadow support for customers/products/inventory/orders/fulfillments

## Phase 2: Merchant Ops Parity

- [x] Provider-backed payment intent lifecycle tools (create/get/capture/cancel/refund) with Stripe adapter skeleton + deterministic runtime
- [x] Payment settlement + reconciliation primitives (batch settlement creation, payout webhook reconciliation, reconciliation reports)
- [x] Shipping provider runtime and tools (provider listing, rate quoting, label create/void/track) with carrier-hub skeleton
- [x] Tax provider abstraction and tools (provider listing, quote, commit, void) with idempotent deterministic runtime
- [x] Fulfillment exception orchestration tool (carrier failures, partial shipments, split tender, returns arbitration)
- [ ] Real carrier API wiring (UPS/FedEx/USPS/ShipStation production credentials and webhooks)
- [ ] Real Stripe settlement/reconciliation + webhook ingestion parity
- [x] Production tax provider failover path and jurisdictional compliance hardening (strict jurisdiction validation + routed fallback planning)
- [ ] Binding/CLI reliability hardening to raise coverage and operational confidence

## Phase 3: Agentic Moat

- [x] SLA-aware routing boosts in agent router + harness `slaLevel` support
- [x] SLA context propagation into `agentic_plan` / `agentic_execute_plan` with per-step routing metadata and replay annotation
- [ ] SLA-aware multi-agent routing planner integrated with plan execution
- [ ] Portable trust attestations and merchant-segment risk profiles
- [ ] Unified governed intent → payment → fulfillment protocol path

## Phase 4: Ecosystem + Network Effects

- [x] WASM connector ecosystem foundation (catalog publish/install/uninstall + installed connector execution runtime + MCP connector tools)
- [ ] Agent/App marketplace certification and safety scorecards
- [ ] ACP/UCP-compatible protocol bridge
- [ ] Managed control plane + embedded edge runtime hybrid mode
