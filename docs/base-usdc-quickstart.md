# Base + USDC Quickstart

Accept USDC payments on Base L2 with StateSet iCommerce.

## Why Base?

- **Low fees**: ~$0.01 per transaction
- **USDC liquidity**: Native Circle USDC with deep liquidity
- **Coinbase ecosystem**: Onramp/offramp via Coinbase
- **EVM compatible**: Standard Solidity tooling
- **iCommerce default**: Base is the recommended chain for standalone users

## Quick Start

### 1. Initialize

```bash
npm install -g @stateset/cli
stateset-init --quickstart
```

### 2. Check Supported Chains

```bash
stateset pay --chains
# Output includes: base (Base L2 — USDC)
```

### 3. Get Your Agent Wallet

```bash
stateset pay --wallet --chain base
# Displays your agent's Base wallet address
```

### 4. Check Balance

```bash
stateset pay --balance --chain base
```

### 5. Send a Payment

```bash
# Preview (safe — no transaction sent)
stateset pay --to 0xRecipient... --amount 50.00 --chain base

# Execute (requires --apply)
stateset pay --apply --to 0xRecipient... --amount 50.00 --chain base --order ORD-123
```

### 6. Via AI Interface

```bash
stateset --apply "pay 50 USDC to 0xRecipient on Base for order ORD-123"
stateset "check my wallet balance on Base"
```

## Architecture

```
Customer → iCommerce (Tier 1) → Base L2 (USDC)
                                   ↓
                              Coinbase Offramp → Bank Account
```

iCommerce handles:
- Order management, inventory, and fulfillment locally (SQLite)
- Payment execution via Base L2 USDC
- Audit trail and reconciliation

No Sequencer or SET Chain required — this is a pure Tier 1 setup.

## Upgrading to SET Chain

When you're ready for yield-bearing stablecoins (ssUSD) and on-chain state anchoring:

```bash
# Add SET Chain support
stateset-sync init --chain-rpc https://rpc.stateset.zone

# Pay with ssUSD (yield-bearing)
stateset pay --apply --to 0xRecipient... --amount 50.00 --chain set_chain
```

See [Three-Tier Architecture](tiers.md) for the full upgrade path.
