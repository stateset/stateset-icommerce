# StateSet Sync Architecture

## Overview

The StateSet Event Sync system provides verifiable, cryptographically-secured event synchronization across distributed commerce systems. This document explains the architecture and components.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        StateSet CLI                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │   Agent     │  │   Agent     │  │   Agent     │                 │
│  │  (Orders)   │  │ (Inventory) │  │  (Returns)  │                 │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                 │
│         │                │                │                         │
│         └────────────────┼────────────────┘                         │
│                          │                                          │
│                    ┌─────▼─────┐                                    │
│                    │   Sync    │                                    │
│                    │  Engine   │                                    │
│                    └─────┬─────┘                                    │
└──────────────────────────┼──────────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼─────┐ ┌────▼────┐ ┌─────▼─────┐
        │  Outbox   │ │ Crypto  │ │ Conflict  │
        │           │ │         │ │ Resolver  │
        └─────┬─────┘ └────┬────┘ └─────┬─────┘
              │            │            │
              └────────────┼────────────┘
                           │
                    ┌──────▼──────┐
                    │  Sequencer  │
                    │   Client    │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ StateSet    │
                    │ Sequencer   │
                    │  (Remote)   │
                    └─────────────┘
```

## Components

### 1. Sync Engine (`src/sync/engine.js`)

The central coordinator for all sync operations.

```javascript
/**
 * @class SyncEngine
 * @description Coordinates event synchronization between local and remote stores
 *
 * Key responsibilities:
 * - Managing sync sessions
 * - Coordinating push/pull operations
 * - Handling reconnection and recovery
 *
 * @example
 * const engine = createSyncEngine({
 *   commerce: commerceInstance,
 *   sequencer: sequencerClient
 * });
 *
 * await engine.push();
 * await engine.pull();
 * await engine.sync(); // bidirectional
 */
```

### 2. Outbox (`src/sync/outbox.js`)

Reliable event delivery with at-least-once semantics.

```javascript
/**
 * @class Outbox
 * @description Stores events for reliable delivery
 *
 * Features:
 * - Persistent queue for events
 * - Automatic retry with exponential backoff
 * - Ordering guarantees per-entity
 *
 * Event lifecycle:
 * 1. Event created → added to outbox (status: pending)
 * 2. Event sent → marked in-flight (status: sending)
 * 3. ACK received → removed from outbox (status: confirmed)
 * 4. Timeout/error → retry with backoff (status: pending)
 *
 * @example
 * const outbox = createOutbox(db);
 * await outbox.add({
 *   entityType: 'order',
 *   entityId: 'abc123',
 *   eventType: 'order.created',
 *   payload: { ... }
 * });
 *
 * const pending = await outbox.getPending();
 * await outbox.markSent(eventId, sequenceNumber);
 */
```

### 3. Crypto (`src/sync/crypto.js`)

Cryptographic primitives for event verification.

```javascript
/**
 * @module crypto
 * @description Cryptographic operations for event integrity
 *
 * Algorithms used:
 * - Ed25519: Event signatures
 * - SHA-256: Content hashing
 * - X25519: Key exchange (for encrypted sync)
 * - AES-256-GCM: Payload encryption
 *
 * @example
 * // Sign an event
 * const signature = await signEvent(event, privateKey);
 *
 * // Verify event signature
 * const valid = await verifyEventSignature(event, signature, publicKey);
 *
 * // Create commitment hash
 * const commitment = await createCommitment(events);
 */
```

**Key Functions:**

```javascript
/**
 * Sign an event with Ed25519
 * @param {Object} event - Event to sign
 * @param {Uint8Array} privateKey - Signing key
 * @returns {Promise<string>} Base64-encoded signature
 */
export async function signEvent(event, privateKey);

/**
 * Verify event signature
 * @param {Object} event - Event to verify
 * @param {string} signature - Base64 signature
 * @param {Uint8Array} publicKey - Verification key
 * @returns {Promise<boolean>} True if valid
 */
export async function verifyEventSignature(event, signature, publicKey);

/**
 * Create Merkle commitment for event batch
 * @param {Array} events - Events to commit
 * @returns {Promise<string>} Root hash
 */
export async function createCommitment(events);
```

### 4. Conflict Resolution (`src/sync/conflict.js`)

Handles concurrent modifications.

```javascript
/**
 * @class ConflictResolver
 * @description Resolves conflicts from concurrent modifications
 *
 * Resolution strategies:
 * - LAST_WRITE_WINS: Most recent timestamp wins
 * - FIRST_WRITE_WINS: Earliest timestamp wins
 * - MANUAL: Requires user intervention
 * - MERGE: Attempts automatic field-level merge
 * - CUSTOM: User-defined resolver function
 *
 * @example
 * const resolver = new ConflictResolver({
 *   defaultStrategy: 'LAST_WRITE_WINS',
 *   entityStrategies: {
 *     'order': 'FIRST_WRITE_WINS',  // Don't overwrite order changes
 *     'inventory': 'MERGE'           // Merge inventory updates
 *   }
 * });
 *
 * const resolved = await resolver.resolve(localEvent, remoteEvent);
 */
```

### 5. Sequencer Client (`src/sync/client.js`)

Communication with the remote sequencer service.

```javascript
/**
 * @class SequencerClient
 * @description Client for StateSet Sequencer service
 *
 * The sequencer provides:
 * - Global event ordering
 * - Causal consistency guarantees
 * - Conflict detection
 *
 * @example
 * const client = createSequencerClient({
 *   endpoint: 'https://sequencer.stateset.com',
 *   apiKey: process.env.STATESET_SEQUENCER_KEY
 * });
 *
 * // Submit events
 * const result = await client.submit(events);
 *
 * // Fetch events since sequence number
 * const events = await client.fetch({ since: 12345 });
 */
