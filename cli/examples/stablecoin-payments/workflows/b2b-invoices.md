# B2B Invoice Settlement with Stablecoins

Enterprise invoice creation and stablecoin payment settlement.

## Overview

Replace slow bank wires and check payments with instant stablecoin settlements. Ideal for:
- Vendor payments
- Consulting/service invoices
- Wholesale orders
- International B2B transactions

## Invoice Creation

### Create Invoice

```bash
# Create B2B invoice
stateset --apply "create invoice for Acme Corporation: \
  - Consulting Services Q1: $15,000 \
  - Software License: $5,000 \
  - Support Package: $2,500 \
  Payment terms: NET30"

# Output:
# Invoice INV-2024-0042 created
#   Customer: Acme Corporation
#   ─────────────────────────────────
#   Consulting Services Q1   $15,000.00
#   Software License          $5,000.00
#   Support Package           $2,500.00
#   ─────────────────────────────────
#   Total:                   $22,500.00
#   Due Date: February 15, 2024
#   Status: Draft
```

### Add Payment Instructions

```bash
# Get your Ethereum address for high-value B2B
stateset pay --wallet --chain ethereum

# Add stablecoin payment option to invoice
stateset --apply "update invoice INV-2024-0042 add payment method: \
  USDC on Ethereum to 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21"
```

### Send Invoice

```bash
# Send to customer
stateset --apply "send invoice INV-2024-0042 to ap@acmecorp.com"

# Output:
# Invoice INV-2024-0042 sent to ap@acmecorp.com
#   Payment Options:
#   1. Bank Wire (ACH/Wire details included)
#   2. USDC on Ethereum: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   3. USDC on Base: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
```

## Payment Receipt

### Monitor for Payment

```bash
# Check if payment received
stateset pay --balance --chain ethereum

# Or query invoice status
stateset "check payment status for invoice INV-2024-0042"

# Output:
# Invoice INV-2024-0042
#   Total: $22,500.00
#   Status: Awaiting Payment
#   Due: February 15, 2024 (12 days remaining)
```

### Customer Pays

Customer sends 22,500 USDC to your Ethereum address.

### Record Payment

```bash
# When payment detected/confirmed
stateset --apply "record payment for invoice INV-2024-0042: \
  22,500 USDC received via Ethereum tx 0xabc123..."

# Output:
# Payment recorded for INV-2024-0042
#   Amount: $22,500.00 USDC
#   Method: Ethereum USDC
#   Tx: 0xabc123...
#   Status: Paid
#   Paid: 18 days early
```

## Partial Payments

### Accept Partial Payment

```bash
# Customer pays in installments
stateset --apply "record partial payment for invoice INV-2024-0042: \
  10,000 USDC via Ethereum tx 0xdef456..."

# Output:
# Partial payment recorded
#   Invoice: INV-2024-0042
#   Payment: $10,000.00 USDC
#   Remaining: $12,500.00
#   Status: Partially Paid
```

### Track Balance

```bash
stateset "show payment history for invoice INV-2024-0042"

# Output:
# Invoice INV-2024-0042 - Payment History
#   ─────────────────────────────────────────
#   Jan 20  $10,000.00  USDC/ETH  0xdef456...
#   ─────────────────────────────────────────
#   Total Paid:    $10,000.00
#   Remaining:     $12,500.00
#   Status:        Partially Paid
```

## Vendor Payments (Paying Invoices)

### Review Incoming Invoice

```bash
# Vendor sends you an invoice
stateset "show pending invoices to pay"

# Output:
# Invoices Payable
#   INV-V-001  TechSupplier Inc   $8,500.00   Due: Jan 25
#   INV-V-002  CloudServices Co   $3,200.00   Due: Jan 30
#   INV-V-003  Marketing Agency   $12,000.00  Due: Feb 5
```

### Pay Invoice with Stablecoin

