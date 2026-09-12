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

### Pull — Receive Remote Events

```bash
stateset-sync pull
```

Fetches new events from the sequencer, verifies each one against its author's
signing key, and stores what verifies in the local pulled-event table. Nothing is
applied to local entity state yet — pulled events are readable, not projected —
and what fails verification is quarantined rather than stored. See
[Receiving events](#receiving-events-verification-and-quarantine).

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

| Reason | What it means | What to do |
|---|---|---|
| `sequencer_key_not_configured` | `sequencerPublicKey` is unset locally, so no directory can be verified. | `stateset-sync config set sequencer-public-key <hex>`, then `stateset-sync doctor --promote`. |
| `key_unresolved` | The key could not be obtained: the sequencer was unreachable past `peerKeyMaxStaleSeconds`, or the peer has no such `key_id` yet. | Usually benign — a peer that pushed before registering its key. Re-run `doctor --promote` once the key lands. |
| `directory_untrusted` | A key-directory response was refused: unsigned, bad signature, issued for a different agent or tenant, too stale, or (gRPC) never cryptographically attested. | **Not benign.** Investigate the sequencer and the transport. |
| `signature_invalid` | The author signature did not verify under the key the directory names. | A forgery or a corrupted envelope. Investigate the peer. |
| `key_revoked` | The key was revoked at or before the event's `createdAt`. | Expected after a revocation; the peer must re-push under a current key. |
| `key_outside_validity_window` | The key exists but was not valid when the event was created. | Check the peer's key validity windows and clock. |
| `peer_key_conflict` | The directory presented a different public key for an already-pinned `(agent_id, key_id)`. | Treat as a compromised or lying sequencer until proven otherwise. |

### `stateset-sync doctor`

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

### Known limitations

- **The gRPC receive path is unsupported.** `pull()` refuses to run on a gRPC
  transport, and streamed events are never stored: the gRPC envelope omits
  fields the signing hash binds, and the gRPC key directory carries no
  directory signature, so it can be neither cached nor trusted. Use an
  `https://` sequencer URL to receive. Pushing over gRPC is unaffected.
- **`securityProfile` is not enforced on receive.** The push path calls
  `assertEventMatchesSecurityProfile`; the receive path deliberately does not,
  so an agent configured `hybrid` or `pqc-strict` still accepts a peer event
  declaring plain Ed25519 (`agentSignatureScheme: 0`). Enforcing it today would
  quarantine every peer that has not migrated. No forgery becomes possible in
  the meantime — the attacker still needs the peer's Ed25519 private key — and
  this is tracked for a release in which peers have migrated.
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
