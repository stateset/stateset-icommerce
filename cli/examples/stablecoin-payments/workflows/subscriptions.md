# Subscription Billing with Stablecoins

Recurring stablecoin payments for subscription-based businesses.

## Overview

Accept recurring cryptocurrency payments for:
- SaaS subscriptions
- Membership programs
- Content subscriptions
- Service retainers

## Create Subscription Plans

### Basic Plans

```bash
# Create monthly plan
stateset --apply "create subscription plan 'Pro Monthly' at $29.99/month"

# Create annual plan (with discount)
stateset --apply "create subscription plan 'Pro Annual' at $299.99/year"

# Create plan with trial
stateset --apply "create subscription plan 'Starter' at $9.99/month with 14-day trial"
```

### View Plans

```bash
stateset "list subscription plans"

# Output:
# Subscription Plans
#   ─────────────────────────────────────────────────────
#   PLAN-001  Pro Monthly   $29.99/month   Active
#   PLAN-002  Pro Annual    $299.99/year   Active
#   PLAN-003  Starter       $9.99/month    Active (14-day trial)
#   ─────────────────────────────────────────────────────
```

## Subscribe Customers

### New Subscription with Stablecoin

```bash
# Customer provides wallet address during signup
stateset --apply "subscribe alice@example.com to Pro Monthly \
  payment method: USDC on Solana from 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"

# Output:
# Subscription created: SUB-2024-001234
#   Customer: alice@example.com
#   Plan: Pro Monthly ($29.99/month)
#   Payment: USDC on Solana
#   Next billing: February 1, 2024
#   Status: Active
```

### Subscription with Trial

```bash
stateset --apply "subscribe bob@example.com to Starter plan"

# Output:
# Subscription created: SUB-2024-001235
#   Customer: bob@example.com
#   Plan: Starter ($9.99/month)
#   Trial ends: January 28, 2024
#   First billing: January 29, 2024
#   Status: Trialing
```

## Billing Cycle

### Process Monthly Billing

```bash
# View subscriptions due for billing
stateset "show subscriptions due for billing today"

# Output:
# Billing Due - January 15, 2024
#   SUB-2024-001234  alice@example.com   Pro Monthly   $29.99
#   SUB-2024-001240  carol@example.com   Pro Monthly   $29.99
#   SUB-2024-001245  dave@example.com    Pro Annual    $299.99
#   ─────────────────────────────────────────────────────────
#   Total: $359.97 from 3 subscriptions

# Process billing (request payments)
stateset --apply "process subscription billing for today"

# Output:
# Billing processed for 3 subscriptions
#   Payment requests sent to customer wallets
#   Monitoring for incoming payments...
```

### Customer Pays

Customers send USDC from their registered wallets.

### Record Payment

```bash
# When payment detected
stateset --apply "record subscription payment for SUB-2024-001234: \
  29.99 USDC via Solana tx 5UfgJ..."

# Output:
# Payment recorded for SUB-2024-001234
#   Amount: $29.99 USDC
#   Tx: 5UfgJ...
#   Next billing: February 15, 2024
#   Status: Active
```

## Automated Pull Payments (Advanced)

### With Pre-Authorized Wallet

```bash
# If customer has pre-authorized your agent to pull payments:

# Process authorized recurring payment
stateset pay --apply \
  --from-customer-wallet SUB-2024-001234 \
  --amount 29.99 \
  --chain solana \
  --memo "Pro Monthly - February 2024"

# Requires customer to have:
# 1. Approved your agent's wallet for recurring debits
# 2. Maintained sufficient USDC balance
```

## Subscription Management

### View Subscriber Details

```bash
stateset "show subscription details for SUB-2024-001234"

# Output:
# Subscription SUB-2024-001234
#   ─────────────────────────────────────────
#   Customer: alice@example.com
#   Plan: Pro Monthly ($29.99/month)
#   Status: Active
#   Started: January 15, 2024
#   Next billing: February 15, 2024
#
#   Payment Method:
#     Chain: Solana
#     Wallet: 7xKX...AsU
#
#   Billing History:
#     Jan 15  $29.99  Paid  5UfgJ...
#   ─────────────────────────────────────────
```

### Pause Subscription

```bash
stateset --apply "pause subscription SUB-2024-001234"

# Output:
# Subscription SUB-2024-001234 paused
#   Billing suspended
#   Customer notified
#   Can resume anytime
```

