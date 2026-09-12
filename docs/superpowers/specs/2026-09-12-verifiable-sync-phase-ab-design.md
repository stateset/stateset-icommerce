# Verifiable Sync — Phases A & B Design

**Date:** 2026-09-12
**Repos:** `stateset-icommerce` (engine + CLI), `stateset-sequencer` (one new endpoint)
**Status:** Approved design, pending implementation plan

## Problem

The VES sync path advertises verifiable event replication between AI agents. Two
defects break that claim at the boundaries.

**Events are lost at the source.** `cli/src/sync/capture.js` monkey-patches the
JS resource API, runs the mutation, then appends to the outbox in a `try`/`catch`
that logs and swallows (`capture.js:213-226`). The mutation commits, the event
does not, and nothing reports it. Any write that does not pass through the
wrapped JS surface — the Rust API, the HTTP crate, raw SQL — emits no event at
all. The log is therefore incomplete by construction, and its completeness
depends on 66 hand-maintained strings in `EVENT_MAPPINGS`.

**Events are not verified at the destination.** The pull path stores remote
events without checking their agent signatures (`engine.js:467`). Only the
sequencer's *receipt* signature is checked, and only when `sequencerPublicKey`
happens to be configured. Cryptography therefore protects the sequencer from
agents, but never protects agents from each other. The path also reports
`applied: result.events.length` and a literal `conflicts: 0` (`engine.js:478-479`,
`485-486`, `396`, `406`, `497`), which is untrue — nothing is applied to local
state, and conflicts are never counted.

## What already exists

Three findings that shape the design.

A **transactional outbox is already built and proven**.
`crates/stateset-db/src/kernel_outbox.rs` defines durable event records "emitted
atomically with commerce mutations", and `SqliteKernelExecutor` writes the outbox
fact inside `with_immediate_transaction` alongside the mutation and the sealed
receipt (`sqlite/kernel_executor.rs:346,377`). It covers the 22 governed command
types dispatched by `crates/stateset-embedded/src/commerce/kernel.rs:212-247` —
`checkout.commit`, `payments.create`, `payments.create_refund`,
`inventory.reserve|reservation.confirm|reservation.release`, `orders.transition`,
`orders.ship`, `returns.transition`, `ledger.post`, `x402.settle`,
`subscriptions.charge`, `inventory.item.create`, `products.create`, and the A2A
escrow and dispute set.

**Nothing drains `kernel_outbox`.** The lease, publish, retry and dead-letter API
in `sqlite/kernel_outbox.rs` has no consumer anywhere in the workspace. The
outbox is durable but inert, so a new pump has no competitor to coordinate with.

**Peer signing keys are not readable.** `POST /api/v1/agents/keys` registers
them; there is no GET. `GET /api/v1/agents/:agent_id` is self-or-admin scoped and
returns API-credential details, not VES signing keys
(`sequencer/src/api/handlers/agents.rs:604-612`). Agent A cannot obtain agent B's
public key, which is why receive-side verification was never wired —
`client.verifyEventSignature()` already exists and handles hybrid PQC
(`cli/src/sync/client.js:792`), but has nothing to verify against.

## Non-goals

Deferred to later phases, and deliberately not addressed here:

- A shared deterministic reducer between the embedded engine and the sequencer's
  projectors (Phase C).
- Applying pulled events to local state, and a real state root (Phase D). The
  sequencer's `compute_state_root` (`src/infra/commitment.rs:383`) hashes
  `(entity_type, entity_id, version)` triples without length delimiters — a
  version-vector digest, not a state digest. Fixing it belongs with the reducer
  that gives it something real to hash.
- Invariant-preserving typed merges for inventory and money (Phase E). Conflict
  strategy stays `RemoteWins`/`LastWriterWins` for now
  (`crates/stateset-sync/src/conflict.rs:19-23`).

---

# Phase A — an honest receive path

## A1. Sequencer: signed key directory

New endpoint `GET /api/v1/agents/:agent_id/signing-keys?tenant_id=<uuid>`.

