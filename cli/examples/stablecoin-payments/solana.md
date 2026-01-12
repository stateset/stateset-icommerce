# Solana USDC Payments

Fast, low-cost stablecoin payments on Solana mainnet.

**Best for:** High-volume B2C, real-time settlements, mobile commerce

## Setup

```bash
# Get your Solana wallet address
stateset pay --wallet --chain solana

# Output:
# Agent Wallet (Solana Mainnet)
#   Agent:   default
#   Chain:   solana
#   Address: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
#   Explorer: https://explorer.solana.com/address/9WzD...

# Check balance
stateset pay --balance --chain solana
```

## Basic Payment

```bash
# Simulate first (safe preview)
stateset pay --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 25.00 \
  --chain solana

# Output:
# Payment Preview
#   Chain:     Solana Mainnet
#   Token:     USDC
#   Amount:    25.00 USDC
#   To:        7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
#   Mode:      SIMULATION (use --apply to execute)

# Execute real payment
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 25.00 \
  --chain solana

# Output:
# Payment confirmed!
#   Transaction: 5UfgJ...2kNvP
#   Explorer:    https://explorer.solana.com/tx/5UfgJ...
#   Block:       234567890
#   Confirms:    32
```

## E-commerce Order Flow

```bash
# 1. Create order
stateset --apply "create order for alice@example.com with 2x Widget Pro at $49.99"

# 2. Customer provides Solana wallet for payment
# 3. Create stablecoin payment for the order

stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 99.98 \
  --chain solana \
  --order ORD-2024-001234 \
  --customer cust_alice123 \
  --memo "2x Widget Pro"

# 4. Mark order as paid
stateset --apply "mark order ORD-2024-001234 as paid"

# 5. Ship the order
stateset --apply "ship order ORD-2024-001234 with tracking FEDEX123456"
```

## Batch Payments (Payouts)

```bash
# Pay multiple vendors/affiliates
stateset pay --apply --to 7xKX...AsU --amount 150.00 --chain solana --memo "Vendor payout - January"
stateset pay --apply --to 9WzD...WWM --amount 75.50 --chain solana --memo "Affiliate commission"
stateset pay --apply --to 3nKp...7Yz --amount 200.00 --chain solana --memo "Supplier payment"
```

## AI Agent Commerce

```bash
# Natural language payments
stateset --apply "pay 50 USDC to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU on Solana"

# Check balance conversationally
stateset "what's my Solana wallet balance?"

# Full checkout flow
stateset --apply "create a cart for bob@example.com"
stateset --apply --resume <session> "add 1 Premium Headphones at $199.99"
stateset --apply --resume <session> "checkout and pay with USDC on Solana to 7xKX...AsU"
```

## Devnet Testing

```bash
# Use devnet for testing (fake USDC)
stateset pay --wallet --chain solana_devnet
stateset pay --balance --chain solana_devnet

# Test payment on devnet
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 100.00 \
  --chain solana_devnet

# Get devnet USDC from faucet:
# https://spl-token-faucet.com/?token-name=USDC-Dev
```

## JSON Output (for integrations)

```bash
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 50.00 \
  --chain solana \
  --json

# Output:
# {
#   "success": true,
#   "chain": "solana",
#   "txHash": "5UfgJ...2kNvP",
#   "blockNumber": 234567890,
#   "confirmations": 32,
#   "amount": "50.00",
#   "symbol": "USDC",
#   "fromAddress": "9WzD...AWWM",
#   "toAddress": "7xKX...AsU",
#   "explorerUrl": "https://explorer.solana.com/tx/5UfgJ..."
# }
```

## Transaction Fees

| Operation | Typical Fee |
|-----------|-------------|
| USDC Transfer | ~0.00025 SOL (~$0.005) |
| Confirmation Time | ~400ms |
| Finality | ~13 seconds (32 confirms) |

## Troubleshooting

```bash
# Insufficient balance
stateset pay --balance --chain solana
# → Need to fund wallet with USDC

# Transaction failed
# Check Solana explorer for details
# Ensure recipient address is valid SPL token account

# Slow confirmation
# Solana network congestion - wait for finality
```
