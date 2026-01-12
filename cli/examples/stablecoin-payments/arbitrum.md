# Arbitrum USDC Payments

Fast, low-cost payments on Arbitrum One L2.

**Best for:** DeFi integration, frequent transactions, cost-sensitive operations

## Why Arbitrum?

- **Lowest EVM fees** - Often < $0.01 per transaction
- **Fast finality** - Sub-second confirmations
- **DeFi hub** - Rich ecosystem for yield and liquidity
- **Ethereum security** - Inherits L1 security guarantees

## Setup

```bash
# Get your Arbitrum wallet address
stateset pay --wallet --chain arbitrum

# Output:
# Agent Wallet (Arbitrum One)
#   Agent:   default
#   Chain:   arbitrum
#   Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   Explorer: https://arbiscan.io/address/0x742d...

# Check USDC balance
stateset pay --balance --chain arbitrum
```

## Basic Payment

```bash
# Simulate payment
stateset pay --to 0xRecipient1234567890abcdef1234567890abcdef \
  --amount 150.00 \
  --chain arbitrum

# Execute payment
stateset pay --apply \
  --to 0xRecipient1234567890abcdef1234567890abcdef \
  --amount 150.00 \
  --chain arbitrum

# Output:
# Payment confirmed!
#   Transaction: 0xfed987...
#   Explorer:    https://arbiscan.io/tx/0xfed987...
#   Block:       178234567
#   Confirms:    1
```

## High-Frequency Commerce

```bash
# Arbitrum is ideal for frequent, smaller transactions

# Gaming/digital goods purchases
stateset pay --apply --to 0xGameDev... --amount 4.99 --chain arbitrum --memo "In-game currency pack"
stateset pay --apply --to 0xGameDev... --amount 9.99 --chain arbitrum --memo "Premium skin bundle"
stateset pay --apply --to 0xGameDev... --amount 2.99 --chain arbitrum --memo "Extra lives"

# Micro-subscriptions
stateset pay --apply --to 0xContentCreator... --amount 5.00 --chain arbitrum --memo "Monthly tip"

# API usage payments
stateset pay --apply --to 0xAPIProvider... --amount 0.50 --chain arbitrum --memo "1000 API calls"
```

## Marketplace Operations

```bash
# Multi-vendor marketplace with instant settlements

# 1. Customer checkout
stateset --apply "complete checkout for cart CART-12345"

# 2. Instant split payments to vendors
stateset pay --apply \
  --to 0xVendorAlice... \
  --amount 45.00 \
  --chain arbitrum \
  --order ORD-12345 \
  --memo "Vendor split - Alice (Handmade Jewelry)"

stateset pay --apply \
  --to 0xVendorBob... \
  --amount 32.00 \
  --chain arbitrum \
  --order ORD-12345 \
  --memo "Vendor split - Bob (Vintage Clothing)"

stateset pay --apply \
  --to 0xVendorCarol... \
  --amount 18.00 \
  --chain arbitrum \
  --order ORD-12345 \
  --memo "Vendor split - Carol (Art Prints)"

# 3. Platform fee collected
# $5.00 retained (5% of $100 order)
```

## Subscription Billing

```bash
# Low-cost recurring billing on Arbitrum

# 1. Create subscription plan
stateset --apply "create monthly plan 'Creator Pro' at $19.99"

# 2. Subscribe customers
stateset --apply "subscribe creator1@email.com to Creator Pro"
stateset --apply "subscribe creator2@email.com to Creator Pro"
stateset --apply "subscribe creator3@email.com to Creator Pro"

# 3. Process monthly billing (automated)
# Each billing cycle:
stateset pay --apply --to 0xYourMerchantWallet... --amount 19.99 --chain arbitrum --customer cust_001
stateset pay --apply --to 0xYourMerchantWallet... --amount 19.99 --chain arbitrum --customer cust_002
stateset pay --apply --to 0xYourMerchantWallet... --amount 19.99 --chain arbitrum --customer cust_003

# Low fees make micro-billing viable
# 3 transactions × $0.01 = $0.03 total fees
```

## Affiliate/Referral Payouts

