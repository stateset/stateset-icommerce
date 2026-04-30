# Base USDC Quickstart

Get started with USDC payments on Base L2 — low fees, fast settlement, Coinbase ecosystem.

## Why Base?

- **Low fees**: Sub-cent transaction costs
- **USDC liquidity**: Native USDC issued by Circle
- **Coinbase ecosystem**: Easy on/off ramp
- **EVM compatible**: Standard Ethereum tooling

## Setup

### 1. Create an Agent Wallet

```bash
stateset --apply "create agent wallet on Base"
```

Or programmatically:

```javascript
const wallet = await toolkit.executeTool('get_agent_wallet', {
    chain: 'base',
    label: 'research-agent-wallet'
});
// → { address: '0x...', chain: 'base' }
```

### 2. Fund the Wallet

Transfer USDC to the agent's wallet address via Coinbase, Circle, or any Base-compatible wallet.

### 3. Check Balance

```bash
stateset "check my USDC balance on Base"
```

```javascript
const balance = await toolkit.executeTool('get_wallet_balance', {
    chain: 'base',
    currency: 'USDC'
});
// → { balance: '100.00', currency: 'USDC', chain: 'base' }
```

## Making Payments

### Preview (No Funds Moved)

```bash
stateset "preview paying data-agent 0.50 USDC for market data"
```

### Execute

```bash
stateset --apply "pay data-agent 0.50 USDC for market data"
```

### Programmatic

```javascript
const payment = await toolkit.executeTool('x402_create_payment_intent', {
    fromAgent: 'my-agent',
    toAgent: 'data-agent',
    amount: 0.50,
    currency: 'USDC',
    chain: 'base'
});
```

## Fee Structure

| Component | Cost |
|-----------|------|
| Base L2 gas | < $0.01 per transaction |
| x402 sequencer fee | ~0.1% of settlement amount |
| Bridge fee (Base → other chain) | ~0.05% |

Total cost for a $1.00 micro-payment: approximately $0.002 (0.2%).

## Failure Handling

| Scenario | Behavior |
|----------|----------|
| Insufficient USDC balance | `InsufficientBalanceError` — agent halts, no partial charge |
| Base network congestion | Circuit breaker opens, queues intents for retry |
| Bridge failure | Funds remain on source chain, retry automatically |
| Sequencer downtime | Fallback sequencer attempted, then offline queue |

All failures are surfaced as structured errors that LLMs can reason about.

## Upgrade Path

Start with Base for development and low-value transactions. When you need:

- **Higher throughput**: Stay on Base, increase budget limits
- **Multi-chain**: Add Arbitrum or Ethereum settlement
- **Native stablecoins**: Upgrade to Tier 3 with ssUSD on SET Chain
- **On-chain anchoring**: Upgrade to Tier 3 for Merkle root settlement

See [Product Tiers](../tiers.md) for the full upgrade path and [Stablecoins & Settlement](stablecoins.md) for multi-chain details.
