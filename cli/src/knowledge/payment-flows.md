# Payment Flows

## Payment Methods

StateSet supports multiple payment methods across traditional and crypto rails:

### Traditional
- **Credit/debit card** — Visa, Mastercard, Amex; captured via payment processor
- **Invoice (net terms)** — B2B net-30/net-60 invoices with manual payment recording
- **Store credit** — Balance held in customer account

### Stablecoin (Native Crypto)
- **USDC on Solana** — Fast (< 1s), very low fees, deep liquidity
- **USDC on Base** — Coinbase L2, low fees, EVM-compatible
- **USDC on Ethereum** — Maximum security, higher gas fees
- **ssUSD on SET Chain** — StateSet native yield-bearing stablecoin on L2
- **USDC on Arbitrum** — Fast, cheap EVM L2

### x402 Protocol (AI Agent Commerce)
- Off-chain payment intents signed with Ed25519 keys
- Enables AI agents to authorize payments without browser UI
- Supports programmable escrow and conditional release

## Payment Lifecycle

```
pending → authorized → captured → settled
                    ↘ failed
        ↘ cancelled (before authorization)
```

- **pending** — Payment intent created; awaiting customer action or authorization
- **authorized** — Funds reserved on customer's account; not yet collected
- **captured** — Funds collected from customer; held by payment processor
- **settled** — Funds transferred to merchant bank account
- **failed** — Payment declined or errored
- **cancelled** — Payment voided before capture

## Payment Operations

- `create_payment` — Create a payment record for an order
- `complete_payment` — Mark payment as captured/completed
- `create_refund` — Initiate a full or partial refund

## Refund Processing

### Full Refund
- Entire order amount returned to original payment method
- Triggered by order cancellation or return approval
- Stablecoin refunds execute on-chain immediately

### Partial Refund
- Specific line items or a fixed dollar amount
- Used when only some returned items are approved
- Multiple partial refunds can be issued against one payment

### Refund Timeline
- Card refunds: 3-10 business days (card network dependent)
- Stablecoin refunds: near-instant (1-3 block confirmations)
- Store credit: immediate

## Multi-Currency Support

- **Base currency** — The merchant's primary accounting currency (e.g. USD)
- **Enabled currencies** — Additional currencies customers can pay in
- **Exchange rates** — Set manually or fetched from market; stored per currency pair
- `convert_currency` — Convert amounts between enabled currencies
- `get_exchange_rate` — Current rate between two currencies
- `format_currency` — Format a number with currency symbol and locale rules

## x402 Protocol Details

The x402 protocol enables AI agent-to-agent payments:

1. Buyer agent calls `x402_create_payment_intent` with amount, currency, seller, chain
2. Buyer agent signs intent with Ed25519 private key (`x402_sign_intent`)
3. Intent submitted to blockchain; settlement confirmed on-chain
4. `x402_mark_settled` records the transaction hash and block number
5. Nonce incremented to prevent replay attacks (`x402_get_next_nonce`)
