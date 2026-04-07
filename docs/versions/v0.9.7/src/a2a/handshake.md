# Handshake Protocol

The handshake protocol enables agents to negotiate capabilities before transacting. It prevents failures from protocol mismatches by exchanging and comparing capability manifests before any payment or data exchange occurs.

## Why Handshake?

Without a handshake, an agent might:
- Attempt payment on a network the counterparty doesn't support
- Try to create an escrow when the counterparty has no escrow capability
- Negotiate in an asset the counterparty can't accept
- Hit a transaction limit the counterparty enforces

The handshake resolves all of this in a single round-trip.

## Capability Manifest

Each agent declares its capabilities:

```javascript
const hs = createHandshakeService({
    agentId: 'agent-seller-01',
    supportedNetworks: ['set_chain', 'base', 'ethereum'],
    supportedAssets: ['USDC', 'USDT', 'ssUSD'],
    features: {
        escrow: true,
        subscriptions: true,
        splits: false,
        sagas: true,
        sse: true,              // Server-Sent Events
    },
    maxTransactionAmount: 50000,
    preferredFinality: 'final',
    webhookEndpoint: 'https://seller.example/hooks',
    publicKey: '0xABCD...',
});
```

## Initiating a Handshake

```javascript
// Get counterparty's capabilities (from their Agent Card or direct exchange)
const theirCapabilities = {
    agentId: 'agent-buyer-01',
    supportedNetworks: ['base', 'arbitrum'],
    supportedAssets: ['USDC', 'DAI'],
    features: { escrow: true, subscriptions: false, splits: true, sagas: false, sse: false },
    maxTransactionAmount: 10000,
    publicKey: '0x1234...',
};

const result = hs.initiateHandshake(theirCapabilities);
// → {
//     compatible: true,
//     bestNetwork: 'base',           // Highest priority shared network
//     bestAsset: 'USDC',             // Highest priority shared asset
//     sharedNetworks: ['base'],
//     sharedAssets: ['USDC'],
//     sharedFeatures: { escrow: true, subscriptions: false, splits: false, sagas: false, sse: false },
//     effectiveMaxAmount: 10000,     // min(50000, 10000)
//     warnings: [
//         'Counterparty does not support subscriptions',
//         'Counterparty does not support SSE event streaming'
//     ]
// }
```

## Priority Resolution

When multiple networks or assets overlap, the handshake picks the best option by priority:

**Network priority** (highest first):

| Priority | Network |
|----------|---------|
| 1 | `set_chain` |
| 2 | `base` |
| 3 | `arbitrum` |
| 4 | `solana` |
| 5 | `ethereum` |

**Asset priority** (highest first):

| Priority | Asset |
|----------|-------|
| 1 | `USDC` |
| 2 | `USDT` |
| 3 | `ssUSD` |
| 4 | `DAI` |

## Compatibility Check

For quick validation without a full handshake:

```javascript
const compat = hs.checkCompatibility(theirCapabilities);
// → {
//     networkCompatible: true,
//     assetCompatible: true,
//     featureCompatible: true,    // all required features overlap
//     amountCompatible: true,     // both sides' limits allow the transaction
//     overallCompatible: true,
//     issues: []
// }
```

When incompatible:

```javascript
// → {
//     networkCompatible: false,
//     overallCompatible: false,
//     issues: ['No shared networks: we support [set_chain], they support [solana]']
// }
```

## Responding to a Handshake

When another agent initiates a handshake with you:

```javascript
const response = hs.respondToHandshake(incomingHandshake);
// → {
//     accepted: true,
//     selectedNetwork: 'base',
//     selectedAsset: 'USDC',
//     effectiveMaxAmount: 10000,
//     myCapabilities: { ... },  // Full capability manifest
// }
```

## Feature Flags

The handshake negotiates 5 feature flags:

| Feature | Description |
|---------|-------------|
| `escrow` | Conditional escrow support |
| `subscriptions` | Recurring A2A billing |
| `splits` | Multi-party payment distribution |
| `sagas` | Multi-step saga orchestration |
| `sse` | Server-Sent Events for real-time updates |

Both agents must support a feature for it to be available in the session.

## Effective Transaction Limits

The handshake computes the effective maximum transaction amount as the minimum of both agents' limits:

```
effectiveMaxAmount = min(myMaxAmount, theirMaxAmount)
```

Transactions above this limit will be rejected by the lower-capacity agent.

## Protocol Version Check

The handshake verifies protocol version compatibility. Version mismatches are reported as warnings (minor version differences) or errors (major version differences).

## Integration with A2A Protocol

The handshake is the first step in any A2A interaction:

```
1. Agent A discovers Agent B via Agent Card
2. Agent A initiates handshake with B's capabilities
3. Handshake resolves: best network, asset, shared features
4. Agent A sends quote request on the resolved network/asset
5. Negotiation, escrow, payment proceed using agreed-upon parameters
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `a2a_handshake_initiate` | Initiate handshake with counterparty capabilities |
| `a2a_handshake_respond` | Respond to an incoming handshake |
| `a2a_handshake_check` | Quick compatibility check |
| `a2a_handshake_capabilities` | Get this agent's capability manifest |
