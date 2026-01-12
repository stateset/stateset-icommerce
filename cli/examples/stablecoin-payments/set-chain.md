# SET Chain ssUSD Payments

Native yield-bearing stablecoin payments on StateSet's L2.

**Best for:** StateSet-native commerce, yield optimization, AI agent payments

## What is ssUSD?

ssUSD is StateSet's native stablecoin with built-in yield:
- **Yield-bearing** - Earns ~4-5% APY while held
- **Native integration** - First-class support in StateSet commerce
- **L2 efficiency** - Low fees on OP Stack rollup
- **Wrapped variant** - wssUSD for DeFi compatibility

## Setup

```bash
# Get your SET Chain wallet address
stateset pay --wallet --chain set_chain

# Output:
# Agent Wallet (SET Chain L2)
#   Agent:   default
#   Chain:   set_chain
#   Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   Explorer: https://explorer.setchain.io/address/0x742d...

# Check ssUSD balance
stateset pay --balance --chain set_chain
```

## Basic Payment

```bash
# Simulate payment
stateset pay --to 0x1234567890abcdef1234567890abcdef12345678 \
  --amount 100.00 \
  --chain set_chain

# Output:
# Payment Preview
#   Chain:     SET Chain L2
#   Token:     ssUSD
#   Amount:    100.00 ssUSD
#   To:        0x1234567890abcdef1234567890abcdef12345678
#   Mode:      SIMULATION (use --apply to execute)

# Execute payment
stateset pay --apply \
  --to 0x1234567890abcdef1234567890abcdef12345678 \
  --amount 100.00 \
  --chain set_chain

# Output:
# Payment confirmed!
#   Transaction: 0xabc123...
#   Explorer:    https://explorer.setchain.io/tx/0xabc123...
#   Block:       1234567
#   Confirms:    12
```

## StateSet Commerce Integration

```bash
# Full commerce flow with native ssUSD

# 1. Create customer
stateset --apply "create customer merchant@example.com"

# 2. Create order
stateset --apply "create order for merchant@example.com: 10x API Credits at $10.00"

# 3. Pay with ssUSD (native StateSet currency)
stateset pay --apply \
  --to 0x1234567890abcdef1234567890abcdef12345678 \
  --amount 100.00 \
  --chain set_chain \
  --order ORD-2024-005678 \
  --memo "API Credits purchase"

# 4. Confirm and fulfill
stateset --apply "mark order ORD-2024-005678 as paid and fulfilled"
```

## AI Agent Autonomous Commerce

```bash
# Agents can hold ssUSD and earn yield between transactions

# Check agent's ssUSD holdings
stateset "what's my ssUSD balance on SET Chain?"

# Agent-to-agent payment
stateset --apply "pay 250 ssUSD to 0x9876...4321 on SET Chain for inventory restock"

# Multi-agent commerce scenario
stateset --apply "create purchase order for 100 widgets from supplier agent 0xABCD..."
stateset --apply --resume <session> "pay the PO with ssUSD"
```

## Yield Tracking

```bash
# ssUSD automatically accrues yield
# Check effective balance (principal + yield)

stateset pay --balance --chain set_chain --json

# Output:
# {
#   "chain": "set_chain",
#   "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21",
#   "balance": "1050.25",
#   "symbol": "ssUSD",
#   "yieldAccrued": "50.25",
#   "apy": "4.8%"
# }
```

## B2B Invoice Settlement

```bash
# 1. Create B2B invoice
stateset --apply "create invoice for Acme Corp: $5000 for Q1 services"

# 2. Send invoice
stateset --apply "send invoice INV-2024-001 to accounting@acmecorp.com"

# 3. Receive ssUSD payment
# (Customer pays to your SET Chain address)

# 4. Record payment
stateset --apply "record payment of 5000 ssUSD for invoice INV-2024-001"
```

## Subscription Billing with ssUSD

```bash
# Create subscription plan with ssUSD pricing
stateset --apply "create monthly plan 'Pro Tier' at 99 ssUSD"

# Subscribe customer
stateset --apply "subscribe merchant@example.com to Pro Tier plan"

# Recurring billing happens automatically
# Payments settled in ssUSD on SET Chain
```

## Cross-Chain Bridge (Coming Soon)

```bash
# Bridge USDC from other chains to ssUSD
# stateset bridge --from solana --to set_chain --amount 1000

# Bridge ssUSD out to other chains
# stateset bridge --from set_chain --to base --amount 500
```

## Testnet

```bash
# Use SET Chain testnet for development
stateset pay --wallet --chain set_chain_testnet
stateset pay --balance --chain set_chain_testnet

# Get testnet ssUSD from faucet
# https://faucet.setchain.io
```

## Transaction Fees

| Operation | Typical Fee |
|-----------|-------------|
| ssUSD Transfer | ~0.001 ssUSD (~$0.001) |
| Confirmation Time | ~2 seconds |
| Finality | ~15 minutes (L1 settlement) |

## Advantages of SET Chain

1. **Native yield** - ssUSD earns while you hold
2. **Lowest fees** - Optimized for StateSet commerce
3. **Direct integration** - No bridges for StateSet operations
4. **AI-first** - Designed for autonomous agent commerce
5. **Event sourcing** - Full audit trail via VES