Handler `list_agent_signing_keys` in
`sequencer/src/api/handlers/agent_keys.rs`, routed in `src/api/mod.rs` beside the
existing `/v1/agents/keys` registration.

Response:

```json
{
  "agentId": "uuid",
  "tenantId": "uuid",
  "keys": [
    {
      "keyId": 1,
      "algorithm": "ed25519",
      "publicKey": "0x<32 bytes>",
      "publicKeyBundle": { "ed25519PublicKey": "0x…", "mlDsa65PublicKey": "0x…" },
      "validFrom": "2026-01-01T00:00:00Z",
      "validTo": "2027-01-01T00:00:00Z",
      "revokedAt": null
    }
  ],
  "signedAt": "2026-09-12T00:00:00Z",
  "directorySignature": "0x<64 bytes>"
}
```

The directory returns **every key ever registered for the agent**, including
expired and revoked ones, each with its validity window. Historical events signed
by a since-rotated key must still verify; the client, not the server, decides
whether a given event falls inside its key's window.

Signature: `SHA256(b"VES_KEYDIR_V1" || JCS({agentId, tenantId, keys, signedAt}))`
signed with the existing VES sequencer signing key (`VES_SEQUENCER_SIGNING_KEY`,
`src/infra/secrets.rs:141`). Add `DOMAIN_KEYDIR: &[u8] = b"VES_KEYDIR_V1"` to
`src/crypto/hash.rs` alongside the existing `VES_*_V1` separators.

If `VES_SEQUENCER_SIGNING_KEY` is unset the endpoint returns **503**, not an
unsigned directory. An unsigned key directory is worse than no directory: it
invites clients to trust key material with no provenance.

Authorization changes from self-or-admin to **any authenticated agent whose
`auth.tenant_id` equals the `tenant_id` query parameter**, plus admin. The
cross-tenant denial is the security-critical test for this endpoint. Reads go
through the existing `src/infra/cache.rs` layer.

## A2. CLI: key directory client with pinning

New module `cli/src/sync/key-directory.js`:

```js
class PeerKeyDirectory {
  async resolve(agentId, keyId, atTime)  // → key material, or null
  async refresh(agentId)                 // → fetch + verify + cache + pin
}
```

Three new tables, created with `CREATE TABLE IF NOT EXISTS` following the
existing pattern in `cli/src/sync/outbox.js`:

- `peer_keys(agent_id, key_id, algorithm, public_key, public_key_bundle,
  valid_from, valid_to, revoked_at, fetched_at, PRIMARY KEY (agent_id, key_id))`
- `peer_key_pins(agent_id, key_id, public_key, pinned_at,
  PRIMARY KEY (agent_id, key_id))`
- `quarantined_events(...)` — the `pulled_events` columns plus `reason` and
  `quarantined_at`

Every directory response has its `directorySignature` verified against the
configured sequencer public key before any key is cached. An invalid signature is
a hard refusal of the whole response — that is a MITM, not a bad event.

Pinning rules:

- First sight of `(agent_id, key_id)` pins its public key.
- A **different** public key later presented for an already-pinned
  `(agent_id, key_id)` is refused. Emit `peer-key-conflict` and quarantine that
  agent's events until an operator resolves it.
- A **new** `key_id` for a known agent is accepted and pinned. This is the
  rotation path, and it is why rotation must always allocate a fresh `key_id`.

Cache freshness: `peerKeyTtlSeconds` (default 300) triggers a refresh. If refresh
fails, cached keys continue to serve until `peerKeyMaxStaleSeconds` (default
86400), after which affected events quarantine rather than sync halting.

## A3. Receive-path verification

In `engine.js` `pull()` (`engine.js:382-500`), each event resolves
`(envelope.sourceAgent, envelope.agentKeyId)` through `PeerKeyDirectory` at
`envelope.createdAt`, then verifies via the existing
`client.verifyEventSignature(envelope, publicKeyBundle)`.