```bash
# Pay vendor with USDC
stateset pay --apply \
  --to 0xTechSupplierWallet1234567890abcdef1234 \
  --amount 8500.00 \
  --chain arbitrum \
  --memo "Payment for INV-V-001"

# Output:
# Payment confirmed!
#   To: TechSupplier Inc (0xTech...1234)
#   Amount: 8,500.00 USDC
#   Chain: Arbitrum
#   Tx: 0xfed789...
#   Fee: $0.02

# Record in your books
stateset --apply "mark invoice INV-V-001 as paid via Arbitrum tx 0xfed789..."
```

## International Payments

### Cross-Border Invoice

```bash
# Invoice to international customer (avoids wire fees)
stateset --apply "create invoice for Munich GmbH (Germany): \
  - Software Development: €25,000 \
  - converted at 1.08 USD/EUR: $27,000 \
  Payment: USDC preferred"

# Send with multiple payment options
stateset --apply "send invoice INV-2024-INT-007 to finance@munich-gmbh.de"
```

### Receive International Payment

```bash
# Customer pays in USDC (avoids SWIFT fees, FX spreads)
stateset pay --balance --chain ethereum

# Record payment
stateset --apply "record payment for INV-2024-INT-007: \
  27,000 USDC via Ethereum"

# Benefits:
# - No $25-50 wire fee
# - No 2-5 day clearing time
# - No FX conversion spread
# - Instant settlement
```

## Recurring Invoices

### Set Up Recurring Invoice

```bash
# Monthly retainer invoice
stateset --apply "create recurring invoice for Client Corp: \
  Monthly Retainer $5,000 \
  Frequency: Monthly on 1st \
  Payment: USDC to 0x742d..."

# Auto-generates INV-2024-REC-001, INV-2024-REC-002, etc.
```

### Process Monthly

```bash
# Each month, invoice auto-created
stateset "show this month's recurring invoices"

# Output:
# Recurring Invoices - January 2024
#   INV-2024-REC-001  Client Corp     $5,000.00  Due: Jan 15
#   INV-2024-REC-002  Agency Partner  $3,500.00  Due: Jan 15
#   INV-2024-REC-003  Consulting Inc  $8,000.00  Due: Jan 15

# Send all
stateset --apply "send all pending recurring invoices"
```

## Overdue Invoice Management

### Check Overdue

```bash
stateset "show overdue invoices"

# Output:
# Overdue Invoices
#   INV-2024-0038  Slow Payer LLC   $4,500.00   15 days overdue
#   INV-2024-0041  Budget Corp      $2,200.00   3 days overdue
```

### Send Reminder

```bash
stateset --apply "send payment reminder for invoice INV-2024-0038"

# Output:
# Reminder sent for INV-2024-0038
#   To: ap@slowpayer.com
#   Amount Due: $4,500.00
#   Days Overdue: 15
#
#   Included: Stablecoin payment instructions for instant settlement
```

## Reporting

### Invoice Summary

```bash
stateset "summarize invoices for January 2024"

# Output:
# Invoice Summary - January 2024
#   ─────────────────────────────────────────
#   Total Invoiced:        $87,500.00
#   Paid (Stablecoin):     $52,000.00 (8 invoices)
#   Paid (Bank):           $15,000.00 (2 invoices)
#   Outstanding:           $20,500.00 (4 invoices)
#   Overdue:               $6,700.00  (2 invoices)
#   ─────────────────────────────────────────
#   Collection Rate: 76.6%
#   Avg Days to Pay: 12 days (stablecoin: 3 days, bank: 21 days)
```

### Export for Accounting

```bash
# Export paid invoices with blockchain proof
stateset "export paid invoices for January 2024" --json > jan_2024_invoices.json

# Each invoice includes:
# - Invoice details
# - Payment amount
# - Blockchain transaction hash
# - Block number (proof of payment)
# - Timestamp
```

## Benefits vs Traditional

| Metric | Bank Wire | Stablecoin |
|--------|-----------|------------|
| Settlement Time | 2-5 days | < 1 minute |
| Wire Fee | $25-50 | $0.01-15 |
| International | +$40, 3-5 days | Same as domestic |
| Weekend/Holiday | No processing | 24/7 |
| Proof of Payment | Statement | Blockchain tx |
| Reversibility | Possible | Final |
