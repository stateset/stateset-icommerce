# ssUSD Stablecoin

ssUSD is a yield-bearing stablecoin native to SET Chain. It is backed 1:1 by USDC deployed into short-duration U.S. Treasury Bills, generating approximately 5% APY for holders. ssUSD is the default settlement asset for autonomous agent commerce.

## Why ssUSD?

Commerce operates in dollars. DeFi operates in volatile tokens. ssUSD bridges this gap:

- **Merchants** receive dollar-denominated payments without volatile token exposure
- **Agents** hold working capital that earns yield automatically
- **Escrow** funds generate yield while held in conditional release
- **Settlement** happens in a stable, auditable asset

## Dual-Token Architecture

| Token | Type | Mechanism | Best For |
|-------|------|-----------|----------|
| **ssUSD** | Rebasing | Balance auto-increases daily | Payments, holding, transfers |
| **wssUSD** (wrapped) | Non-rebasing (ERC-4626) | Share price accrues | DeFi, AMMs, lending protocols |

### ssUSD (Rebasing)

Your ssUSD balance increases automatically as yield accrues:

```
Day 0:  1,000.00 ssUSD (1,000 shares @ $1.00 NAV)
Day 30: 1,004.11 ssUSD (1,000 shares @ $1.00411 NAV, ~5% APY)
Day 90: 1,012.50 ssUSD
Day 365: 1,050.00 ssUSD
```

No manual claiming. No staking. Just hold ssUSD and earn.

### wssUSD (Wrapped, ERC-4626)

For DeFi compatibility, wssUSD is a non-rebasing vault token:

```
Day 0:  Deposit 1,000 ssUSD → receive ~1,000 wssUSD shares
Day 30: 1,000 wssUSD shares now worth 1,004.11 ssUSD
        (share price increased, share count unchanged)
```

wssUSD is compatible with AMMs, lending protocols, and any ERC-4626 integration.

## Yield Mechanism

```
USDC deposits
    │
    ▼
Treasury Reserve Manager
    │
    ├──► Short-duration U.S. Treasury Bills (~5.20% gross)
    │
    ├──► Yield accrues off-chain
    │
    ▼
NAV Oracle attests daily
    │
    ▼
ssUSD balances increase (rebasing)
    │
    └──► Protocol keeps ~0.20% spread
         Net to holders: ~5.00% APY
```

### NAV (Net Asset Value) Controller

The NAV Controller uses linear projection between oracle attestations:

1. Authorized attestor submits current NAV daily
2. Between attestations, NAV is projected linearly (constant rate)
3. On new attestation, NAV snaps to the attested value
4. Safety bounds: `maxNavJumpBps` limits per-update changes, `minNavRay` prevents NAV collapse

NAV uses ray-precision arithmetic (1e27) for accurate share/asset conversions:

```
convertToAssets = floor(shares × navRay / RAY)
convertToShares = floor(assets × RAY / navRay)
```

## Minting and Redemption

### Mint ssUSD

```javascript
await toolkit.executeTool('mint_ssusd', {
    amount: 1000.00,
    fromAsset: 'USDC',
    chain: 'base'
});
```

Flow: USDC deposited → ssUSD minted 1:1 → Reserve manager deploys to T-Bills.

### Redeem ssUSD

```javascript
await toolkit.executeTool('redeem_ssusd', {
    amount: 500.00,
    toAsset: 'USDC',
    chain: 'base'
});
```

Redemption follows a T+1 delay to protect against bank-run scenarios. Users can cancel pending redemptions. The claim queue processes redemptions in FIFO order.

### Check Balance (with yield)

```javascript
const balance = await toolkit.executeTool('get_ssusd_balance', {
    agentId: 'my-agent'
});
// → { balance: 1002.50, principal: 1000.00, yield: 2.50 }
```

## Safety Mechanisms

| Mechanism | Protection |
|-----------|-----------|
| **NAV staleness check** | If NAV not updated within 24 hours, deposits restricted (redemptions always work) |
| **Independent pause controls** | Deposits and redemptions can be paused independently |
| **Full collateralization** | Every ssUSD backed 1:1 by USDC or T-Bills |
| **Redemption delay** | T+1 delay prevents bank-run scenarios |
| **Max NAV jump** | `maxNavJumpBps` bounds per-update NAV changes |
| **Min NAV floor** | `minNavRay` prevents NAV from going to zero |
| **Circuit breaker** | Emergency halt via SSDCCircuitBreakerV2 |

## Contract Architecture

| Contract | Purpose |
|----------|---------|
| `wSSDCVaultV2` | ERC-4626 share vault (core) |
| `NAVControllerV2` | Linear NAV projection between attestations |
| `YieldEscrowV2` | Invoice escrow that earns yield while held |
| `SSDCClaimQueueV2` | Async redemption queue (FIFO, T+1) |
| `SSDCPolicyModuleV2` | KYC/AML gating for mint/redeem |
| `SSDCGroundingRegistryV2` | Grounding attestations for off-chain reserves |
| `YieldPaymasterV2` | Gas sponsorship funded by yield |
| `WSSDCCrossChainBridgeV2` | Cross-chain asset transfer |
| `SSDCGatewayV2` | Unified deposit/withdrawal gateway |
| `SSDCStatusLensV2` | Read-only aggregator for vault state |
| `SSDCCircuitBreakerV2` | Emergency circuit breaker |

## Yield Escrow

YieldEscrowV2 is purpose-built for commerce: escrow funds earn yield while held in conditional release.

```javascript
// Create escrow that earns yield
await toolkit.executeTool('a2a_create_escrow', {
    amount: 5000.00,
    currency: 'ssUSD',
    releaseConditions: [
        { type: 'delivery_confirmed', trackingId: 'TRACK-123' }
    ]
});
// Funds earn ~5% APY while held in escrow
// Yield accrues to the escrow, distributed on release
```

## Cross-Chain Bridging

ssUSD can be bridged across chains:

```javascript
await toolkit.executeTool('bridge_usdc', {
    fromChain: 'base',
    toChain: 'set',
    amount: 500.00
});
```

The WSSDCCrossChainBridgeV2 handles cross-chain transfers with message verification.

## Revenue Model

At scale, the protocol generates revenue from the yield spread:

| Metric | Value |
|--------|-------|
| Gross T-Bill yield | ~5.20% APY |
| Net to ssUSD holders | ~5.00% APY |
| Protocol spread | ~0.20% of yield |
| Revenue at $100M TVL | ~$200k/year |

## MCP Tools

| Tool | Description |
|------|-------------|
| `mint_ssusd` | Mint ssUSD from USDC |
| `redeem_ssusd` | Redeem ssUSD for USDC |
| `get_ssusd_balance` | Check balance with accrued yield |
| `swap_stablecoin` | Convert USDC ↔ ssUSD |
| `bridge_usdc` | Transfer across chains |
