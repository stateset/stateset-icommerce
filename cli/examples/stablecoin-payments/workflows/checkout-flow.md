# E-commerce Checkout with Stablecoin Payment

Complete cart-to-payment flow using native stablecoins.

## Overview

This workflow demonstrates a full e-commerce checkout where the customer pays with USDC/stablecoins instead of credit cards.

## Step-by-Step Flow

### 1. Create Shopping Cart

```bash
# Create cart for customer
stateset --apply "create cart for customer@example.com"

# Output:
# Created cart CART-2024-abc123 for customer@example.com
# Session ID: sess_xyz789
```

### 2. Add Items to Cart

```bash
# Add products (using session for context)
stateset --apply --resume sess_xyz789 "add 2x Wireless Headphones at $149.99"
stateset --apply --resume sess_xyz789 "add 1x Phone Stand at $29.99"

# View cart
stateset --resume sess_xyz789 "show my cart"

# Output:
# Cart CART-2024-abc123
# ─────────────────────────────────────
# 2x Wireless Headphones    $299.98
# 1x Phone Stand            $29.99
# ─────────────────────────────────────
# Subtotal:                 $329.97
```

### 3. Set Shipping Address

```bash
stateset --apply --resume sess_xyz789 "ship to 456 Oak Ave, Austin TX 78701"

# Output:
# Shipping address set:
#   456 Oak Ave
#   Austin, TX 78701
#   United States
```

### 4. Calculate Tax

```bash
stateset --resume sess_xyz789 "calculate tax"

# Output:
# Tax Calculation (Texas):
#   Subtotal:    $329.97
#   Tax (8.25%): $27.22
#   ─────────────
#   Total:       $357.19
```

### 5. Get Shipping Rates

```bash
stateset --resume sess_xyz789 "show shipping options"

# Output:
# Shipping Options:
#   1. Standard (5-7 days)  $5.99
#   2. Express (2-3 days)   $12.99
#   3. Overnight            $24.99

stateset --apply --resume sess_xyz789 "select express shipping"

# Final total: $370.18
```

### 6. Customer Chooses Stablecoin Payment

```bash
# Display merchant wallet for payment
stateset pay --wallet --chain solana

# Output:
# Pay with USDC on Solana
# Send exactly: 370.18 USDC
# To address: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
#
# Or scan QR code: [QR displayed]
```

### 7. Customer Sends Payment

Customer sends 370.18 USDC from their wallet to the merchant address.

### 8. Verify Payment Received

```bash
# Check for incoming payment
stateset pay --balance --chain solana

# Or verify specific transaction
stateset "check if payment received for cart CART-2024-abc123"

# Output:
# Payment detected!
#   Amount: 370.18 USDC
#   From: 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
#   Tx: 5UfgJ2kN...
#   Confirmations: 32 ✓
```

### 9. Complete Checkout

```bash
stateset --apply --resume sess_xyz789 "complete checkout - paid via Solana USDC tx 5UfgJ2kN..."

# Output:
# Order created: ORD-2024-001234
#   Status: Paid
#   Payment: 370.18 USDC (Solana)
#   Tx: 5UfgJ2kN...
#
# Confirmation email sent to customer@example.com
```

### 10. Fulfill Order

```bash
# Pack and ship
stateset --apply "ship order ORD-2024-001234 with tracking FEDEX789456123"

# Output:
# Order ORD-2024-001234 shipped
#   Carrier: FedEx
#   Tracking: FEDEX789456123
#   ETA: 2-3 business days
#
# Shipping notification sent to customer@example.com
```

## Complete Script

```bash
#!/bin/bash
# Full checkout flow script

CUSTOMER="customer@example.com"
CHAIN="solana"

# Create cart
RESULT=$(stateset --apply --json "create cart for $CUSTOMER")
CART_ID=$(echo $RESULT | jq -r '.cart_id')
SESSION=$(echo $RESULT | jq -r '.session_id')

# Add items
stateset --apply --resume $SESSION "add 2x Wireless Headphones at 149.99"
stateset --apply --resume $SESSION "add 1x Phone Stand at 29.99"

# Set shipping
stateset --apply --resume $SESSION "ship to 456 Oak Ave, Austin TX 78701"
stateset --apply --resume $SESSION "select express shipping"

# Get total
TOTAL=$(stateset --resume $SESSION --json "what's the total?" | jq -r '.total')
echo "Total: $TOTAL USDC"

# Display payment address
WALLET=$(stateset pay --wallet --chain $CHAIN --json | jq -r '.address')
echo "Send $TOTAL USDC to: $WALLET"

# Wait for payment (in production, use webhook or polling)
echo "Waiting for payment..."
# ... payment monitoring logic ...

# Complete checkout
stateset --apply --resume $SESSION "complete checkout - paid via $CHAIN USDC"
```

## Multi-Chain Support

```bash
# Customer can choose their preferred chain

# Option 1: Solana (fastest, cheapest)
stateset pay --wallet --chain solana
# Send to: 9WzD...AWWM

# Option 2: Base (Coinbase ecosystem)
stateset pay --wallet --chain base
# Send to: 0x742d...fE21

# Option 3: SET Chain (yield-bearing)
stateset pay --wallet --chain set_chain
# Send to: 0x742d...fE21

# All addresses derived from same agent identity
# Payment auto-detected regardless of chain
```

## Handling Payment Failures

```bash
# If payment not received within timeout

stateset --resume sess_xyz789 "check payment status"

# Output:
# Payment Status: Pending
# Waiting for: 370.18 USDC
# Time remaining: 25 minutes
#
# If you've already sent payment, please provide:
#   - Transaction hash
#   - Sending wallet address

# Manual verification
stateset --apply "verify payment tx 5UfgJ2kN... for cart CART-2024-abc123"
```

## Refund Flow

```bash
# If customer requests refund

# 1. Create return
stateset --apply "create return for order ORD-2024-001234"

# 2. Approve return
stateset --apply "approve return RET-001234"

# 3. Refund to customer's original wallet
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 370.18 \
  --chain solana \
  --order ORD-2024-001234 \
  --memo "Refund for return RET-001234"

# 4. Update return status
stateset --apply "mark return RET-001234 as refunded"
```
