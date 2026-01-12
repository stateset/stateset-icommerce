# Refund Processing with Stablecoins

Return-to-refund workflows using native stablecoin payments.

## Overview

Process refunds instantly via stablecoins:
- No 5-10 day credit card refund delays
- No chargeback disputes
- Instant customer satisfaction
- Full blockchain audit trail

## Standard Refund Flow

### 1. Customer Requests Return

```bash
# Customer initiates return
stateset --apply "create return for order ORD-2024-001234 \
  reason: item damaged in shipping \
  items: 1x Wireless Headphones"

# Output:
# Return request created: RET-2024-00567
#   Order: ORD-2024-001234
#   Customer: alice@example.com
#   Item: 1x Wireless Headphones ($149.99)
#   Reason: Item damaged in shipping
#   Status: Pending Review
```

### 2. Review Return Request

```bash
stateset "show return details RET-2024-00567"

# Output:
# Return RET-2024-00567
#   ─────────────────────────────────────────
#   Order: ORD-2024-001234
#   Customer: alice@example.com
#
#   Original Payment:
#     Amount: $149.99
#     Method: USDC on Solana
#     From: 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
#     Tx: 5UfgJ2kN...
#
#   Return Items:
#     1x Wireless Headphones  $149.99
#
#   Reason: Item damaged in shipping
#   Photos: 2 attached
#   Status: Pending Review
#   ─────────────────────────────────────────
```

### 3. Approve Return

```bash
stateset --apply "approve return RET-2024-00567"

# Output:
# Return RET-2024-00567 approved
#   Refund amount: $149.99
#   Return label generated
#   Customer notified
#   Status: Approved - Awaiting Item Return
```

### 4. Receive Returned Item

```bash
stateset --apply "mark return RET-2024-00567 item received"

# Output:
# Return RET-2024-00567 item received
#   Condition: Verified damaged
#   Ready for refund
#   Status: Ready to Refund
```

### 5. Process Refund

```bash
# Refund to customer's original wallet
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 149.99 \
  --chain solana \
  --order ORD-2024-001234 \
  --memo "Refund for RET-2024-00567 - damaged item"

# Output:
# Payment confirmed!
#   Amount: 149.99 USDC
#   To: 7xKX...AsU
#   Tx: 9AbcD3fG...
#   Block: 234567891
#   Confirms: 32
```

### 6. Complete Return

```bash
stateset --apply "mark return RET-2024-00567 as refunded via tx 9AbcD3fG..."

# Output:
# Return RET-2024-00567 completed
#   Refund: $149.99 USDC
#   Tx: 9AbcD3fG...
#   Status: Refunded
#   Customer notified via email
```

## Partial Refunds

### Partial Item Return

```bash
# Order had multiple items, customer returns one
stateset --apply "create partial return for order ORD-2024-001235 \
  items: 1x Phone Case ($29.99) \
  reason: wrong color"

# Approve and process
stateset --apply "approve return RET-2024-00568"
stateset --apply "mark return RET-2024-00568 item received"

# Partial refund
stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 29.99 \
  --chain base \
  --order ORD-2024-001235 \
  --memo "Partial refund - wrong color phone case"
```

### Partial Refund (Keep Item)

```bash
# Customer keeps item but gets partial refund (e.g., minor defect)
stateset --apply "create refund for order ORD-2024-001236 \
  amount: 25.00 \
  reason: minor scratch - customer keeping item"

stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 25.00 \
  --chain arbitrum \
  --order ORD-2024-001236 \
  --memo "Goodwill refund - minor defect"
```

## Instant Refunds (No Return Required)

### Low-Value Items

```bash
# For items under $20, refund without requiring return
stateset --apply "create instant refund for order ORD-2024-001237 \
  item: 1x USB Cable ($12.99) \
  reason: defective \
  note: no return required - low value item"

# Process immediately
stateset pay --apply \
  --to 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU \
  --amount 12.99 \
  --chain solana \
  --order ORD-2024-001237 \
  --memo "Instant refund - defective USB cable"

stateset --apply "mark refund complete for order ORD-2024-001237"
```

### Customer Satisfaction Refund

```bash
# Full refund for customer satisfaction (keep item)
stateset --apply "create goodwill refund for order ORD-2024-001238 \
  amount: 79.99 \
  reason: customer dissatisfied with quality"

stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 79.99 \
  --chain base \
  --memo "Goodwill refund - customer satisfaction"
```

