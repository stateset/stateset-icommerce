# Summary

# Getting Started

- [Overview](index.md)
- [Getting Started](getting-started.md)
- [Standalone Quickstart](standalone-quickstart.md)
- [AI Agent Quickstart](ai-agents.md)
- [Product Tiers](tiers.md)
- [Trust Foundation](trust-foundation.md)

# Concepts & Architecture

- [What is iCommerce?](concepts/icommerce.md)
- [Design Principles](concepts/design-principles.md)
- [Architecture](architecture.md)
- [Dependency Direction](guides/dependency-direction.md)
- [The Agentic Reasoning Loop](concepts/reasoning-loop.md)
- [Case Studies](concepts/case-studies.md)
- [Architecture Decisions](adr/README.md)
  - [ADR-0001: Layered Architecture](adr/0001-layered-architecture.md)
  - [ADR-0002: Embedded SQLite as the Default Backend](adr/0002-embedded-sqlite-default.md)
  - [ADR-0003: Event-Driven Extensions and Auditability](adr/0003-event-driven-extensions.md)
  - [ADR-0004: CLI Safety Model (`--apply`)](adr/0004-cli-safety-model.md)
  - [ADR-0005: Binding Generation From a Single Spec](adr/0005-binding-generation.md)

# Core Commerce

- [Domain Model](commerce/domain-model.md)
- [Orders & Fulfillment](commerce/orders.md)
- [Inventory & Warehousing](commerce/inventory.md)
- [Products & Catalog](commerce/products.md)
- [Customers & Segments](commerce/customers.md)
- [Payments & Refunds](commerce/payments.md)
- [Returns & RMA](commerce/returns.md)
- [Subscriptions & Billing](commerce/subscriptions.md)
- [Carts & Checkout](commerce/carts.md)
- [Shipping & Fulfillment](commerce/shipping.md)
- [Tax & Promotions](commerce/tax-promotions.md)
- [Manufacturing & Supply Chain](commerce/manufacturing.md)
- [Accounting & Finance](commerce/accounting.md)
- [Analytics & Forecasting](commerce/analytics.md)
- [Customer Engagement](commerce/engagement.md)
- [Fraud Detection](commerce/fraud.md)
- [B2B Operations](commerce/b2b-operations.md)

# Agent-to-Agent Commerce

- [A2A Protocol Overview](a2a/overview.md)
- [Quotes & Negotiation](a2a/quotes.md)
- [Escrow & Conditional Payments](a2a/escrow.md)
- [Split Payments](a2a/splits.md)
- [Subscriptions (A2A)](a2a/subscriptions.md)
- [Reputation & Trust](a2a/reputation.md)
- [Event Streaming](a2a/event-streaming.md)
- [Disputes & Resolution](a2a/disputes.md)
- [Marketplace & Discovery](a2a/marketplace.md)
- [Infrastructure](a2a/infrastructure.md)
- [Handshake Protocol](a2a/handshake.md)
- [Saga Orchestration](a2a/sagas.md)
- [Agent Memory & Learning](a2a/agent-memory.md)
- [Cost Analytics & Forecasting](a2a/cost-analytics.md)
- [Rules Engine](a2a/rules-engine.md)
- [Advanced: Strategies, Workflows & Messaging](a2a/advanced.md)

# The StateSet Trilogy

- [Trilogy Overview](trilogy/overview.md)
- [Sequencer](trilogy/sequencer.md)
- [SET Chain L2](trilogy/set-chain.md)
- [STARK Compliance Proofs](trilogy/stark-proofs.md)
- [ssUSD Stablecoin](trilogy/ssusd.md)
- [Anchor Service](trilogy/anchor.md)

# Payments

- [x402 Payment Protocol](payments/x402.md)
- [Base USDC Quickstart](payments/base-usdc.md)
- [Budget Governance](payments/budget.md)
- [Stablecoins & Settlement](payments/stablecoins.md)

# Cryptography & Security

- [VES v1.0 Specification](security/ves.md)
- [Security Architecture](security/architecture.md)
- [ERC-8004 Agent Identity](security/erc8004-identity.md)

# Policy & Safety

- [Policy Engine](policy/engine.md)
- [Permissions & Auth](guides/permissions.md)

# Platform Adapters

- [Adapter Overview](adapters/overview.md)
- [Stripe](adapters/stripe.md)
- [WooCommerce](adapters/woocommerce.md)
- [Shopify](adapters/shopify.md)

# Operations & Guides

- [CLI](guides/cli.md)
- [MCP Tools](guides/mcp-tools.md)
- [Sync (VES)](guides/sync.md)
- [Operations](guides/operations.md)
- [Heartbeat Monitor](guides/heartbeat.md)
- [Observability & Telemetry](guides/observability.md)
- [Multi-Agent System](guides/multi-agent.md)
- [Autonomous Engine](guides/autonomous-engine.md)
- [Semantic Search](guides/semantic-search.md)
- [Data Migration & Import](guides/data-migration.md)
- [Embedded Agent Toolkit](guides/agent-toolkit.md)

# Performance & Advanced

- [Async vs Sync](guides/async-vs-sync.md)
- [Performance Tuning](guides/performance.md)
- [Logging & Debugging](guides/logging.md)
- [Testing Strategy](advanced/testing.md)
- [Database Schema](advanced/database-schema.md)
- [Compliance & Audit](advanced/compliance.md)
- [Admin Dashboard](advanced/admin-dashboard.md)
- [Deployment](advanced/deployment.md)
- [WASM Connectors](advanced/wasm-connectors.md)

# API Reference

- [Overview](api/index.md)
  - [Rust](api/rust.md)
  - [Node.js](api/node.md)
  - [Python](api/python.md)
  - [Ruby](api/ruby.md)
  - [PHP](api/php.md)
  - [Java](api/java.md)
  - [Kotlin](api/kotlin.md)
  - [Swift](api/swift.md)
  - [C# / .NET](api/dotnet.md)
  - [Go](api/go.md)
  - [WASM](api/wasm.md)

# Appendix

- [Examples](examples.md)
- [Agent Inventory](appendix/agent-inventory.md)
- [MCP Tool Inventory](appendix/mcp-tool-inventory.md)
- [Workspace Inventory](appendix/workspace-inventory.md)
- [Troubleshooting](appendix/troubleshooting.md)
- [Versioning](versioning.md)
