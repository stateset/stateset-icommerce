# Ethereum USDC Payments

Maximum security stablecoin payments on Ethereum mainnet.

**Best for:** High-value B2B transactions, enterprise commerce, maximum security

## Why Ethereum Mainnet?

- **Maximum security** - Most battle-tested blockchain
- **Highest liquidity** - Deepest USDC markets
- **Enterprise trust** - Recognized by institutions
- **DeFi integration** - Access to full Ethereum ecosystem

## Setup

```bash
# Get your Ethereum wallet address
stateset pay --wallet --chain ethereum

# Output:
# Agent Wallet (Ethereum Mainnet)
#   Agent:   default
#   Chain:   ethereum
#   Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21
#   Explorer: https://etherscan.io/address/0x742d...

# Check USDC balance
stateset pay --balance --chain ethereum
```

## High-Value B2B Payment

```bash
# Enterprise transaction - $50,000 wholesale order

# 1. Verify sufficient balance
stateset pay --balance --chain ethereum
# Output: Balance: 75,000.00 USDC

# 2. Simulate first (critical for large amounts)
stateset pay --to 0xEnterpriseVendor1234567890abcdef12345678 \
  --amount 50000.00 \
  --chain ethereum

# Output:
# Payment Preview
#   Chain:     Ethereum Mainnet
#   Token:     USDC
#   Amount:    50,000.00 USDC
#   To:        0xEnterpriseVendor1234567890abcdef12345678
#   Gas Est:   ~0.005 ETH (~$15)
#   Mode:      SIMULATION (use --apply to execute)

# 3. Execute with full audit trail
stateset pay --apply \
  --to 0xEnterpriseVendor1234567890abcdef12345678 \
  --amount 50000.00 \
  --chain ethereum \
  --order PO-2024-ENT-001 \
  --customer enterprise_client_acme \
  --memo "Q1 inventory purchase - NET30 settlement"

# Output:
# Payment confirmed!
#   Transaction: 0x789abc...
#   Explorer:    https://etherscan.io/tx/0x789abc...
#   Block:       19234567
#   Confirms:    12
```

## Invoice Settlement Workflow

```bash
# B2B invoice → Ethereum USDC settlement

# 1. Create invoice
stateset --apply "create invoice for Acme Corp: $25,000 for consulting services"

# 2. Send to customer
stateset --apply "send invoice INV-2024-ENT-042 to cfo@acmecorp.com"

# 3. Customer pays to your Ethereum address
# (They send 25,000 USDC to 0x742d35Cc6634C0532925a3b844Bc9e7595f8fE21)

# 4. Verify receipt
stateset pay --balance --chain ethereum

# 5. Record payment
stateset --apply "record payment of $25,000 USDC for invoice INV-2024-ENT-042 via Ethereum tx 0x789abc..."
```

## Escrow for Large Orders

```bash
# For very large transactions, use escrow pattern

# 1. Create escrow order
stateset --apply "create order for enterprise@bigcorp.com: Custom Manufacturing $100,000"

# 2. Customer deposits to escrow
# Escrow address: 0xStateSetEscrow...

# 3. Verify escrow deposit
stateset pay --balance --chain ethereum
# (Check escrow contract balance)

# 4. Fulfill order milestones
stateset --apply "mark milestone 1 complete for order ORD-ENT-001"

# 5. Release partial payment
stateset pay --apply \
  --to 0xManufacturerWallet... \
  --amount 33333.33 \
  --chain ethereum \
  --order ORD-ENT-001 \
  --memo "Milestone 1 release - 33%"
```

## Multi-Sig Treasury Operations

```bash
# For enterprise treasury management

# 1. Check treasury balance
stateset pay --balance --chain ethereum --agent treasury
# Output: Balance: 500,000.00 USDC

# 2. Prepare monthly vendor payments
stateset pay --to 0xVendorA... --amount 15000.00 --chain ethereum --memo "Monthly retainer"
stateset pay --to 0xVendorB... --amount 8500.00 --chain ethereum --memo "SaaS subscription"
stateset pay --to 0xVendorC... --amount 22000.00 --chain ethereum --memo "Inventory restock"

# 3. Execute with multi-sig approval
# (Requires additional signers for amounts > threshold)
stateset pay --apply \
  --to 0xVendorA... \
  --amount 15000.00 \
  --chain ethereum \
  --agent treasury
```

## Compliance & Reporting

```bash
# Generate transaction report for accounting

stateset "list all Ethereum payments from last month" --json > eth_payments_jan.json

# Detailed transaction for audit
stateset pay --apply \
  --to 0xAuditedVendor... \
  --amount 75000.00 \
  --chain ethereum \
  --order PO-2024-AUDIT-001 \
  --customer vendor_id_12345 \
  --memo "Annual software license - Tax ID: 12-3456789"

# Export for tax reporting
stateset "export all 2024 stablecoin transactions for tax reporting"
```

## Gas Cost Considerations

```bash
# Ethereum mainnet has higher gas costs
# Best practices for cost optimization:

# 1. Check current gas prices
# https://etherscan.io/gastracker

# 2. Schedule non-urgent payments during low-gas periods
# (Weekends, early morning UTC)

# 3. For frequent small payments, consider L2s instead
# Use Base or Arbitrum for < $1000 transactions

# 4. Batch payments when possible (coming soon)
# stateset pay --batch payments.json --chain ethereum
```

## Transaction Fees

| Operation | Typical Fee | High Gas |
|-----------|-------------|----------|
| USDC Transfer | ~0.003 ETH (~$10) | ~0.01 ETH (~$30) |
| Confirmation Time | ~12 seconds | ~12 seconds |
| Safe Finality | ~2 minutes (12 blocks) | ~2 minutes |

## When to Use Ethereum vs L2

| Amount | Recommendation |
|--------|----------------|
| < $500 | Use Solana or Base |
| $500 - $10,000 | Use Base or Arbitrum |
| $10,000 - $100,000 | Ethereum or L2 based on urgency |
| > $100,000 | Ethereum mainnet (maximum security) |

## Enterprise Security

```bash
# Best practices for high-value transactions

# 1. Always simulate first
stateset pay --to 0x... --amount 50000 --chain ethereum

# 2. Verify recipient address independently
# Double-check via secure channel

# 3. Use --json for programmatic verification
stateset pay --apply \
  --to 0x... \
  --amount 50000 \
  --chain ethereum \
  --json | jq '{txHash, blockNumber, confirmations}'

# 4. Wait for sufficient confirmations
# 12+ blocks for amounts > $10,000
# 32+ blocks for amounts > $100,000

# 5. Keep audit trail
# All payments logged in VES with full metadata
```
