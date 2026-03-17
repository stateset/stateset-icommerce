# Product Tiers

StateSet iCommerce is available in three tiers. Start with Tier 1 for free, upgrade when you need it.

## Tier 1: iCommerce Standalone (Free, Open Source)

**The SQLite of Commerce** — everything runs locally, no external services required.

| Feature | Included |
|---------|----------|
| SQLite commerce engine | Yes |
| 520+ MCP tools (orders, inventory, payments, etc.) | Yes |
| Policy engine (YAML rules, explainable denials) | Yes |
| Shopify adapter (CSV import + webhooks) | Yes |
| Stripe adapter (webhooks) | Yes |
| WooCommerce adapter (API import + webhooks) | Yes |
| Standalone webhook server (`stateset-webhooks`) | Yes |
| Multi-agent CLI (18 specialized agents) | Yes |
| Analytics & forecasting | Yes |
| A2A agent commerce (agent cards, x402) | Yes |

**Install:**
```bash
npm install -g @stateset/cli
stateset-init --quickstart
```

**Best for:** Solo developers, small teams, MVPs, hackathons, AI agent commerce experiments.

## Tier 2: iCommerce + Sequencer (Enterprise)

**The Git of Commerce Data** — adds cryptographic sync, multi-agent coordination, and audit trails via the [StateSet Sequencer](trilogy/sequencer.md).

Everything in Tier 1, plus:

| Feature | Included |
|---------|----------|
| [VES (Verifiable Event Sync)](security/ves.md) | Yes |
| Multi-agent state coordination | Yes |
| Cryptographic audit trail (Ed25519 signed events) | Yes |
| [Gap-free sequencing](trilogy/sequencer.md) with signed receipts | Yes |
| Merkle-tree commitments | Yes |
| Outbox pattern for reliable delivery | Yes |
| Event replay & time-travel debugging | Yes |
| [Agent key management](trilogy/sequencer.md) (Ed25519 rotation/revocation) | Yes |
| x402 payment intent processing | Yes |
| Multi-tenant support | Yes |
| Schema validation (disabled/warn/strict) | Yes |
| gRPC streaming for high-throughput ingestion | Yes |

**The Sequencer provides two finality levels:**
- **Soft finality** (milliseconds) — sequencer receipt confirms acceptance
- **Hard finality** (minutes) — batch anchored on-chain (requires Tier 3)

**Configuration:**
```bash
# Add sync.json to enable Sequencer
stateset-sync init --sequencer-url https://sequencer.stateset.com

# Register agent signing keys
stateset-sync keys:generate
stateset-sync keys:register
```

**Best for:** Enterprise teams, regulated industries (fintech, healthcare, supply chain), multi-agent orchestration.

## Tier 3: Full Trilogy — iCommerce + Sequencer + SET Chain (Settlement)

**The Settlement Layer of AI Economy** — adds on-chain anchoring, native stablecoins, zero-knowledge compliance, and autonomous agent payments via [SET Chain L2](trilogy/set-chain.md).

Everything in Tiers 1 & 2, plus:

| Feature | Included |
|---------|----------|
| [SET Chain L2](trilogy/set-chain.md) (OP Stack, 2s blocks, Chain ID 84532001) | Yes |
| [ssUSD yield-bearing stablecoin](trilogy/ssusd.md) (~5% APY, T-Bill backed) | Yes |
| On-chain state anchoring ([SetRegistry](trilogy/set-chain.md)) | Yes |
| [STARK compliance proofs](trilogy/stark-proofs.md) (zero-knowledge) | Yes |
| [SetPaymaster](trilogy/set-chain.md) (gasless UX, ERC-4337) | Yes |
| [Anchor service](trilogy/anchor.md) (sequencer → chain bridge) | Yes |
| Cross-chain bridges (Base, Arbitrum, Ethereum) | Yes |
| Yield-bearing escrow (funds earn while held) | Yes |
| Hard finality (independently verifiable on-chain) | Yes |

**Key economics:**
- Anchoring cost: ~$0.08 per batch of 100 events (~$0.0008/event)
- ssUSD yield: ~5.00% APY (T-Bill backed, 0.20% protocol spread)
- Gas sponsorship: Merchants sponsor agent gas via SetPaymaster tiers

**Configuration:**
```bash
# Add chain RPC to sync.json
stateset-sync init --chain-rpc https://rpc.stateset.zone
```

**Best for:** DeFi integrations, autonomous agent economies, on-chain commerce settlements, yield-bearing escrow, regulated industries requiring ZK compliance proofs.

## Tier Detection

iCommerce automatically detects your tier based on configuration:

```javascript
import { detectTier, TIERS } from '@stateset/cli/standalone';

const tier = detectTier();
// TIERS.STANDALONE  — no sync.json
// TIERS.SEQUENCER   — sync.json present
// TIERS.FULL        — sync.json + chain RPC configured
```

## Capability Matrix

| Capability | Tier 1 | Tier 2 | Tier 3 |
|------------|--------|--------|--------|
| Commerce engine (SQLite) | Yes | Yes | Yes |
| Platform adapters (Stripe, WooCommerce, Shopify) | Yes | Yes | Yes |
| Policy engine | Yes | Yes | Yes |
| Webhook server | Yes | Yes | Yes |
| VES sync | | Yes | Yes |
| Cryptographic audit trail | | Yes | Yes |
| x402 payment intents | | Yes | Yes |
| On-chain settlement | | | Yes |
| Stablecoins (ssUSD) | | | Yes |
| SET Chain L2 | | | Yes |

## Upgrade Path

Tiers are additive — upgrading never breaks existing functionality:

```
Tier 1 (Standalone)
  └── Add sync.json → Tier 2 (Sequencer)
       └── Add chain RPC → Tier 3 (Full Trilogy)
```

No data migration required. Your SQLite database, policies, and adapter configurations carry forward.

## Learn More

- [The StateSet Trilogy](trilogy/overview.md) — Full three-layer architecture overview
- [Sequencer](trilogy/sequencer.md) — How the sequencer provides canonical ordering
- [SET Chain](trilogy/set-chain.md) — Commerce-optimized L2 with gas abstraction
- [STARK Proofs](trilogy/stark-proofs.md) — Zero-knowledge compliance verification
- [ssUSD](trilogy/ssusd.md) — Yield-bearing settlement stablecoin
