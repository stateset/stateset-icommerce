# Base USDC Payments

Low-cost stablecoin payments on Coinbase's L2 network.

**Best for:** Coinbase ecosystem, easy fiat onramps, retail commerce

## Why Base?

- **Coinbase integration** - Easy fiat on/off ramps
- **Low fees** - ~$0.01 per transaction
- **EVM compatible** - Standard Ethereum tooling
- **Growing ecosystem** - Strong DeFi and NFT presence

## Setup

```bash
# Get your Base wallet address
stateset pay --wallet --chain base

# Output:
# Agent Wallet (Base L2)
#   Agent:   default
#   Chain:   base
#   Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   Explorer: https://basescan.org/address/0x742d...

# Check USDC balance
stateset pay --balance --chain base
```

## Basic Payment

```bash
# Simulate payment
stateset pay --to 0xRecipientAddress1234567890abcdef12345678 \
  --amount 75.00 \
  --chain base

# Execute real payment
stateset pay --apply \
  --to 0xRecipientAddress1234567890abcdef12345678 \
  --amount 75.00 \
  --chain base

# Output:
# Payment confirmed!
#   Transaction: 0xdef456...
#   Explorer:    https://basescan.org/tx/0xdef456...
#   Block:       8765432
#   Confirms:    12
```

## E-commerce with Coinbase Onramp

```bash
# Typical flow: Customer buys USDC via Coinbase, pays merchant on Base

# 1. Create cart
stateset --apply "create cart for customer@gmail.com"

# 2. Add items
stateset --apply --resume <session> "add 1 Wireless Earbuds at $79.99"
stateset --apply --resume <session> "add 1 Phone Case at $29.99"

# 3. Set shipping
stateset --apply --resume <session> "ship to 123 Main St, San Francisco CA 94102"

# 4. Calculate total with tax
stateset --resume <session> "what's the total with tax?"
# Output: $117.38 (including CA sales tax)

# 5. Customer pays from Coinbase wallet to merchant Base address
stateset pay --apply \
  --to 0xMerchantWallet1234567890abcdef12345678 \
  --amount 117.38 \
  --chain base \
  --order CART-abc123

# 6. Complete checkout
stateset --apply --resume <session> "complete checkout - paid via Base USDC"
```

## Merchant Payouts

```bash
# Daily settlement to merchant bank via Coinbase

# 1. Check today's revenue
stateset "what's today's total revenue?"
# Output: $2,450.00 from 47 orders

# 2. Transfer to Coinbase-linked wallet for offramp
stateset pay --apply \
  --to 0xCoinbaseWallet1234567890abcdef12345678 \
  --amount 2450.00 \
  --chain base \
  --memo "Daily settlement 2024-01-15"

# 3. Coinbase converts to USD and deposits to bank
```

## Multi-Store Commerce

```bash
# Marketplace with multiple sellers

# Buyer purchases from Seller A and Seller B
stateset --apply "create order: Widget from Seller A ($50), Gadget from Seller B ($75)"

# Split payment to sellers
stateset pay --apply --to 0xSellerA... --amount 47.50 --chain base --memo "Order split - Seller A (95%)"
stateset pay --apply --to 0xSellerB... --amount 71.25 --chain base --memo "Order split - Seller B (95%)"

# Platform fee retained
# $6.25 kept in platform wallet (5% commission)
```

## Refund Processing

```bash
# Customer requests refund

# 1. Create return
stateset --apply "create return for order ORD-2024-001234 - item defective"

# 2. Approve return
stateset --apply "approve return RET-001234"

# 3. Refund via Base USDC
stateset pay --apply \
  --to 0xCustomerWallet1234567890abcdef12345678 \
  --amount 79.99 \
  --chain base \
  --order ORD-2024-001234 \
  --memo "Refund - defective item"

# 4. Update return status
stateset --apply "mark return RET-001234 as refunded"
```

## AI Agent Integration

```bash
# Natural language commerce on Base

stateset --apply "pay 100 USDC to 0x1234...5678 on Base for the wholesale order"

stateset "check my Base wallet balance"

stateset --apply "process refund of 49.99 USDC to customer 0xABCD... on Base"
```

## Gas Optimization

```bash
# Base has low fees, but here are tips for high volume

# 1. Batch payments during low-activity periods
stateset pay --apply --to 0xVendor1... --amount 500 --chain base
stateset pay --apply --to 0xVendor2... --amount 750 --chain base
stateset pay --apply --to 0xVendor3... --amount 300 --chain base

# 2. Use JSON output for programmatic processing
stateset pay --apply --to 0x... --amount 100 --chain base --json | jq '.txHash'
```

## Transaction Fees

| Operation | Typical Fee |
|-----------|-------------|
| USDC Transfer | ~0.0001 ETH (~$0.01) |
| Confirmation Time | ~2 seconds |
| L1 Finality | ~15 minutes |

## Coinbase Wallet Integration

```bash
# For customers using Coinbase Wallet

# 1. Display merchant Base address
stateset pay --wallet --chain base
# → 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21

# 2. Customer scans QR or copies address

# 3. Customer sends USDC from Coinbase Wallet

# 4. Confirm receipt
stateset pay --balance --chain base
```

## Testnet (Base Sepolia)

```bash
# For development/testing
# Configure to use Base Sepolia testnet

# Get testnet ETH for gas
# https://www.coinbase.com/faucets/base-ethereum-sepolia-faucet

# Get testnet USDC
# Bridge from Sepolia or use test faucets
```
