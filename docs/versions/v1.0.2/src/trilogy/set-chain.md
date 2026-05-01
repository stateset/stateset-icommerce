# SET Chain L2

SET Chain (Chain ID: 84532001) is a commerce-optimized Ethereum Layer 2 network built on the OP Stack. It provides three enshrined primitives that general-purpose blockchains lack: verifiable event commitments, gas-sponsored merchant transactions, and a yield-bearing stablecoin for settlement.

## Why a Commerce-Specific L2?

| Problem | General-Purpose L2 | SET Chain |
|---------|-------------------|-----------|
| Gas friction | Users see gas costs; merchants have no control | Merchants sponsor via SetPaymaster |
| Verification gaps | Merchants rely on database assertions for audits | SetRegistry anchors Merkle commitments on-chain |
| Settlement mismatch | DeFi operates in volatile tokens | ssUSD: yield-bearing, T-Bill backed stablecoin |

## Chain Parameters

| Parameter | Value |
|-----------|-------|
| Chain ID | 84532001 |
| Block time | 2 seconds |
| Gas limit | 30M gas/block |
| Native token | ETH (standard gas token) |
| EIP-1559 denominator | 50 (stable, predictable fees) |
| Data availability | EIP-4844 Blob space (Ethereum-grade) |
| Settlement | Ethereum Sepolia L1 |
| Stack | OP Stack v1.8.0 |

## Three Enshrined Primitives

### 1. SetRegistry — On-Chain Commitment Storage

SetRegistry is the cryptographic anchor point for the entire StateSet commerce system. It stores Merkle commitments from the sequencer, enabling any third party to verify that a commerce event was included in a committed batch.

**Batch Commitment Structure:**

```solidity
struct BatchCommitment {
    bytes32 eventsRoot;      // Merkle root of event leaves
    bytes32 prevStateRoot;   // Previous batch's newStateRoot (chaining)
    bytes32 newStateRoot;    // This batch's state root
    uint64  sequenceStart;   // First sequence number
    uint64  sequenceEnd;     // Last sequence number
    uint32  eventCount;      // Number of events in batch
    uint256 timestamp;       // Block timestamp
    address submitter;       // Authorized sequencer address
}
```

**Key Functions:**

| Function | Gas | Description |
|----------|-----|-------------|
| `commitBatch()` | 60–80k | Submit Merkle root + metadata |
| `verifyInclusion()` | ~200 | Verify a Merkle proof against a batch |
| `getLatestStateRoot()` | ~200 | Get current state root for a stream |
| `setSequencerAuthorization()` | ~40k | Authorize/deauthorize sequencers |
| `setStrictMode()` | ~30k | Enable state chain continuity enforcement |

**Verification example:**

```javascript
// Any third party can verify an event was included
const isValid = await setRegistry.verifyInclusion(
    batchId,     // Which batch to check
    leafHash,    // Hash of the event
    merkleProof, // Proof path
    leafIndex    // Position in tree
);
// true → event was in this batch, cryptographically proven
```

**Strict Mode**: When enabled, each batch's `prevStateRoot` must match the previous batch's `newStateRoot`, preventing history forks.

### 2. SetPaymaster — Gas Abstraction

SetPaymaster is an ERC-4337 paymaster that sponsors gas for commerce transactions. AI agents and end users never need to hold ETH or manage gas tokens.

**Sponsorship Tiers:**

| Tier | Per-Tx Limit | Daily Limit | Monthly Limit |
|------|-------------|-------------|---------------|
| Starter | 0.001 ETH | 0.01 ETH | 0.1 ETH |
| Growth | 0.005 ETH | 0.05 ETH | 0.5 ETH |
| Enterprise | 0.01 ETH | 0.1 ETH | 1.0 ETH |

**Supported Operation Types:**

| Type | Description |
|------|-------------|
| `ORDER_CREATE` | New order placement |
| `ORDER_UPDATE` | Status transitions |
| `PAYMENT_PROCESS` | Payment execution |
| `INVENTORY_UPDATE` | Stock adjustments |
| `RETURN_PROCESS` | Return processing |
| `COMMITMENT_ANCHOR` | Batch anchoring |

Merchants deposit ETH into the paymaster and configure sponsorship rules. The paymaster validates each UserOperation against the merchant's tier limits and operation type whitelist before sponsoring gas.

### 3. ssUSD — Yield-Bearing Stablecoin

