# Sync (VES)

Verifiable Event Sync (VES) keeps local SQLite stores aligned with the sequencer service, providing cryptographic audit trails and multi-agent state coordination. This is a Tier 2+ feature.

## How Sync Works

```
Local Agent                    Sequencer                   Other Agents
    │                              │                            │
    │── 1. Commerce operation ──►  │                            │
    │   (creates local event)      │                            │
    │                              │                            │
    │── 2. Push (outbox) ────────►│                            │
    │   (Ed25519 signed events)   │                            │
    │                              │── 3. Broadcast ──────────►│
    │                              │   (ordered delivery)       │
    │                              │                            │
    │◄── 4. Pull ─────────────────│                            │
    │   (events from other agents)│                            │
    │                              │                            │
    │── 5. Verify & store ──────► │                            │
    │   (author signature; failures│                           │
    │    quarantined, not applied) │                           │
```

## Setup

```bash
# Initialize sync with a sequencer
stateset-sync init \
    --sequencer-url https://sequencer.stateset.com \
    --tenant-id <uuid> \
    --store-id <uuid> \
    --api-key <key> \
    --sequencer-public-key <hex>
```

This creates `.stateset/sync.json` which activates Tier 2 capabilities.

`--sequencer-public-key` is **required to receive events.** Without it the agent
can push, but every pulled event is quarantined (see
[Receiving events](#receiving-events-verification-and-quarantine)) and nothing is
stored. It can also be set afterwards:

```bash
stateset-sync config set sequencer-public-key <hex>
stateset-sync config show          # secrets redacted
```

## Core Operations

### Push — Send Local Events

```bash
stateset-sync push
```

Pushes all unsent events from the local outbox to the sequencer. Each event is signed with the agent's Ed25519 key before transmission. If a `SyncEvent` already carries VES envelope metadata such as `command_id`, `base_version`, `source_agent_id`, or `agent_key_id`, the Rust transport forwards it instead of reconstructing it. On success, the sequencer can acknowledge each event with its canonical remote sequence number, and the Rust engine retains that local-to-canonical mapping plus any receipt handle in a bounded durable confirmation log.

The engine and REST client check every outgoing event against the configured
tenant and store before sending the batch. Missing or mismatched identifiers
refuse the entire batch without changing pending records. This also applies to
engine previews and runs before the REST retry loop, so a configuration mismatch
does not cause repeated requests. Changing the configured destination does not
retarget previously signed events; use the correct store configuration or a
separately reviewed migration process. The transport never rewrites their signed
scope to make them fit.

Push settlement validates the existing ordered, contiguous receipt format before
changing local state. Accepted and rejected counts must account for the entire
submitted batch; rejection IDs must be unique members of that batch; the accepted
sequence range must match its count and fit under the reported head. Counts and
sequences must be nonnegative safe integers (accepted sequences start at 1).
Incomplete or inconsistent receipts leave the batch pending rather than guessing
which events succeeded. Entirely rejected batches are marked rejected with their
reasons; those rows are retained for inspection.

Acknowledgements, rejection statuses, and sync cursors commit together. A local
write failure leaves events available for retry, so the sequencer must deduplicate
by event ID. A late rejection cannot undo an existing acknowledgement, and a
conflicting remote sequence cannot replace the first acknowledged sequence.
`lastPushedSequence` tracks this client's accepted events; `headSequence` tracks
the highest reported remote head. This changes the earlier behavior that copied
the global head into both fields. Receipt validation is not cryptographic proof
of external execution. Noncontiguous acknowledgements require a future per-event
receipt contract, and a duplicate rejection without an earlier local acceptance
still needs operator reconciliation with the sequencer.

### REST Request Deadlines and Retries

REST requests default to a 30-second deadline covering connection, response
headers and the complete success/error body. Set `sync.requestTimeoutMs` in
`.stateset/sync.json` (or the programmatic `SyncConfig`) to an integer from 1 to
2,147,483,647 milliseconds. Invalid values are refused when constructing the
REST client. gRPC uses its separate transport settings.

A deadline aborts the HTTP request and raises an error with code
`SEQUENCER_TIMEOUT`. Each push retry gets a fresh deadline; the total duration
can include all attempts and their backoff delays. `sync.retryPolicy.maxRetries`
controls additional attempts, and an explicit `pushWithRetry(batch, 0)` disables
them. Retry counts must be safe nonnegative integers; backoff delays must be
integers from 0 to 2,147,483,647 milliseconds.

A timed-out write may already have reached the sequencer. It does not prove
remote rollback or cancellation. Without a valid receipt, the engine keeps the
signed event pending and leaves its push cursor unchanged, including across
restart. A later retry sends the same event identity and signature. The receiver
must deduplicate it or provide a reconcilable prior acknowledgement; local
timeouts do not establish exactly-once remote effects.

REST `disconnect()` aborts active requests and retry backoff with
`SEQUENCER_DISCONNECTED`. New requests are refused until `connect()` starts a
new connection. Reconnecting does not revive an earlier retry loop or pull
iterator, including events still buffered inside that iterator. Concurrent
unified-client connection attempts share one handshake; disconnect invalidates
an unfinished handshake and prevents it from publishing a late connection.

Engine shutdown stops background scheduling and disconnects the client. An
interrupted push keeps its unacknowledged signed records pending. Cancellation
does not undo remote effects, and shutdown is not a general drain of application
work: await outstanding operations before closing their database handle. These
request/backoff cancellation guarantees apply to REST; gRPC retains its own
connection and RPC cancellation behavior. The unified wrapper suppresses events
from retired transports and closes a retired gRPC transport if its handshake
finishes late.

### Pull — Receive Remote Events

```bash
stateset-sync pull
```

Fetches new events from the sequencer, verifies each one against its author's
signing key, and stores what verifies in the local pulled-event table. Nothing is
applied to local entity state yet — pulled events are readable, not projected —
and what fails verification is quarantined rather than stored. See
[Receiving events](#receiving-events-verification-and-quarantine).

The CLI REST cursor is the **next inclusive `from` value**, despite the persisted
field name `lastPulledSequence`. A nonempty page advances it to one past the
highest returned sequence; an empty page leaves it unchanged. Explicit
`has_more` responses control continuation, including underfilled pages. Without
that field, the client continues when a page reaches the requested limit.

Pages with duplicate, stale, fractional or unsafe sequence numbers, too many
events, or inconsistent head/cursor/continuation metadata are refused before
verification or storage. An empty page cannot claim more results. Sequence
numbers must leave room for an exactly representable next cursor in JavaScript;
gaps are allowed because a scoped feed need not contain every global sequence.
Saved pull progress and remote head never decrease when concurrent pulls finish
out of order or an operator explicitly replays an earlier page. This validates
page consistency; it does not prove that the server supplied every event.

### Reading Sync Health

The engine, `stateset-sync status`, direct sync command and `sync_status` tool
share the same status calculation. `localHead` is the receive cursor minus one
(with zero for a fresh store); `nextPullCursor` is the next inclusive request
cursor. `lag` is a nonnegative **sequence distance**, not an event count. Scoped
feeds can have gaps. A cursor includes quarantined and retained records, so zero
lag alone does not mean every event is verified or usable.

`receive` reports verified record count and highest verified sequence, quarantine
counts by reason, and the count/earliest sequence of retained receive failures.
These aggregates include all records, without returning payloads or journal
error details. Counts survive restarts and do not stop at the event-list limit.

Connected status is `degraded` when quarantine or retained failures exist, any
outgoing record is failed/rejected, the sequence gap reaches 100, or pending
writes reach 1,000. An invalid or regressed remote head also prevents healthy
status. `healthReasons` identifies the conditions; offline reports preserve the
local diagnostics and cached head. Missing or invalid REST head responses now
fail instead of silently becoming zero.

`lastSyncAt` is null until a sync timestamp has been recorded. `lastPullAt`
(also exposed as the engine's `lastPull`) records a committed nonempty receive
batch, including a batch needing quarantine or recovery. It commits with the
receive records and cursor. Pushes and empty responses do not update it; older
stores have null until their next committed receive batch. It is not evidence
of entity projection or application readiness: received events remain unprojected.

### Full Sync

```bash
stateset-sync push && stateset-sync pull
```


## Receiving events: verification and quarantine

Every event returned by `pull()` is verified against the signing key its author
claims, resolved through the sequencer's **signed key directory**. Nothing
unverified is ever written to the pulled-event store, and there is no
configuration flag that turns verification off.

### Configuration

| Field (`.stateset/sync.json`) | Default | Meaning |
|---|---|---|
| `sequencerPublicKey` | `null` | The sequencer's Ed25519 public key, used to verify the signature over each key-directory response. **Required to receive.** |
| `peerKeyTtlSeconds` | `300` | How long a cached peer key directory is reused before re-fetching. Also the worst case for how long a revoked peer key keeps verifying events. |
| `peerKeyMaxStaleSeconds` | `86400` | How long cached peer keys may keep serving while the sequencer is *unreachable*. Past this, affected events quarantine rather than sync halting. |

**Encoding of `sequencerPublicKey`:** the raw 32-byte Ed25519 public key as 64
hex characters, `0x` prefix optional. It is stored normalized as
`0x`-prefixed lowercase hex. It is not a DER/SPKI blob and not base64; if you
have a PEM key, export the raw key bytes (the last 32 bytes of the SPKI DER).

Set it at `init` (`--sequencer-public-key`, `--peer-key-ttl-seconds`,
`--peer-key-max-stale-seconds`) or afterwards with
`stateset-sync config set <key> <value>`.

### Pinning

The first time a `(agent_id, key_id)` pair is resolved, its public key is
pinned. A *different* public key later presented for an already-pinned
`key_id` is refused — a sequencer that can silently swap key material can forge
any agent's events. A *new* `key_id` for a known agent is accepted and pinned:
that is the rotation path, and it is why rotation must always allocate a fresh
`key_id`.

### Quarantine reasons

Events that cannot be verified go to `_ves_quarantined_events` and are never
returned by application reads. The reason is the current diagnosis:

The configured tenant and store are checked before key lookup or signature
verification. Even a correctly signed event for another tenant or store is
quarantined as `scope_mismatch`. Missing scope fields that cannot fit the
quarantine schema remain in the receive-failure journal. These checks do not
block valid events in the same batch. Scope comes only from operator
configuration; an event cannot select its own receive destination.

Verification checks the received content against the signed hashes before
checking the author signature. Plaintext must match its canonical payload hash
and carry a zero ciphertext hash. Encrypted events bind their nonce, ciphertext,
tag, envelope-derived AAD, and canonical recipient list to the ciphertext hash.
Recipient aliases must agree with that list, and encrypted events cannot carry
an unauthenticated parallel cleartext payload. Malformed encodings, unsupported
encryption headers, or content/hash mismatches fail verification. The salted
plaintext hash of encrypted content is checked later during decryption, when a
recipient has the private key; ciphertext integrity alone does not prove that
decryption will succeed or that the business contents are valid.

| Reason | What it means | What to do |
|---|---|---|
| `sequencer_key_not_configured` | `sequencerPublicKey` is unset locally, so no directory can be verified. | `stateset-sync config set sequencer-public-key <hex>`, then `stateset-sync doctor --promote`. |
| `key_unresolved` | The key could not be obtained: the sequencer was unreachable past `peerKeyMaxStaleSeconds`, or the peer has no such `key_id` yet. | Usually benign — a peer that pushed before registering its key. Re-run `doctor --promote` once the key lands. |
| `directory_untrusted` | A key-directory response was refused: unsigned, bad signature, issued for a different agent or tenant, too stale, or (gRPC) never cryptographically attested. | **Not benign.** Investigate the sequencer and the transport. |
| `signature_invalid` | The received payload does not match its signed hashes, or the author signature did not verify under the directory key. | A forgery, malformed content, or a corrupted envelope. Investigate the peer. |
| `security_profile_mismatch` | Signature or encryption material violates the configured security profile. | Upgrade the peer's signing/encryption configuration; replay or re-verify once compatible. Never silently downgrade policy to promote an event. |
| `scope_mismatch` | The event's tenant or store does not exactly match the configured destination. | Investigate routing and configuration. Do not alter the signed event or promote it into another store. |
| `event_identity_conflict` | Promotion would replace an existing verified event ID or sequence with different content or metadata. | Investigate the source and sequencer; promotion must not overwrite the verified record. |
| `key_revoked` | The key was revoked at or before the event's `createdAt`. | Expected after a revocation; the peer must re-push under a current key. |
| `key_outside_validity_window` | The key exists but was not valid when the event was created. | Check the peer's key validity windows and clock. |
| `peer_key_conflict` | The directory presented a different public key for an already-pinned `(agent_id, key_id)`. | Treat as a compromised or lying sequencer until proven otherwise. |

### `stateset-sync doctor`

Local storage failures are retained separately in `_ves_receive_failures`. A
malformed signed record can violate the normal receive or quarantine schema;
the failure journal keeps its normalized envelope, sequence, verification stage,
diagnosis, and database error without those envelope constraints. These records
are excluded from normal application reads. The receive batch and its pull
cursor commit in one SQLite transaction. If even the journal or cursor cannot be
written, the batch rolls back and the cursor stays unchanged.

`doctor` reports the retained count and earliest sequence (also available as
`receiveFailures` in JSON output). `--promote` does not promote these records.
Promotion also checks tenant/store scope. The CLI uses its configured client;
programmatic `syncDoctor` callers must provide a configured client or an explicit
operator-owned `scope: { tenantId, storeId }`. Unconfigured read-only inspection
remains available. This is an upgrade behavior change for callers that previously
promoted events without a destination configuration. Previously stored events
are not retroactively audited or removed by this change.
Inspect them with `outbox.getReceiveFailures(limit)` or read-only SQL against
`_ves_receive_failures`; the journal may contain sensitive commerce payloads.
After repairing the storage cause, replay from an earlier cursor through the
normal `SyncEngine.pull({ fromSequence })` path. Successful storage or quarantine
clears the corresponding journal entry. Do not edit signed event contents to
make them fit the schema. This mechanism preserves evidence and permits replay;
it does not apply events to business records or guarantee upstream retention.

Verified receive records are immutable through the outbox APIs. An identical
replay leaves the first row and its `pulled_at` unchanged; JSON key ordering is
ignored when comparing payloads and bundles. Changing an event's sequence,
content, author, signatures, or stored metadata raises `VES_EVENT_CONFLICT`.
The direct batch API rolls back the batch; the engine isolates the conflicting
record in the failure journal and continues with records it can retain safely.
Conflicts carry `event_identity_conflict` and are deliberately not cleared by a
later successful replay or overwritten by a subsequent generic storage error.
The journal retains the first such conflict per sequence for operator review;
it is not an exhaustive history of all conflicting variants. Automatic repair
does not choose which competing history is authoritative. Promotion commits
the verified write, quarantine deletion, and ordinary recovery cleanup in one
transaction, and leaves conflicts quarantined.

This replaces the old receive API's `INSERT OR REPLACE` behavior. Applications
must create a new event for a correction rather than overwrite an earlier one.
Existing database rows are preserved; no migration or retrospective repair is
performed by this change.

The journal is created automatically on outbox initialization. Older clients do
not provide this retention guarantee. The old `receive-store-dropped` diagnostic
is replaced by `receive-store-retained`, emitted after a successful commit;
per-event `receive-store-failed` and `receive-quarantine-failed` signals still
describe failed write attempts, which may subsequently roll back.

```bash
stateset-sync doctor              # counts by reason, current pins
stateset-sync doctor --promote    # re-verify quarantined events, promote what now passes
stateset-sync doctor --json
```

`--promote` is the designed recovery path for the common benign case: a peer
pushed before its key reached the directory. Events that now verify are moved
into the pulled-event store; events that still do not are left quarantined with
their reason **updated** to the current diagnosis, so a `key_unresolved` that is
really a forgery reads as `signature_invalid` afterwards.

A reason is only ever sharpened, never weakened. A directory outage resolves
every event as `key_unresolved`, so both the `--promote` sweep and the ordinary
pull path refuse to overwrite a finding about the event (`signature_invalid`,
`directory_untrusted`, `peer_key_conflict`, `key_revoked`,
`key_outside_validity_window`) with a failure to *obtain* a key. Running
`doctor --promote` while the sequencer is down is therefore safe: it cannot
erase evidence, it simply promotes nothing.

### Known limitations

- **The gRPC receive path is unsupported.** `pull()` refuses to run on a gRPC
  transport, and streamed events are never stored: the gRPC envelope omits
  fields the signing hash binds, and the gRPC key directory carries no
  directory signature, so it can be neither cached nor trusted. Use an
  `https://` sequencer URL to receive. Pushing over gRPC is unaffected.
- **`securityProfile` is enforced on receive and promotion.** Hybrid agents
  require hybrid signatures and, for encrypted events, hybrid recipient wraps;
  strict agents require ML-DSA-only signatures and ML-KEM-only wraps. Incompatible
  events are retained as `security_profile_mismatch` before key lookup. This is
  an upgrade behavior change: legacy peers no longer enter a hybrid/strict
  receive store automatically. Legacy interoperability requires an operator's
  explicit `legacy` profile, subject to the existing profile downgrade controls.
  Historical stored events are not retroactively reclassified by this change.
  `doctor --promote` applies the same profile before verifying or promoting.
  Verification follows the declared signature scheme: missing PQ keys,
  malformed PQ signatures, unavailable native verification, and unknown schemes
  cannot fall back to Ed25519. Profile validation and cryptographic verification
  are separate checks; passing the first never implies passing the second.
- Pulled events are **stored and readable, not applied.** Nothing is projected
  into local entity state yet.

## Key Management

```bash
# Generate a new Ed25519 signing key pair
stateset-sync keys:generate

# Register the public key with the sequencer
stateset-sync keys:register

# Rotate all keys and re-register
stateset-sync keys:rotate --all --register
```

Key rotation creates a new key pair, signs a rotation event with the old key (proving continuity), and registers the new public key with the sequencer.

## Sync State Machine

Events progress through states:

```
local → outbox → pushed → confirmed → anchored (Tier 3)
```

Ordering authority:

- `local` and `outbox` are provisional local states.
- `confirmed` begins only after the sequencer assigns a canonical remote sequence number or acknowledgement.
- Explicit non-retryable rejections are terminal for that local attempt and move the event to a dead-letter queue until an operator resolves or replays it.
- Use canonical remote sequence numbers for replication cursors; never local outbox positions.
- Persist remote cursor state separately from the local outbox; local FIFO position and distributed replication position are different pieces of state.
- If the sequencer returns a continuation cursor for the next page request, keep it separate from the highest canonical sequence actually observed in pulled events.

| State | Description |
|-------|-------------|
| `local` | Event created by a commerce operation |
| `outbox` | Queued for push (outbox pattern) |
| `pushed` | Sent to sequencer, awaiting confirmation |
| `confirmed` | Sequencer has ordered and distributed |
| `anchored` | Merkle root anchored on-chain (Tier 3) |

## Conflict Resolution

When two agents modify the same entity concurrently, the sequencer detects a conflict. Resolution strategies:

| Strategy | Behavior |
|----------|----------|
| `last-write-wins` | Most recent timestamp wins (default) |
| `first-write-wins` | First event received wins |
| `custom` | User-defined merge function |

Conflicts are surfaced as events that agents can inspect:

```bash
stateset-sync conflicts list
stateset-sync conflicts resolve <conflict-id> --strategy last-write-wins
```

## Outbox Pattern

Events are never sent directly. Instead, they're written to a local outbox table in the same transaction as the commerce operation. This guarantees:

- **Atomicity**: If the commerce operation fails, no event is sent
- **Durability**: Events survive process crashes
- **Ordering**: Events are sent in the order they were created

The push operation drains the outbox. The local outbox preserves FIFO for one node, but distributed ordering is finalized only by the sequencer. When the sequencer returns per-event acknowledgements, the engine removes the acknowledged local event ids directly instead of assuming acceptance was a contiguous prefix. When the sequencer explicitly rejects a local event, `stateset-sync` keeps retryable rejections in the outbox and moves non-retryable rejections into a dead-letter queue:

```javascript
// Under the hood
await db.transaction(async (tx) => {
    const order = await tx.insert('orders', orderData);
    await tx.insert('outbox', {
        type: 'order.created',
        payload: order,
        status: 'pending'
    });
});
// Later: stateset-sync push → sends outbox events
```

For durable Rust sync runtimes, persist both layers explicitly:

- `outbox_path` stores pending local events.
- `state_path` stores the remote cursor, latest remote head metadata (`state_root`, `last_commitment_id`), highest acknowledged remote sequence, retained push confirmations, retained dead-letter entries, and any in-progress pull continuation cursor.
- `confirmation_capacity` bounds how many local-to-canonical confirmations are retained after the outbox drains.
- If `state_path` is omitted and `outbox_path` is set, `stateset-sync` derives a sibling `*.state.json` snapshot automatically.

The Rust crate now also ships a concrete `SequencerHttpTransport` for the documented REST flow (`POST /api/v1/ves/events/ingest`, `GET /api/v1/events`), so the Rust path is no longer just a trait boundary. `SyncEvent` now preserves core VES envelope metadata across push and pull flows, dead-letter entries can be inspected through `dead_letter_for_event`, `dead_letters_for_command`, `dead_letters_for_entity`, `latest_dead_letter_for_command`, and `latest_dead_letter_for_entity` before operators requeue or discard them, and `SyncEngine::confirmations()` plus lookup helpers like `confirmation_for_event`, `confirmations_for_command`, `confirmations_for_entity`, `latest_confirmation_for_command`, and `latest_confirmation_for_entity` expose the retained acknowledgement log when exact receipts are available.

## Event Replay

Replay events to reconstruct state at any point in time:

```bash
# Replay from the beginning
stateset-sync replay --from-start

# Replay from a specific event ID
stateset-sync replay --from-event evt_abc123

# Replay to a specific timestamp
stateset-sync replay --to-time "2026-03-15T00:00:00Z"
```

This is useful for:
- Debugging issues (what happened at time X?)
- Auditing (prove that event Y occurred before event Z)
- Disaster recovery (rebuild a database from the event log)

## Monitoring

```bash
# Check sync status
stateset-sync status
# → { lastPush: '2026-03-16T10:30:00Z', lastPull: '2026-03-16T10:30:05Z',
#     outboxPending: 0, eventsReceived: 1547, conflicts: 0 }
# Rust `SyncEngine::status()` also reports `caught_up`, `next_pull_cursor`, and `retained_confirmations` for pagination-aware health checks.

# View sync history
stateset-sync history --limit 20
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `sync_push` | Push outbox events to sequencer |
| `sync_pull` | Pull events from sequencer |
| `sync_status` | Check sync status |
| `sync_history` | View event history |
| `sync_conflicts` | List unresolved conflicts |
| `sync_resolve_conflict` | Resolve a conflict |
| `sync_replay` | Replay events |
| `sync_outbox_status` | Check outbox queue |
| `ves_create_receipt` | Create a VES receipt |
| `ves_verify_receipt` | Verify a receipt |
| `ves_audit_trail` | Query the audit log |

## Troubleshooting

### "Push failed: signature verification error"

The sequencer rejected the event signature. Check:
1. Your signing key is registered: `stateset-sync keys:register`
2. The key hasn't been rotated without re-registering
3. Clock skew is less than 5 minutes

### "Pull returned conflicts"

Two agents modified the same entity. Resolve:
```bash
stateset-sync conflicts list
stateset-sync conflicts resolve <id> --strategy last-write-wins
```

### "Outbox growing but not draining"

Push is not running or the sequencer is unreachable:
1. Check connectivity: `curl https://sequencer.stateset.com/health`
2. Check outbox: `stateset-sync outbox status`
3. Retry: `stateset-sync push --retry`

See [VES v1.0 Specification](../security/ves.md) for cryptographic details.
