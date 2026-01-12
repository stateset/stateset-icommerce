# Stablecoin Payment Examples

Native cryptocurrency payments for AI-powered commerce using the StateSet CLI.

## Quick Start

```bash
# 1. Check your agent wallet addresses
stateset pay --wallet

# 2. Fund your wallet on chosen chain (use faucet for testnet)

# 3. Check balance
stateset pay --balance --chain solana

# 4. Make a payment
stateset pay --apply --to <recipient> --amount 50.00 --chain solana
```

## Chain-Specific Examples

| Chain | Stablecoin | Use Case |
|-------|------------|----------|
| [Solana](./solana.md) | USDC | Fast B2C payments, high volume |
| [SET Chain](./set-chain.md) | ssUSD | Yield-bearing, StateSet native |
| [Base](./base.md) | USDC | Coinbase ecosystem, onramps |
| [Ethereum](./ethereum.md) | USDC | High-value B2B, maximum security |
| [Arbitrum](./arbitrum.md) | USDC | DeFi integration, low fees |

## Full Commerce Workflows

- [E-commerce Checkout](./workflows/checkout-flow.md) - Cart to payment
- [B2B Invoice Settlement](./workflows/b2b-invoices.md) - Invoice → stablecoin
- [Subscription Billing](./workflows/subscriptions.md) - Recurring payments
- [Refund Processing](./workflows/refunds.md) - Return → refund flow
- [Multi-Currency](./workflows/multi-currency.md) - Cross-border commerce

## Security Model

All payments follow the StateSet safety architecture:

1. **Simulation by default** - Preview what will happen
2. **Explicit opt-in** - Requires `--apply` to execute
3. **VES audit trail** - All transactions recorded
4. **Agent-controlled keys** - Derived from VES identity