- Verified → `storePulledEvents` as today.
- Unverified → `storeQuarantinedEvents` with a reason of `signature_invalid`,
  `key_unresolved`, `key_outside_validity_window`, `key_revoked`, or
  `peer_key_conflict`. Quarantined events are **never** returned by application
  reads.
- The cursor advances either way. One bad event must not wedge an agent's sync.
- Self-authored echoes (`sourceAgent === own agentId`) are verified too. It costs
  nothing and continuously checks our own signing path.

Verification is unconditional. There is no configuration flag that disables it.

## A4. Counter honesty

`applied` is **deleted**, not renamed. Nothing is applied to local state until
Phase D, and a field claiming otherwise is the defect. The emitted `pull` payload
becomes:

```js
{ pulled, verified, quarantined, stored, conflicts }
```

Every value computed from what actually happened. `conflicts` is wired to
`detectConflicts()` (`engine.js:573-580`), which already exists and is currently
bypassed by the hardcoded literal.

## A5. `sync doctor`

New subcommand in `cli/src/commands/sync.js`, exposed through
`cli/bin/stateset-sync.js`:

- list quarantined events with reasons and counts by peer
- show current pins and any pin conflicts
- re-verify quarantined events after a key registration lands, promoting those
  that now verify into `pulled_events`

Re-verification is the designed recovery path for the common benign case: an
agent pushed events before its key reached the directory.

---

# Phase B — a complete log at the source

## B1. Migration 096

`crates/stateset-db/migrations/096_kernel_outbox_tier.sql`, registered in
`crates/stateset-db/src/migrations.rs` after `095_kernel_economic_budgets`:

```sql
ALTER TABLE kernel_outbox ADD COLUMN tier TEXT NOT NULL DEFAULT 'governed';
CREATE INDEX idx_kernel_outbox_tier_unpublished
  ON kernel_outbox (tier, published_at) WHERE published_at IS NULL;
```

Existing rows are governed by definition, so the default backfills correctly.

The two backends carry independent numbering sequences. The mirrored Postgres
migration is `crates/stateset-db/src/postgres/migrations/103_kernel_outbox_tier.sql`
(next free there is 103, after `102_kernel_economic_budgets`; next free under
`crates/stateset-db/migrations/` is 096).

## B2. The recorded tier

New helper `record_outbox_fact(tx, RecordedFact)` in
`crates/stateset-db/src/kernel_outbox.rs`, with a SQLite implementation in
`sqlite/kernel_outbox.rs`. It takes an open transaction and writes one
`kernel_outbox` row with `tier = 'recorded'`.

It is called inside the existing `with_immediate_transaction` block of every
mutating SQLite write path not already covered by a governed command.

The authoritative work list is the parity lint in B3, not a hand-written
enumeration — that is the point of building the lint first. `EVENT_MAPPINGS`
(66 entries: `customers.*`, `products.update|delete`, `carts.*`,
`orders.create|update`, `returns.create` and the rest) is a useful lower bound
for estimating, but the lint will surface mutating paths that `capture.js` never
covered, and those are exactly the ones whose absence from the log is invisible
today.

Recorded facts carry no policy evaluation, no budget check and no sealed receipt.
They exist to make one invariant true: **no mutation commits without its event.**
That invariant, not governance ceremony, is what makes the log trustworthy.

## B3. Parity lint

New `crates/stateset-db/tests/outbox_emission_parity.rs`, modeled directly on the
existing `crates/stateset-db/tests/backend_transaction_parity.rs`: enumerate the
mutating public methods of the embedded engine, assert each produces exactly one
`kernel_outbox` row in the same transaction, and fail closed on anything new.
Genuine exemptions (read-through caches, idempotent no-ops) go in an explicit
tracked list in the test, so adding one is a visible diff.

This is what prevents the decay that produced the current state.
`EVENT_MAPPINGS` rotted because a missing entry cost only a `console.warn`.

**B3 lands before B2.** The lint is written first, against the current code, with
every uncovered mutating path recorded in its tracked-violation list. B2 then
empties that list. Building it in that order means the work list is generated by
the tooling rather than guessed, and the list shrinking to zero is the completion
criterion for Phase B.

