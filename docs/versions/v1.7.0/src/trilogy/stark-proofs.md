# STARK Compliance Proofs

StateSet STARK is a zero-knowledge proof system that enables privacy-preserving commerce compliance. Merchants and platforms can prove that a private transaction amount satisfies regulatory policies — without revealing the amount itself.

## Why Zero-Knowledge?

Commerce operates under conflicting requirements:

- **Privacy**: Transaction amounts, pricing agreements, and customer data must remain confidential between parties.
- **Compliance**: Regulators, auditors, and platforms need assurance that transactions satisfy AML thresholds, order caps, and authorization limits.

STARK proofs resolve this tension. A prover demonstrates that a secret amount satisfies a policy predicate, and a verifier confirms this without learning the amount.

## Properties

| Property | Description |
|----------|-------------|
| **No trusted setup** | Transparent — no ceremony required |
| **Post-quantum secure** | Built on hash functions and Reed-Solomon codes |
| **Succinct proofs** | 100–200 KB regardless of transaction complexity |
| **Batch-aggregatable** | Up to 128 compliance decisions in one proof |
| **Fast** | ~50ms proof generation, <10ms verification per event |

## Supported Policies

| Policy | ID | Constraint | Use Case |
|--------|-----|------------|----------|
| AML Threshold | `aml.threshold` | `amount < threshold` (strict) | Anti-money laundering |
| Order Total Cap | `order_total.cap` | `amount ≤ cap` (non-strict) | Order value limits |
| Agent Authorization | `agent.authorization.v1` | `amount ≤ maxTotal` + intent hash | Autonomous agent compliance |

## How It Works

### Per-Event Proofs

For each commerce event, the prover demonstrates:

```
There EXISTS a private amount (u64) such that:
  1. amount ≤ effective_limit           (policy inequality)
  2. witnessCommitment = Rescue(amount) (commitment binding)
  3. Public inputs are bound to proof   (boundary assertions)
```

The verifier learns only that the policy is satisfied, not the actual amount.

### Proof Flow

```
Agent (private amount)        STARK Prover              Sequencer
        │                          │                       │
        │─ 1. amount + policy ────►│                       │
        │                          │                       │
        │                    2. Build AIR trace            │
        │                       (248 columns × 128 rows)  │
        │                                                  │
        │                    3. Generate proof             │
        │                       (~50ms, 100-200 KB)       │
        │                                                  │
        │◄─ 4. proof + commitment─│                       │
        │                          │                       │
        │─ 5. Submit proof ───────────────────────────────►│
        │                          │                       │
        │                          │    6. Verify (<10ms)  │
        │                          │    Store in            │
        │                          │    ves_compliance_proofs│
```

### Public Inputs

Public inputs are canonicalized using RFC 8785 JSON Canonicalization:

```json
{
    "eventId": "uuid",
    "tenantId": "uuid",
    "storeId": "uuid",
    "sequenceNumber": 123,
    "payloadKind": 1,
    "payloadPlainHash": "hex64",
    "payloadCipherHash": "hex64",
    "eventSigningHash": "hex64",
    "policyId": "aml.threshold",
    "policyParams": { "threshold": 10000 },
    "policyHash": "hex64"
}
```

### Batch Proofs

For higher throughput, batch proofs aggregate 64–128 compliance events into a single proof:

- Enforce sequence continuity across events
- Ensure policy consistency within the batch
- Produce a Merkle tree of event leaves with a finalized state root
- The `new_state_root` is committable to SetRegistry on SET Chain

Batch proofs provide the same guarantees as per-event proofs but amortize proof generation cost across many events.

## Cryptographic Foundations

| Component | Implementation |
|-----------|---------------|
| **Field** | Goldilocks (64-bit prime: p = 2^64 - 2^32 + 1) |
| **Hash** | Rescue-Prime (STARK-friendly algebraic S-box, 7 rounds) |
| **Backend** | Winterfell v0.10 (FRI protocol, Fiat-Shamir) |
| **Security** | ~100-bit security target |

### Execution Trace