```bash
# Pay affiliates instantly when sales occur

# Sale happens
stateset --apply "create order from referral link REF-alice123"

# Instant affiliate commission
stateset pay --apply \
  --to 0xAliceAffiliate... \
  --amount 15.00 \
  --chain arbitrum \
  --memo "Referral commission - Order ORD-98765 (15%)"

# Batch daily affiliate payouts
stateset pay --apply --to 0xAffiliate1... --amount 45.00 --chain arbitrum --memo "Daily commission"
stateset pay --apply --to 0xAffiliate2... --amount 127.50 --chain arbitrum --memo "Daily commission"
stateset pay --apply --to 0xAffiliate3... --amount 22.00 --chain arbitrum --memo "Daily commission"
```

## DeFi Integration

```bash
# Arbitrum has rich DeFi ecosystem

# 1. Accept payment
stateset pay --apply \
  --to 0xYourWallet... \
  --amount 10000.00 \
  --chain arbitrum \
  --memo "Large order payment"

# 2. Deploy to yield (manual step via DeFi protocol)
# Options: Aave, GMX, Radiant, etc.
# Earn yield on idle treasury

# 3. Check balance includes DeFi positions
stateset "what's my total treasury value on Arbitrum?"
```

## Refund Processing

```bash
# Fast, cheap refunds

# 1. Customer requests refund
stateset --apply "create return for order ORD-56789 - wrong size"

# 2. Approve and process instantly
stateset --apply "approve return RET-56789"

stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 89.99 \
  --chain arbitrum \
  --order ORD-56789 \
  --memo "Refund - wrong size"

# 3. Update status
stateset --apply "mark return RET-56789 as refunded"

# Total cost: ~$0.01 (vs $0.30+ credit card refund fee)
```

## AI Agent Workflows

```bash
# Autonomous agent commerce on Arbitrum

# Agent checks inventory and reorders automatically
stateset "check inventory levels for all products"
# → WIDGET-001 is low (5 units remaining)

stateset --apply "create purchase order for 100x WIDGET-001 from supplier"
# → PO created, total: $500

stateset pay --apply \
  --to 0xSupplierWallet... \
  --amount 500.00 \
  --chain arbitrum \
  --order PO-AUTO-001 \
  --memo "Automated restock - WIDGET-001"

# Agent confirms payment and updates inventory
stateset --apply "mark PO-AUTO-001 as paid, expect delivery in 3 days"
```

## Cost Comparison

```bash
# Why Arbitrum for frequent transactions

# Scenario: 100 daily transactions averaging $50 each

# Credit Card (2.9% + $0.30):
# 100 × ($1.45 + $0.30) = $175/day in fees

# Arbitrum USDC:
# 100 × $0.01 = $1/day in fees

# Monthly savings: ~$5,220
```

## Transaction Fees

| Operation | Typical Fee |
|-----------|-------------|
| USDC Transfer | ~0.0001 ETH (~$0.005-0.02) |
| Confirmation Time | < 1 second |
| L1 Finality | ~15 minutes |

## Batch Operations

```bash
# Process multiple payments efficiently

# Morning payouts
echo "Processing vendor payouts..."
stateset pay --apply --to 0xVendor1... --amount 234.50 --chain arbitrum &
stateset pay --apply --to 0xVendor2... --amount 567.00 --chain arbitrum &
stateset pay --apply --to 0xVendor3... --amount 123.75 --chain arbitrum &
wait
echo "All payouts complete"

# JSON output for logging
for vendor in vendor1 vendor2 vendor3; do
  stateset pay --apply \
    --to 0x${vendor}... \
    --amount 100.00 \
    --chain arbitrum \
    --json >> daily_payouts.jsonl
done
```

## When to Choose Arbitrum

| Use Case | Recommendation |
|----------|----------------|
| < 10 tx/day | Any chain works |
| 10-100 tx/day | Arbitrum or Base |
| 100+ tx/day | Arbitrum (lowest fees) |
| DeFi integration needed | Arbitrum |
| Coinbase ecosystem | Base |
| Maximum security | Ethereum |
