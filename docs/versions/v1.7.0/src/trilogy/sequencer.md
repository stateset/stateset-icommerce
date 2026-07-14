# StateSet Sequencer

The StateSet Sequencer is a Verifiable Event Sync (VES) v1.0 service that provides deterministic event ordering, cryptographic commitments, and zero-knowledge compliance proofs for distributed commerce systems. It is the "central truth clock" that bridges AI agents with cryptographically verifiable infrastructure.

## The Problem

When multiple autonomous agents operate on the same commerce data, three critical issues arise:

1. **Ordering ambiguity** — When two agents modify inventory concurrently, which modification wins? Wall-clock timestamps are unreliable across distributed systems.
2. **Auditability gaps** — Traditional databases allow silent reordering, dropping, and duplication. Operators must trust the database administrator.
3. **Forgery risk** — Without cryptographic attribution, a compromised agent can forge events on behalf of another.

The sequencer solves all three by providing a canonical, gap-free, cryptographically signed event log.

## Architecture

```
┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│   Agent A   │   │   Agent B   │   │   Agent C   │
│  (Ed25519)  │   │  (Ed25519)  │   │  (Ed25519)  │
└──────┬──────┘   └──────┬──────┘   └──────┬──────┘
       │ VES events      │                  │
       └─────────────────┼──────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                  StateSet Sequencer                   │
│                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐│
│  │  Sequencing   │  │  Commitment  │  │  Agent Key ││
│  │  Engine       │  │  Engine      │  │  Registry  ││
│  │  (gap-free)   │  │  (Merkle)    │  │  (Ed25519) ││
│  └──────┬───────┘  └──────┬───────┘  └────────────┘│
│         │                  │                         │
│  ┌──────┴──────────────────┴───────────────────────┐│
│  │              PostgreSQL                          ││
│  │  ves_events · batch_commitments · agent_keys    ││
│  │  x402_payment_intents · x402_payment_batches    ││
│  │  ves_validity_proofs · ves_compliance_proofs    ││
│  └──────────────────────────────────────────────────┘│
└──────────────────────┬──────────────────────────────┘
                       │ Merkle roots
                       ▼
              ┌────────────────┐
              │  SET Chain L2  │
              │  (SetRegistry) │
              └────────────────┘
```

## VES Event Envelope

Every event submitted to the sequencer follows the VES v1.0 envelope format:

| Field | Type | Description |
|-------|------|-------------|
| `ves_version` | `1` | Protocol version |
| `event_id` | UUID | Globally unique event identifier |
| `tenant_id` | UUID | Tenant isolation |
| `store_id` | UUID | Store within tenant |
| `source_agent_id` | string | Signing agent identity |
| `entity_type` | string | Commerce domain (e.g., `order`, `inventory`) |
| `entity_id` | string | Entity identifier (e.g., `ORD-001`) |
| `event_type` | string | Domain event (e.g., `order.created`) |
| `payload` | JSON | Event data (plaintext) |
| `payload_encrypted` | bytes | Event data (AES-256-GCM, optional) |
| `payload_plain_hash` | hex | `SHA-256("VES_PAYLOAD_PLAIN_V1" \|\| JCS(payload))` |
| `agent_signature` | hex | `Ed25519(event_signing_hash)` |
| `sequence_number` | u64 | **Sequencer-assigned** canonical position |
| `sequenced_at` | timestamp | Sequencer acceptance time |

## Key Guarantees

### Gap-Free Ordering

For each stream (tenant + store), sequence numbers strictly increase by 1 with no gaps:

```
stream_1: [1, 2, 3, 4, 5, ...]     ← valid
stream_1: [1, 2, 3, 5, ...]         ← INVALID (gap at 4)
```

Only the `sequence_number` is authoritative for ordering. The `created_at` timestamp is an agent claim (signed but never trusted for ordering). The `sequenced_at` timestamp is a sequencer observation (useful for monitoring but non-binding).

Enforcement: PostgreSQL `SELECT FOR UPDATE` ensures linearizable sequencing per stream.

### Exactly-Once Delivery

Events are idempotent by `event_id`. Resubmitting an identical event returns the previously assigned sequence number. Resubmitting with different content is rejected.

### Commitment Chaining (Fork Prevention)

Each batch commitment references the previous batch, creating a state chain:

```
Batch 1: events [1..100]
  events_root     = Merkle(leaves[1..100])
  prev_state_root = 0x0 (genesis)
  new_state_root  = events_root

Batch 2: events [101..200]
  events_root     = Merkle(leaves[101..200])
  prev_state_root = Batch 1 new_state_root   ← must match
  new_state_root  = events_root

Batch 3: events [201..300]
  events_root     = Merkle(leaves[201..300])
  prev_state_root = Batch 2 new_state_root   ← must match
  ...
```

When strict mode is enabled on the SetRegistry contract, each new commitment is validated against the on-chain state root, preventing history forks.

### Finality Model

| Level | Latency | Guarantee |
|-------|---------|-----------|
| **Soft finality** | Milliseconds | Sequencer receipt confirms acceptance (non-repudiable) |
| **Hard finality** | Minutes | Batch anchored on-chain via SetRegistry (independently verifiable) |

## Agent Key Management

Agents register Ed25519 public keys indexed by `(tenant_id, agent_id, key_id)`:

```
agent_signing_keys {
  tenant_id:  UUID
  agent_id:   string
  key_id:     integer  (increments on rotation)
  public_key: Ed25519 bytes
  valid_from: unix_seconds
  valid_until: unix_seconds
  status:     active | revoked | expired
}
```