Each per-event proof operates on:
- **248 columns**: Rescue permutation state (12), amount limbs (8), threshold limbs (8), comparison intermediates (8), control flags (4), bit decomposition (64), public input binding (127)
- **128+ rows** (power of 2)

### Constraint System

- **157 transition constraints**: range checking (132), subtraction (4), Rescue permutation (12), witness binding (8)
- **80 boundary assertions**: trace framing, limb binding, Rescue output, public input pinning

## Integration

### With the Sequencer

The sequencer stores compliance proofs in two tables:

| Table | Scope | Content |
|-------|-------|---------|
| `ves_validity_proofs` | Batch-level | STARK proof over a sequence range |
| `ves_compliance_proofs` | Per-event | Encrypted proof for a specific event |

The sequencer provides canonical public inputs via `POST /api/v1/ves/compliance/{event_id}/inputs`, ensuring the prover and verifier agree on the statement being proved.

### With SET Chain

Batch proof state roots are submitted to SetRegistry, creating an on-chain, publicly auditable history of compliance decisions.

## Usage

### Rust

```rust
use ves_stark_prover::{ComplianceProver, ComplianceWitness, Policy};
use ves_stark_verifier::verify_compliance_proof_auto_bound_strict;

// Generate proof
let witness = ComplianceWitness::new(amount, public_inputs);
let prover = ComplianceProver::with_policy(Policy::aml_threshold(10000));
let proof = prover.prove(&witness)?;

// Verify proof
let result = verify_compliance_proof_auto_bound_strict(
    &proof.proof_bytes,
    &public_inputs
)?;
assert!(result.is_valid);
```

### Node.js

```javascript
const { prove, verifyHex, computePolicyHash } = require('@stateset/ves-stark');

// Generate proof
const proof = prove(5000n, publicInputs, 'aml.threshold', 10000n);

// Verify proof
const result = verifyHex(
    proof.proofBytes,
    publicInputs,
    proof.witnessCommitmentHex
);
console.log(result.valid); // true
```

### CLI

```bash
# Generate a compliance proof
ves-stark prove \
    --amount 5000 \
    --policy aml.threshold \
    --threshold 10000 \
    --public-inputs inputs.json \
    --output proof.bin

# Verify a proof
ves-stark verify \
    --proof proof.bin \
    --public-inputs inputs.json

# Submit to sequencer
ves-stark submit \
    --proof proof.bin \
    --event-id $EVENT_ID \
    --sequencer-url https://sequencer.stateset.com
```

## Security Model

**What the proof guarantees:**
- A private amount exists that satisfies the policy inequality
- The amount is bound to the public witness commitment via Rescue hash
- The amount is a valid u64 (range checked via bit decomposition)
- Policy parameters are pinned to the proof instance

**What the proof does NOT guarantee:**
- That the amount corresponds to the event's encrypted payload (this linkage is enforced at the protocol layer via trusted decryption)
- Encryption/decryption correctness (out of scope for the AIR)

## Crate Architecture

| Crate | Purpose |
|-------|---------|
| `ves-stark-primitives` | Field arithmetic, Rescue-Prime hash, public input canonicalization |
| `ves-stark-air` | Algebraic Intermediate Representation (constraint system) |
| `ves-stark-prover` | Per-event and batch proof generation |
| `ves-stark-verifier` | Stateless proof verification |
| `ves-stark-batch` | Batch state-transition proofs (64–128 events) |
| `ves-stark-client` | HTTP client for sequencer and SET Chain submission |
| `ves-stark-cli` | Command-line tool |
| `ves-stark-nodejs` | NAPI-RS bindings (`@stateset/ves-stark`) |
| `ves-stark-python` | PyO3 bindings |

## Roadmap

**Phase 1** (complete): Per-event and batch compliance proofs for AML threshold, order cap, and agent authorization policies.

**Phase 2** (planned): Commerce domain expansion — additional event types, multi-policy proofs, international compliance fields, fraud prevention attestation.

**Phase 3** (future): Recursive proof compression for on-chain verification, multi-region policy governance.