```

### 6. Key Management (`src/sync/keys.js`)

Manages encryption and signing keys.

```javascript
/**
 * @module keys
 * @description Key management for sync encryption
 *
 * Key types:
 * - Identity key: Long-term Ed25519 signing key
 * - Session key: Ephemeral X25519 for key exchange
 * - Content key: Derived AES-256 for payload encryption
 *
 * @example
 * const keyManager = createKeyManager({
 *   storePath: '~/.stateset/keys'
 * });
 *
 * // Generate or load identity
 * const identity = await keyManager.getOrCreateIdentity();
 *
 * // Create session for sync
 * const session = await keyManager.createSession(remotePublicKey);
 */
```

### 7. Rotation Policy (`src/sync/rotation-policy.js`)

Automatic key rotation for security.

```javascript
/**
 * @class RotationPolicy
 * @description Manages automatic key rotation
 *
 * Configuration:
 * - rotationInterval: How often to rotate (default: 7 days)
 * - maxAge: Maximum key age before forced rotation
 * - gracePeriod: Overlap period for smooth transition
 *
 * @example
 * const policy = createRotationPolicy({
 *   rotationInterval: 7 * 24 * 60 * 60 * 1000, // 7 days
 *   gracePeriod: 24 * 60 * 60 * 1000 // 1 day overlap
 * });
 *
 * if (policy.shouldRotate(currentKey)) {
 *   await keyManager.rotate();
 * }
 */
```

### 8. Groups (`src/sync/groups.js`)

Permission groups for multi-tenant sync.

```javascript
/**
 * @class GroupManager
 * @description Manages sync permission groups
 *
 * Groups allow:
 * - Scoped sync (only sync certain entities)
 * - Team permissions (who can sync what)
 * - Tenant isolation
 *
 * @example
 * const groups = createGroupManager(db);
 *
 * // Create a group
 * await groups.create({
 *   name: 'inventory-team',
 *   permissions: ['inventory:read', 'inventory:write']
 * });
 *
 * // Add member
 * await groups.addMember('inventory-team', userId);
 */
```

### 9. Configuration (`src/sync/config.js`)

Sync configuration management.

```javascript
/**
 * @module config
 * @description Sync configuration utilities
 *
 * @example
 * const config = loadSyncConfig();
 *
 * // Check if sync is configured
 * if (isSyncConfigured()) {
 *   const engine = createSyncEngine(config);
 * }
 *
 * // Save configuration
 * await saveSyncConfig({
 *   sequencerEndpoint: 'https://...',
 *   apiKey: '...'
 * });
 */
```

## Event Flow

### Push Flow (Local → Remote)

```
1. Local Change
   │
   ▼
2. Event Created
   │
   ▼
3. Event Signed (Ed25519)
   │
   ▼
4. Added to Outbox
   │
   ▼
5. Batch Commitment Created
   │
   ▼
6. Submitted to Sequencer
   │
   ▼
7. ACK Received
   │
   ▼
8. Event Removed from Outbox
```

### Pull Flow (Remote → Local)

```
1. Fetch Events (since last seq)
   │
   ▼
2. Verify Signatures
   │
   ▼
3. Check for Conflicts
   │
   ├──[No Conflict]──▶ Apply Directly
   │
   └──[Conflict]──▶ Resolve
                    │
                    ▼
                 Apply Resolution
```

## CLI Commands

```bash
# Check sync status
stateset-sync status

# Pull remote changes
stateset-sync pull

# Push local changes
stateset-sync push

# Bidirectional sync
stateset-sync

# Show conflicts
stateset-sync conflicts

# View entity history
stateset-sync history <entityType> <entityId>

# Initialize sync
stateset-sync init --endpoint <url> --key <api-key>
```

## Security Considerations

1. **Key Storage**: Private keys are stored encrypted at rest
2. **Transport**: All communication uses TLS 1.3
3. **Signatures**: Every event is individually signed
4. **Commitments**: Batch commitments prevent tampering
5. **Rotation**: Keys are rotated regularly

## Best Practices

1. **Enable sync only when needed** - Sync adds overhead
2. **Use appropriate conflict strategies** - Match your domain
3. **Monitor the outbox** - Large outbox indicates issues
4. **Regular key rotation** - Use the rotation policy
5. **Test conflict resolution** - Ensure your strategy works

## Troubleshooting

### "Sync failed: signature verification failed"
- Keys may be out of sync
- Try `stateset-sync reset-keys`

### "Outbox growing indefinitely"
- Network issues with sequencer
- Check `stateset-sync status`

### "Too many conflicts"
- Consider changing resolution strategy
- Review concurrent modification patterns

## API Reference

See the individual module files for detailed API documentation:

- `src/sync/engine.js` - SyncEngine class
- `src/sync/outbox.js` - Outbox class
- `src/sync/crypto.js` - Cryptographic functions
- `src/sync/conflict.js` - ConflictResolver class
- `src/sync/client.js` - SequencerClient class
- `src/sync/keys.js` - Key management
- `src/sync/rotation-policy.js` - RotationPolicy class
- `src/sync/groups.js` - GroupManager class
- `src/sync/config.js` - Configuration utilities
