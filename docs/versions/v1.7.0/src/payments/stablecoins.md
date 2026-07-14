# Stablecoins & Settlement

iCommerce supports multiple stablecoin assets for agent payments and settlement.

## Supported Stablecoins

| Asset | Issuer | Chains | Description |
|-------|--------|--------|-------------|
| **USDC** | Circle | Base, Ethereum, Arbitrum, Solana | Most widely adopted, native on Base |
| **USDT** | Tether | Ethereum, Arbitrum | Legacy stablecoin |
| **ssUSD** | StateSet | SET Chain | Yield-bearing stablecoin (Tier 3) |
| **DAI** | MakerDAO | Ethereum | Decentralized, crypto-collateralized |

## Settlement Process

Payment intents created via the x402 protocol accumulate off-chain and are batch-settled on a periodic schedule:

```
Off-chain intent → Sequencer queue → Batch transaction → On-chain settlement
```

### Settlement States

| State | Description |
|-------|-------------|
| `unsigned` | Intent created but not signed |
| `signed` | Ed25519 signature attached |
| `pending` | Submitted to sequencer for settlement |
| `settled` | Confirmed on-chain |

## ssUSD (Tier 3)

ssUSD is a yield-bearing stablecoin native to SET Chain. Backed 1:1 by USDC deployed into short-duration U.S. Treasury Bills, ssUSD generates ~5% APY for holders. Funds held in ssUSD earn yield automatically, making escrow and reserve holdings productive.

ssUSD uses a dual-token architecture:
- **ssUSD** (rebasing) — balance auto-increases daily, ideal for payments and holding
- **wssUSD** (ERC-4626 wrapped) — share price accrues, ideal for DeFi integration

For the complete technical specification including NAV mechanics, contract architecture, yield escrow, and safety mechanisms, see **[ssUSD Stablecoin](../trilogy/ssusd.md)**.

**Requirements:**
- Tier 3 configuration (sync.json + chain RPC)
- SET Chain wallet

```javascript
// Mint ssUSD
await toolkit.executeTool('mint_ssusd', {
    amount: 1000.00,
    fromAsset: 'USDC',
    chain: 'base'
});

// Check ssUSD balance (includes accrued yield)
const balance = await toolkit.executeTool('get_ssusd_balance', {
    agentId: 'my-agent'
});
// → { balance: 1002.50, principal: 1000.00, yield: 2.50 }
```

## Cross-Chain Settlement

For multi-chain operations, the bridge handles asset transfers:

```javascript
await toolkit.executeTool('bridge_usdc', {
    fromChain: 'base',
    toChain: 'set',
    amount: 500.00
});
```

## Settlement Finality Tracking

Each chain has different confirmation requirements before a settlement is considered final:

| Chain | Confirmations | Time to Finality | Reorg Risk |
|-------|---------------|-------------------|------------|
| SET Chain | 1 | ~2 seconds | Negligible |
| Base | 2 | ~4 seconds | Very low |
| Arbitrum | 1 | ~1 second | Very low |
| Solana | 32 slots | ~13 seconds | Low |
| Ethereum | 12 | ~3 minutes | Low |

### Finality States

```
broadcast → unconfirmed → confirming → final → settled
                                      → reorged (block reorganization detected)
                                      → failed
```

The settlement finality tracker monitors confirmation counts and automatically updates intent status. Block reorganizations are detected and flagged.

```javascript
// Track a settlement
await toolkit.executeTool('x402_track_settlement', {
    intentId: 'intent-123',
    txHash: '0xabc...',
    chain: 'base'
});

// Check finality
const status = await toolkit.executeTool('x402_check_finality', {
    intentId: 'intent-123'
});
// → { isFinal: true, state: 'final', confirmations: 3, chain: 'base' }
```

## Agent Wallet Derivation

Agent wallets are derived from VES signing keys. Each agent gets a unique wallet per chain:

```javascript
const wallet = await toolkit.executeTool('get_agent_wallet', {
    agentId: 'my-agent',
    chain: 'base'
});
// → { address: '0x...', chain: 'base', derivationPath: 'm/44/8004/0/0' }
```

## Multi-Chain Balance

```javascript
const balance = await toolkit.executeTool('treasury_balance', {
    agentId: 'my-agent'
});
// → {
//     base: { USDC: 1000.00 },
//     set_chain: { ssUSD: 500.00, USDC: 200.00 },
//     ethereum: { USDC: 0.00, USDT: 50.00 }
// }
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `get_agent_wallet` | Get agent wallet address for a chain |
| `get_wallet_balance` | Check stablecoin balance by chain/token |
| `send_stablecoin` | Transfer between wallets |
| `swap_stablecoin` | Convert USDC ↔ ssUSD |
| `estimate_gas_fee` | Estimate transaction cost |
| `treasury_balance` | Multi-chain balance overview |
| `treasury_fund_agent` | Distribute funds to sub-agents |
| `list_settlement_history` | View settlement records |
| `x402_settle_intent` | Trigger manual settlement |
| `x402_track_settlement` | Track settlement confirmations |
| `x402_check_finality` | Check if settlement is final |
| `mint_ssusd` | Mint ssUSD from USDC (Tier 3) |
| `get_ssusd_balance` | Check ssUSD with accrued yield |
| `bridge_usdc` | Transfer USDC across chains |
