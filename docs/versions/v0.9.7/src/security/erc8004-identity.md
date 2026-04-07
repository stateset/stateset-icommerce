# ERC-8004 Agent Identity

ERC-8004 is a standard for self-sovereign AI agent identity on Ethereum-compatible chains. It binds an agent's identity to a wallet address with cryptographic proof, enabling cross-chain agent discovery and payment verification without centralized registries.

## Why ERC-8004?

Traditional agent registries are centralized — a single database maps agent IDs to capabilities. ERC-8004 decentralizes this:

- **Self-sovereign**: Agents own their identity records, not a platform
- **Verifiable**: Wallet binding is cryptographically proven via EIP-712 or ERC-1271
- **Portable**: Identity works across any EVM chain
- **Composable**: Agent Cards reference the ERC-8004 record for trust verification

## Identity Record

An ERC-8004 identity record contains:

| Field | Description |
|-------|-------------|
| `registry` | Agent registry URI (e.g., `https://registry.stateset.zone`) |
| `agentId` | Unique agent identifier |
| `agentUri` | Agent's endpoint URI |
| `agentWallet` | Ethereum wallet address |
| `ownerAddress` | Owner who controls the identity |
| `agentCardId` | Link to the agent's A2A Agent Card |
| `walletProofType` | `eip712` (EOA) or `erc1271` (smart contract wallet) |
| `walletProof` | Cryptographic proof binding wallet to identity |
| `active` | Whether the identity is active |

## Registering an Identity

```javascript
await toolkit.executeTool('erc8004_register_identity', {
    registry: 'https://registry.stateset.zone',
    agentId: 'fulfillment-agent-01',
    agentUri: 'https://agent.example.com/a2a',
    agentWallet: '0x1234...abcd',
    ownerAddress: '0xOwner...1234',
    walletProofType: 'eip712',
    walletProof: '0xSignature...',
    walletProofChainId: 84532001,
    walletProofDeadline: '2027-03-17T00:00:00Z',
    active: true,
});
```

## Wallet Proof Types

### EIP-712 (EOA Wallets)

For externally owned accounts, the agent signs a typed data message:

```
Domain: { name: "ERC8004", chainId: 84532001, verifyingContract: registryAddress }
Types:  { AgentIdentity: [agentId, agentUri, wallet, deadline] }
```

### ERC-1271 (Smart Contract Wallets)

For smart contract wallets (Gnosis Safe, ERC-4337 accounts), the wallet contract's `isValidSignature()` method is called to verify the proof.

## Linking a Wallet

Add or update a wallet binding on an existing identity:

```javascript
await toolkit.executeTool('erc8004_link_wallet', {
    registry: 'https://registry.stateset.zone',
    agentId: 'fulfillment-agent-01',
    agentWallet: '0xNewWallet...5678',
    walletProofType: 'eip712',
    walletProof: '0xNewSignature...',
    walletProofChainId: 84532001,
});
```

## Looking Up an Identity

```javascript
const identity = await toolkit.executeTool('erc8004_lookup_identity', {
    registry: 'https://registry.stateset.zone',
    agentId: 'fulfillment-agent-01',
});
// → {
//     agentId: 'fulfillment-agent-01',
//     agentUri: 'https://agent.example.com/a2a',
//     agentWallet: '0x1234...abcd',
//     ownerAddress: '0xOwner...1234',
//     walletProofType: 'eip712',
//     active: true,
//     registeredAt: '2026-03-17T10:30:45Z',
// }
```

## Integration with A2A Protocol

ERC-8004 identity is the foundation of the A2A trust chain:

```
1. Agent registers ERC-8004 identity       → wallet + proof on-chain
2. Agent creates A2A Agent Card             → references ERC-8004 record
3. Counterparty discovers Agent Card        → verifies wallet proof
4. Handshake negotiates capabilities        → confirms identity
5. Payment signed with verified wallet      → non-repudiable
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `erc8004_register_identity` | Register or update an agent identity |
| `erc8004_link_wallet` | Link a wallet to an identity |
| `erc8004_lookup_identity` | Look up an identity by agent ID |
| `erc8004_list_identities` | List identities in a registry |