## B4. The outbox pump

New `cli/src/sync/outbox-pump.js`. Per cycle:

1. Lease a batch of unpublished `kernel_outbox` rows in `(created_at, id)` order,
   using the existing `lease_owner`/`lease_expires_at` columns.
2. Map `(aggregate_type, event_type)` to VES `(entity_type, event_type)`.
   `EVENT_MAPPINGS` survives as exactly this translation table, relocated from
   `capture.js` into the pump.
3. Derive `event_id = uuidv5(kernel_outbox.id, VES_OUTBOX_NAMESPACE)` with a
   fixed namespace UUID constant. Deterministic derivation is what makes replay
   idempotent.
4. Build the VES envelope, sign it with the existing `signEventHash*` helpers in
   `cli/src/sync/crypto.js`, append to the VES outbox.
5. Mark the row published.

Signing after commit is sound because the envelope is derived deterministically
from a durably committed row. The commit is the no-loss point; a crash between
commit and sign re-derives a byte-identical envelope on the next cycle. Failures
increment `attempts` and set `last_error`, and exhausted rows dead-letter —
machinery that already exists in the table and has never been used.

## B5. Retiring `capture.js`

`wrapCommerceWithEvents` becomes a no-op shim that logs a deprecation warning for
one release, then is deleted. Its `EVENT_MAPPINGS` table moves to the pump.

---

## Failure modes

| Situation | Behavior |
|---|---|
| Crash between commit and pump | Row committed and unpublished; replays next cycle |
| Pump signs, dies before marking published | Same `event_id` re-derived; sequencer dedupes on `event_id`/`command_id` |
| Sequencer rejects an event | Existing dead-letter path via `attempts`/`last_error`/`dead_lettered_at` |
| Key directory unreachable | Cached keys serve to `peerKeyMaxStaleSeconds`, then quarantine; sync continues |
| Directory signature invalid | Hard refusal of the entire response; nothing cached |
| Pinned key changed for same `key_id` | Refuse, emit `peer-key-conflict`, quarantine that peer |
| Event outside its key's validity window | Quarantine with `key_outside_validity_window` |
| `VES_SEQUENCER_SIGNING_KEY` unset | Directory endpoint returns 503 |

## Testing

TDD throughout — test first, watch it fail, then implement.

**Phase A, sequencer:** cross-tenant read denied; admin read allowed; directory
signature computed over canonical bytes under `VES_KEYDIR_V1`; revoked and
expired keys present with correct markers; 503 when the signing key is absent.

**Phase A, CLI:** directory signature verification rejects a tampered response;
pin conflict refuses and quarantines; new `key_id` accepted as rotation; event
outside validity window quarantined; tampered event body quarantined rather than
stored; stale-cache fallback then quarantine; `sync doctor` promotes
newly-verifiable events. One assertion exists solely to prevent regression: the
`pull` payload has no `applied` field.

**Phase B:** property test that every mutating method emits exactly one
`kernel_outbox` row within its own transaction; crash-replay produces exactly one
VES event; double-pump is idempotent; a governed command still produces its
sealed receipt unchanged; `tier` defaults correctly for pre-migration rows.

**Integration:** two agents against a live sequencer — A writes, B pulls and
verifies; then A's event is tampered in transit and B quarantines it.

## Rollout and compatibility

Retiring `capture.js` changes which events reach the log and their shape. This is
wire-visible to anything consuming the VES stream and requires a release note and
an event-catalog version bump.

Deploy order matters: the sequencer's key directory endpoint must ship **before**
CLI clients that require it, or every agent quarantines everything on upgrade.
Phase A and Phase B are independent and may ship in either order relative to each
other.

Existing deployments have no `peer_key_pins` rows, so first sync after upgrade
pins whatever the directory currently serves — trust-on-first-use at the
population level. Operators who need stronger initial assurance should register
keys and inspect `sync doctor` output before the first post-upgrade pull.
