# StateSet iCommerce — Three-Tier Architecture

StateSet iCommerce is available in three tiers. Start with Tier 1 for free, upgrade when you need it.

## Tier 1: iCommerce Standalone (Free, Open Source)

**The SQLite of Commerce** — everything runs locally, no external services required.

| Feature | Included |
|---------|----------|
| SQLite commerce engine | Yes |
| 650+ MCP tools (orders, inventory, payments, supply chain, finance, etc.) | Yes |
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

---

## Tier 2: iCommerce + Sequencer (Enterprise)

**The Git of Commerce Data** — adds cryptographic sync, multi-agent coordination, and audit trails.

Everything in Tier 1, plus:

| Feature | Included |
|---------|----------|
| VES (Verifiable Event Sync) | Yes |
| Multi-agent state coordination | Yes |
| Cryptographic audit trail (Ed25519 signed events) | Yes |
| Merkle-tree anchoring | Yes |
| Outbox pattern for reliable delivery | Yes |
| Event replay & time-travel debugging | Yes |
| Multi-tenant support | Yes |

**Configuration:**
```bash
# Add sync.json to enable Sequencer
stateset-sync init --sequencer-url https://sequencer.stateset.com
```

**Best for:** Enterprise teams, regulated industries (fintech, healthcare, supply chain), multi-agent orchestration.

---

## Tier 3: Full Trilogy — iCommerce + Sequencer + SET Chain (Settlement)

**The Settlement Layer of AI Economy** — adds on-chain anchoring, native stablecoins, and autonomous agent payments.

Everything in Tiers 1 & 2, plus:

| Feature | Included |
|---------|----------|
| SET Chain L2 (OP Stack + STARKs) | Yes |
| ssUSD yield-bearing stablecoin | Yes |
| On-chain state anchoring | Yes |
| x402 protocol for AI agent payments | Yes |
| SetPaymaster (gasless UX) | Yes |
| Cross-chain bridges (Base, Arbitrum, Ethereum) | Yes |

**Configuration:**
```bash
# Add chain RPC to sync.json
stateset-sync init --chain-rpc https://rpc.stateset.zone
```

**Best for:** DeFi integrations, autonomous agent economies, on-chain commerce settlements, yield-bearing escrow.

---

## Tier Detection

iCommerce automatically detects your tier based on configuration:

```javascript
import { detectTier, TIERS } from '@stateset/cli/standalone';

const tier = detectTier();
// TIERS.STANDALONE  — no sync.json
// TIERS.SEQUENCER   — sync.json present
// TIERS.FULL        — sync.json + chain RPC configured
```

## Upgrade Path

Tiers are additive — upgrading never breaks existing functionality:

```
Tier 1 (Standalone)
  └── Add sync.json → Tier 2 (Sequencer)
       └── Add chain RPC → Tier 3 (Full Trilogy)
```

No data migration required. Your SQLite database, policies, and adapter configurations carry forward.