## Multi-Chain Refunds

### Refund to Different Chain

```bash
# Customer paid on Solana but wants refund on Base
stateset "show original payment for order ORD-2024-001239"

# Output:
# Original Payment
#   Chain: Solana
#   Amount: 199.99 USDC
#   Tx: 5UfgJ2kN...

# Customer requests refund to Base wallet
stateset pay --apply \
  --to 0xCustomerNewBaseWallet1234567890abcdef \
  --amount 199.99 \
  --chain base \
  --order ORD-2024-001239 \
  --memo "Refund to customer Base wallet (originally paid Solana)"
```

### SET Chain Refund (with Yield Note)

```bash
# Customer paid with ssUSD
stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 99.99 \
  --chain set_chain \
  --order ORD-2024-001240 \
  --memo "Refund in ssUSD - will accrue yield"

# Customer receives ssUSD which earns yield while held
```

## Rejected Returns

### Reject Return Request

```bash
# Review and reject
stateset --apply "reject return RET-2024-00570 \
  reason: item not in original condition \
  evidence: photos show wear and tear beyond shipping damage"

# Output:
# Return RET-2024-00570 rejected
#   Reason: Item not in original condition
#   Customer notified with explanation
#   Appeal window: 7 days
```

### Customer Appeals

```bash
# Customer provides additional evidence
stateset --apply "reopen return RET-2024-00570 \
  new evidence: manufacturer confirmed defect \
  action: approve for refund"

# Process approved refund
stateset pay --apply \
  --to 0xCustomerWallet... \
  --amount 149.99 \
  --chain solana \
  --order ORD-2024-001241 \
  --memo "Refund after appeal - manufacturer defect confirmed"
```

## Automated Refund Processing

### Batch Refunds

```bash
# Process all approved returns ready for refund
stateset "show returns ready for refund"

# Output:
# Returns Ready for Refund
#   RET-2024-00571  alice@...   $49.99   Solana   7xKX...
#   RET-2024-00572  bob@...     $29.99   Base     0xABC...
#   RET-2024-00573  carol@...   $89.99   Arbitrum 0xDEF...

# Process all
stateset pay --apply --to 7xKX... --amount 49.99 --chain solana --memo "Refund RET-00571"
stateset pay --apply --to 0xABC... --amount 29.99 --chain base --memo "Refund RET-00572"
stateset pay --apply --to 0xDEF... --amount 89.99 --chain arbitrum --memo "Refund RET-00573"

# Mark all as refunded
stateset --apply "mark returns RET-00571, RET-00572, RET-00573 as refunded"
```

## Refund Reporting

### Refund Summary

```bash
stateset "show refund summary for January 2024"

# Output:
# Refund Summary - January 2024
#   ─────────────────────────────────────────
#   Total Refunds:        42
#   Total Amount:         $3,847.56
#   Refund Rate:          2.3% of orders
#
#   By Chain:
#     Solana:    $2,150.00 (24 refunds)
#     Base:      $897.56 (10 refunds)
#     Arbitrum:  $500.00 (5 refunds)
#     SET Chain: $300.00 (3 refunds)
#
#   By Reason:
#     Damaged in shipping: 15 (36%)
#     Wrong item:          10 (24%)
#     Defective:           8 (19%)
#     Changed mind:        6 (14%)
#     Other:               3 (7%)
#
#   Avg Processing Time:  0.5 days
#   (vs 5-10 days credit card)
#   ─────────────────────────────────────────
```

### Refund vs Chargeback Comparison

```bash
# Traditional: Customer disputes with credit card company
# - 2-3 months to resolve
# - $15-25 chargeback fee
# - Risk of losing dispute

# Stablecoin: Direct refund to customer
# - Instant processing
# - $0.01-0.05 transaction fee
# - No dispute possible (funds are final)
# - Customer satisfaction higher
```

## Benefits Summary

| Metric | Credit Card Refund | Stablecoin Refund |
|--------|-------------------|-------------------|
| Processing Time | 5-10 business days | < 1 minute |
| Cost | $0.30+ (or no refund of fee) | $0.01-0.05 |
| Disputes/Chargebacks | Possible | Not possible |
| Customer Satisfaction | Delayed | Instant |
| Audit Trail | Statement | Blockchain proof |
| Fraud Risk | Higher | Lower |
