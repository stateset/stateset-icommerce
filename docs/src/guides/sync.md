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
    │── 5. Verify & apply ──────► │                            │
    │   (signature + Merkle check)│                            │
```

## Setup

```bash
# Initialize sync with a sequencer
stateset-sync init \
    --sequencer-url https://sequencer.stateset.com \
    --tenant-id <uuid> \
    --store-id <uuid> \
    --api-key <key>
```

This creates `.stateset/sync.json` which activates Tier 2 capabilities.

## Core Operations

### Push — Send Local Events

```bash
stateset-sync push
```

Pushes all unsent events from the local outbox to the sequencer. Each event is signed with the agent's Ed25519 key before transmission.

### Pull — Receive Remote Events

```bash
stateset-sync pull
```

Fetches new events from the sequencer and applies them to the local database after verifying signatures and Merkle proofs.

### Full Sync

```bash
stateset-sync push && stateset-sync pull
```

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

The push operation drains the outbox:

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