ssUSD is a yield-bearing stablecoin backed 1:1 by USDC deployed into short-duration U.S. Treasury Bills. See [ssUSD Stablecoin](ssusd.md) for full details.

## Gas Economics

| Operation | Gas | Cost (approx.) |
|-----------|-----|----------------|
| `commitBatch` (100 events) | 60–80k | ~$0.08 |
| Per-event anchoring cost | — | ~$0.0008 |
| `commitStarkProof` | 40–50k | ~$0.05 |
| `verifyInclusion` | ~200 | ~$0.0001 |

## Governance

**Current Stage**: Phase 0 (Single Sequencer)

```
Gnosis Safe Multisig (3-of-5)
         │
    24-hour delay
         │
         ▼
    SetTimelock
         │
    ┌────┴────┐
    ▼         ▼
SetRegistry  SetPaymaster
(UUPS)       (UUPS)
```

**Multisig Signers** (3-of-5): Lead developer, CTO, security lead, operations, advisor.

**Decentralization Roadmap:**

| Phase | Description |
|-------|-------------|
| **Phase 0** (current) | Single sequencer with explicit authorization |
| **Phase 1** | Backup sequencer key via `setSequencerAuthorization` |
| **Phase 2** | P2P enabled, L1 confirmations required |
| **Phase 3** | Permissionless participation with on-chain governance |

## MEV Protection

| Phase | Mechanism |
|-------|-----------|
| **Phase 1** (current) | Private sequencer with implicit ordering control |
| **Phase 2** (implemented) | Threshold encrypted mempool (DKG keepers) |
| **Phase 3** (planned) | Forced L1 inclusion for censorship resistance |
| **Phase 4** (future) | Shared sequencing (Espresso or similar) |

## Bridge (L1 ↔ L2)

**Deposits (L1 → L2)**: ~2–5 minutes via OP Stack bridge.

**Withdrawals (L2 → L1)**: 7-day challenge period (standard OP Stack security window), then finalize and claim.

## OP Stack Components

| Component | Purpose |
|-----------|---------|
| **op-geth** | Execution (EVM, Cancun-compatible) |
| **op-node** | Consensus and L1 derivation |
| **op-batcher** | L1 batch submission (EIP-4844 blobs) |
| **op-proposer** | State root submission to L1 |
| **op-challenger** | Dispute resolution |

## Smart Contracts

| Contract | Lines | Description |
|----------|-------|-------------|
| `SetRegistry.sol` | 433 | Merkle commitment anchoring (UUPS-upgradeable) |
| `SetPaymaster.sol` | 558 | ERC-4337 gas sponsorship |
| `SetTimelock.sol` | — | 24-hour governance delay |
| `wSSDCVaultV2.sol` | — | ERC-4626 yield-bearing vault |
| `NAVControllerV2.sol` | — | Net Asset Value oracle |
| `YieldEscrowV2.sol` | — | Invoice escrow with yield |

## Quick Start

### Local Development (Anvil)

```bash
cd /path/to/set
./scripts/dev.sh start         # Start local Anvil node
./scripts/dev.sh deploy        # Deploy contracts
./scripts/dev.sh test          # Run Foundry tests
./scripts/dev.sh anchor-start  # Start anchor service
```

### Full Devnet (OP Stack)

```bash
./scripts/install-op-stack.sh
./scripts/deploy-l1.sh
./scripts/generate-genesis.sh
./scripts/start-devnet.sh
```

### Interact with Contracts

```bash
# Check sequencer authorization
cast call $REGISTRY "authorizedSequencers(address)" $SEQUENCER

# Get latest state root for a stream
cast call $REGISTRY "getLatestStateRoot(bytes32,bytes32)" $TENANT $STORE

# Verify an event inclusion proof
cast call $REGISTRY "verifyInclusion(bytes32,bytes32,bytes32[],uint256)" \
    $BATCH_ID $LEAF $PROOF $INDEX
```

## Configuration

Key environment variables for the anchor service:

| Variable | Description |
|----------|-------------|
| `L2_RPC_URL` | SET Chain RPC endpoint |
| `SET_REGISTRY_ADDRESS` | SetRegistry contract address |
| `SEQUENCER_PRIVATE_KEY` | Authorized sequencer signing key |
| `L2_CHAIN_ID` | 84532001 |
| `SEQUENCER_API_URL` | Sequencer API for fetching pending commitments |