**Rotation**: Agents increment `key_id` and register a new key. The old key remains valid for historical verification within its validity window.

**Revocation**: Keys can be revoked; the sequencer evaluates revocation against the `sequenced_at` timestamp.

### CLI Key Management

```bash
# Generate agent signing keys
stateset-sync keys:generate

# Register public key with sequencer
stateset-sync keys:register

# Rotate keys (old key remains valid for verification)
stateset-sync keys:rotate --all --register
```

## API Surface

### REST Endpoints (Axum)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/ves/events/ingest` | POST | Accept signed event envelopes |
| `/api/v1/ves/commitments` | GET | Fetch batch commitments with Merkle proofs |
| `/api/v1/ves/commitments` | POST | Create new batch commitment |
| `/api/v1/ves/proofs/:sequence_number` | GET | Inclusion proof for a specific event |
| `/api/v1/ves/proofs` | POST | Submit STARK compliance proofs |
| `/api/v1/ves/compliance/:event_id/inputs` | POST | Canonical public inputs for STARK prover |
| `/api/v1/agent-keys` | POST | Register agent Ed25519 public key |
| `/api/v1/agent-keys` | GET | List registered agent keys |
| `/api/v1/schemas` | POST | Store event schemas |
| `/health` | GET | Basic health check |
| `/ready` | GET | Database connectivity check |
| `/metrics` | GET | Prometheus metrics |

### gRPC Services

| Service | Description |
|---------|-------------|
| `SequencerService` | Event sequencing (streaming) |
| `SequencerServiceV2` | Enhanced streaming with auth |
| `KeyManagementServiceV2` | Agent key management |

## End-to-End Encryption (VES-ENC-1)

For privacy-sensitive events (payment details, customer PII, pricing agreements):

```
1. Generate ephemeral X25519 key pair
2. ECDH: shared_secret = X25519(ephemeral_private, recipient_public)
3. HKDF: encryption_key = HKDF-SHA256(shared_secret, salt, "VES-ENC-1")
4. Encrypt: AES-256-GCM(encryption_key, nonce, plaintext, AAD)
5. Bundle: { ephemeral_public, nonce, ciphertext, tag }
6. Zeroize: ephemeral_private, shared_secret, encryption_key
```

The sequencer is **sequencer-blind** — it sequences encrypted events without accessing plaintext. Both `payload_plain_hash` and `payload_cipher_hash` are included in the agent signature, binding the encrypted and plaintext representations together.

## x402 Payment Processing

The sequencer also processes x402 payment intents:

| Table | Purpose |
|-------|---------|
| `x402_payment_intents` | Individual signed payment requests |
| `x402_payment_batches` | Batched payments for gas-efficient settlement |

Payment intents progress through: `pending` → `sequenced` → `submitted` → `settled`

Batch settlement compresses 100–1,000 payments into a single on-chain transaction, reducing per-payment gas costs to fractions of a cent.

## Schema Validation

The sequencer supports per-tenant JSON Schema validation:

| Mode | Behavior |
|------|----------|
| `disabled` | No validation (default) |
| `warn` | Validate, log warnings, accept anyway |
| `strict` | Validate, reject non-conforming events |

## Observability

- **Prometheus metrics**: Event ingestion rate, sequencing latency, commitment generation time
- **OpenTelemetry**: Distributed tracing with OTLP export
- **Structured logging**: JSON format with configurable redaction
- **Health endpoints**: `/health` (basic), `/ready` (DB connectivity)

## Configuration

Key environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | PostgreSQL connection string |
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | HTTP port |
| `AUTH_MODE` | `api_key` | Authentication mode |
| `RATE_LIMIT_PER_MINUTE` | `1000` | Per-tenant rate limit |
| `PAYLOAD_ENCRYPTION_MODE` | `disabled` | Encryption mode |
| `L2_RPC_URL` | — | SET Chain RPC for anchoring |

## Database Schema

The sequencer uses PostgreSQL with 13 migrations:

| Table | Purpose |
|-------|---------|
| `ves_events` | Signed event envelopes with sequence numbers |
| `batch_commitments` | Merkle roots with state chain continuity |
| `agent_signing_keys` | Ed25519 key registry |
| `sequence_counters` | Gap-free sequence assignment |
| `entity_versions` | Optimistic concurrency control |
| `projection_checkpoints` | Event projection progress |
| `rejected_events_log` | Failed event audit trail |
| `x402_payment_intents` | Agent-to-agent payment requests |
| `x402_payment_batches` | Batched payment settlement |
| `ves_validity_proofs` | STARK batch proof storage |
| `ves_compliance_proofs` | Per-event encrypted proofs |
| `ves_sequencer_receipts` | Signed sequencer receipts |
| `api_keys` | Authentication (SHA-256 hashed, never plaintext) |

## Economics

- **Per-event cost**: Zero (agents submit events at no charge)
- **Anchoring cost**: ~$0.08 per batch of 100+ events (amortized across tenants)
- **Per-event anchoring**: Fractions of a cent
- **Payment settlement**: Gas-efficient batching (100–1,000 intents per batch)

## Running the Sequencer

```bash
# Local development
docker-compose up -d postgres
cargo run --release

# Production (Kubernetes)
helm install stateset-sequencer ./k8s/helm \
  --set database.url=postgresql://... \
  --set anchoring.rpcUrl=https://rpc.stateset.zone
```