### Resume Subscription

```bash
stateset --apply "resume subscription SUB-2024-001234"

# Output:
# Subscription SUB-2024-001234 resumed
#   Next billing: February 15, 2024
#   Status: Active
```

### Cancel Subscription

```bash
stateset --apply "cancel subscription SUB-2024-001234"

# Output:
# Subscription SUB-2024-001234 cancelled
#   Access until: February 15, 2024 (end of billing period)
#   Status: Cancelled
#   Cancellation survey sent to customer
```

### Upgrade/Downgrade

```bash
# Upgrade with proration
stateset --apply "upgrade subscription SUB-2024-001234 to Pro Annual"

# Output:
# Subscription upgraded
#   From: Pro Monthly ($29.99/month)
#   To: Pro Annual ($299.99/year)
#   Proration credit: $15.00 (unused days)
#   Amount due: $284.99
#
#   Payment request sent for 284.99 USDC
```

## Failed Payment Handling

### Payment Failed

```bash
stateset "show failed subscription payments"

# Output:
# Failed Payments
#   SUB-2024-001250  frank@example.com  $29.99  Insufficient balance
#   SUB-2024-001255  grace@example.com  $29.99  Wallet not found
```

### Retry Payment

```bash
# Send payment reminder
stateset --apply "send payment reminder for subscription SUB-2024-001250"

# Output:
# Payment reminder sent to frank@example.com
#   Amount due: $29.99 USDC
#   Wallet: 9WzD...
#   Grace period: 7 days
#
#   If unpaid, subscription will be suspended on January 22
```

### Grace Period Actions

```bash
# Check grace period subscriptions
stateset "show subscriptions in grace period"

# Suspend after grace period
stateset --apply "suspend subscription SUB-2024-001250 - payment failed"

# Output:
# Subscription SUB-2024-001250 suspended
#   Reason: Payment failed
#   Access revoked
#   Can reactivate upon payment
```

### Reactivate

```bash
# Customer pays overdue amount
stateset --apply "record payment for suspended subscription SUB-2024-001250: \
  29.99 USDC via Solana"

# Output:
# Subscription SUB-2024-001250 reactivated
#   Payment received: $29.99 USDC
#   Access restored
#   Next billing: February 22, 2024
```

## Multi-Chain Support

### Accept Multiple Chains

```bash
# Customer can pay from any supported chain
stateset "show payment options for subscription SUB-2024-001234"

# Output:
# Payment Options for SUB-2024-001234
#   Amount due: $29.99 USDC
#
#   Solana:    9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
#   Base:      0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   Arbitrum:  0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   SET Chain: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21 (ssUSD)
```

## Reporting

### Subscription Metrics

```bash
stateset "show subscription metrics for January 2024"

# Output:
# Subscription Metrics - January 2024
#   ─────────────────────────────────────────
#   Active Subscriptions:     247
#   New Subscriptions:        32
#   Churned:                  8
#   Net Growth:               +24
#
#   MRR (Monthly Recurring):  $7,382.53
#   MRR Growth:               +8.2%
#
#   Payment Methods:
#     Solana USDC:   145 (58.7%)
#     Base USDC:     62 (25.1%)
#     SET Chain:     28 (11.3%)
#     Arbitrum:      12 (4.9%)
#
#   Collection Rate:          96.4%
#   Avg Days to Pay:          1.2 days
#   ─────────────────────────────────────────
```

### Churn Analysis

```bash
stateset "analyze subscription churn for Q4 2023"

# Output:
# Churn Analysis - Q4 2023
#   Total Churned: 24 subscriptions
#
#   Reasons:
#     Payment failed (no retry): 8 (33%)
#     Customer requested: 10 (42%)
#     Downgraded to free: 4 (17%)
#     Other: 2 (8%)
#
#   Lost MRR: $718.76
#
#   Recommendation: Implement payment retry with grace period
```

## Benefits vs Traditional

| Feature | Credit Card | Stablecoin |
|---------|-------------|------------|
| Processing Fee | 2.9% + $0.30 | $0.01-0.05 |
| Chargebacks | Yes (risk) | No |
| International | Complex | Same as domestic |
| Failed Payment Recovery | Retry with delays | Instant retry |
| Settlement | 2-7 days | Instant |
| PCI Compliance | Required | Not needed |
