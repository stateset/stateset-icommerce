# `ICPEscrow` — Base USDC Settler contract

Solidity implementation of the on-chain side of the
`settler:circle.usdc.base` ICP Settler binding (see
`../../settlers/usdc-base.md`).

**Status:** ICP-1.0 reference contract. Designed for audit. Not yet deployed
to Base mainnet — the testnet bootstrap (`settler:stateset.usdc.base-sepolia`)
deploys this same source.

**Build status:** Compiles clean on Solc 0.8.24. Foundry test suite is
**15/15 PASS** covering fund/release, time-lock enforcement, dispute
state, arbiter restrictions (cannot redirect funds), pause behavior, and
escrow-ID collision. Reproduce with `forge install OpenZeppelin/openzeppelin-contracts foundry-rs/forge-std --no-git && forge test`.

## Audit-relevant properties

- **No upgrade path in this version.** Deliberate. Upgradeability adds
  attack surface that the v1 lifecycle doesn't need. If the contract needs
  to evolve, deploy a new instance with a new SettlerID and migrate; the
  spec's allowlist machinery handles cutover.
- **Reentrancy:** all state-changing functions use `nonReentrant`. State
  transitions occur before external transfers; balances are zeroed before
  transfer.
- **Access control:** OpenZeppelin AccessControl. ARBITER_ROLE and
  PAUSER_ROLE are distinct from DEFAULT_ADMIN_ROLE so an admin compromise
  doesn't immediately drain escrows.
- **Arbiter cannot redirect to third party:** `arbiterResolve` requires the
  beneficiary be either the recorded buyer or merchant. A compromised
  arbiter can favor a wrong party but cannot exfiltrate to an attacker.
- **Time-lock:** `release` requires `block.timestamp >= fulfillmentDeadline +
  disputeWindow`. No way to release earlier even if both parties agree —
  agreement is signaled off-chain by emitting a Settler-signed
  EscrowEvent, but the on-chain contract enforces the bound regardless.
- **No fee extraction:** the contract takes no fees. Settler fees, if any,
  are off-chain (operator-charged on the rail edge).
- **USDC pause-aware:** if Circle pauses USDC globally, `safeTransfer` will
  revert and escrows remain held until unpause. PAUSER_ROLE on this
  contract is for compliance pauses on the ICPEscrow side specifically.

## Build / test

Foundry-based. Install Foundry, then:

```sh
forge install OpenZeppelin/openzeppelin-contracts
forge install foundry-rs/forge-std
forge build
forge test -vvv
forge coverage
```

Expected: all tests pass, ~95%+ coverage on `src/ICPEscrow.sol`.

## Deploy (testnet — Base Sepolia)

For the StateSet-operated bootstrap Settler:

```sh
export BASE_SEPOLIA_RPC=https://sepolia.base.org
export DEPLOYER=0x...
export BASESCAN_KEY=...

forge script script/Deploy.s.sol --rpc-url $BASE_SEPOLIA_RPC \
  --broadcast --verify --etherscan-api-key $BASESCAN_KEY \
  --sig "deployTestnet()"
```

Records the deployed address; update `settlers/usdc-base.md` with the
testnet address under "Bootstrapping (StateSet-operated until Circle
adopts)".

## Deploy (mainnet — Base)

For the eventual `settler:circle.usdc.base` mainnet deployment — operated
by Circle, not StateSet:

```sh
export BASE_RPC=https://mainnet.base.org
export ADMIN_SAFE=0x...      # 5-of-9 Circle Safe with 48h timelock
export ARBITER_SAFE=0x...    # Foundation arbiter Safe
export PAUSER_SAFE=0x...     # Circle compliance Safe
export BASESCAN_KEY=...

forge script script/Deploy.s.sol --rpc-url $BASE_RPC \
  --broadcast --verify --etherscan-api-key $BASESCAN_KEY \
  --sig "deployMainnet()"
```

Pre-deploy checklist (`SETTLERS.md` §S.5 + `settlers/usdc-base.md`
"Production checklist"):
- Safes deployed and ownership-tested
- Discovery document staged at `https://api.circle.com/.well-known/icp-settler`
- Proof-of-reserves endpoint live with ≤24h refresh
- Settler signing service running with HSM-backed Ed25519 key
- WebSocket `observe` endpoint load-tested
- Two Foundation co-attestations (or, during bootstrap, StateSet
  attestation) on file

## Mapping events to ICP EscrowEvents

A Settler operator runs an off-chain indexer that subscribes to logs and
builds signed ICP EscrowEvents from them. One Solidity event maps to one
ICP EscrowEvent transition:

| Solidity event       | ICP EscrowEvent (`to_state` field) |
|----------------------|-----------------------------------|
| `EscrowFunded`       | `funded`                          |
| `EscrowDisputed`     | `disputed`                        |
| `EscrowReleased`     | `released` (terminal, emits SettlementReceipt) |
| `EscrowRefunded`     | `refunded` (terminal, emits SettlementReceipt) |
| `EscrowResolved`     | `released` or `refunded` based on beneficiary |

The Settler indexer SHOULD wait `finality.blocks_to_finality` confirmations
(per the discovery document — 18 on Base) before emitting the corresponding
ICP EscrowEvent, to avoid reorg-induced retractions.

## Why no `attestFulfillment` on-chain?

ICP-1.0's `funded → fulfilled` is an off-chain Settler attestation. We
considered making it an on-chain transition, but:

1. Fulfillment evidence (tracking number, delivery photo, API webhook) is
   inherently off-chain. Forcing an on-chain hash adds gas without adding
   verifiability.
2. The economic effect of fulfillment is encoded by the time-lock —
   release happens after the dispute window even without explicit
   attestation. The off-chain attestation is for human/agent visibility,
   not for state-machine progression.
3. Keeping the on-chain surface minimal reduces audit surface and gas.

If a future ICP version requires on-chain attestation (e.g. for regulated
high-value escrows), it can be added via a new `attestFulfillment` function
without changing existing semantics.
