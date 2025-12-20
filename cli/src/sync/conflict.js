/**
 * Conflict Resolver
 *
 * Detects and resolves conflicts between local and remote events
 * in the Verifiable Event Sync (VES) protocol.
 */

import crypto from 'crypto';

/**
 * @typedef {'version' | 'concurrent' | 'invariant'} ConflictType
 */

/**
 * @typedef {'remote-wins' | 'local-wins' | 'merge' | 'manual'} ResolutionStrategy
 */

/**
 * @typedef {Object} ConflictInfo
 * @property {string} id - Unique conflict ID
 * @property {ConflictType} type - Type of conflict
 * @property {import('./outbox.js').OutboxEvent} localEvent - Local event
 * @property {Object|null} remoteEvent - Remote event (if applicable)
 * @property {string} entityType - Entity type
 * @property {string} entityId - Entity ID
 * @property {string} description - Human-readable description
 * @property {ResolutionStrategy} suggestedStrategy - Recommended resolution
 * @property {Date} detectedAt - When conflict was detected
 */

/**
 * @typedef {Object} Resolution
 * @property {string} conflictId - Conflict ID
 * @property {ResolutionStrategy} strategy - Strategy used
 * @property {boolean} success - Whether resolution succeeded
 * @property {string} [error] - Error message if failed
 * @property {Object} [result] - Resolution result data
 */

/**
 * Conflict Resolver for VES sync
 */
export class ConflictResolver {
  /**
   * @param {import('./outbox.js').Outbox} outbox - Outbox instance
   * @param {Object} [options]
   * @param {ResolutionStrategy} [options.defaultStrategy='remote-wins'] - Default resolution strategy
   */
  constructor(outbox, options = {}) {
    this.outbox = outbox;
    this.defaultStrategy = options.defaultStrategy || 'remote-wins';
    this._initializeConflictTable();
  }

  /**
   * Initialize the conflicts tracking table
   * @private
   */
  _initializeConflictTable() {
    this.outbox.db.exec(`
      CREATE TABLE IF NOT EXISTS _ves_conflicts (
        id TEXT PRIMARY KEY,
        conflict_type TEXT NOT NULL,
        local_event_seq INTEGER,
        remote_event_seq INTEGER,
        entity_type TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        description TEXT,
        suggested_strategy TEXT,
        status TEXT DEFAULT 'unresolved',
        resolved_at TEXT,
        resolution_strategy TEXT,
        resolution_data TEXT,
        detected_at TEXT DEFAULT (datetime('now')),
        CHECK(status IN ('unresolved', 'resolved', 'skipped')),
        CHECK(conflict_type IN ('version', 'concurrent', 'invariant'))
      );

      CREATE INDEX IF NOT EXISTS idx_ves_conflicts_status
        ON _ves_conflicts (status) WHERE status = 'unresolved';

      CREATE INDEX IF NOT EXISTS idx_ves_conflicts_entity
        ON _ves_conflicts (entity_type, entity_id);
    `);
  }

  /**
   * Detect conflicts between local pending events and remote events
   * @param {Array<import('./outbox.js').OutboxEvent>} localEvents - Pending local events
   * @param {Array<Object>} remoteEvents - Pulled remote events
   * @returns {Array<ConflictInfo>}
   */
  detectConflicts(localEvents, remoteEvents) {
    const conflicts = [];

    // Build a map of remote events by entity
    const remoteByEntity = new Map();
    for (const remote of remoteEvents) {
      const key = `${remote.entity_type || remote.entityType}:${remote.entity_id || remote.entityId}`;
      if (!remoteByEntity.has(key)) {
        remoteByEntity.set(key, []);
      }
      remoteByEntity.get(key).push(remote);
    }

    // Check each local pending event for conflicts
    for (const local of localEvents) {
      const key = `${local.entityType}:${local.entityId}`;
      const remoteForEntity = remoteByEntity.get(key) || [];

      // Version conflict: local base_version is behind current version
      const conflict = this._detectVersionConflict(local, remoteForEntity);
      if (conflict) {
        conflicts.push(conflict);
        continue;
      }

      // Concurrent modification: same entity modified by different agents
      const concurrentConflict = this._detectConcurrentConflict(local, remoteForEntity);
      if (concurrentConflict) {
        conflicts.push(concurrentConflict);
      }
    }

    // Store detected conflicts
    for (const conflict of conflicts) {
      this._storeConflict(conflict);
    }

    return conflicts;
  }

  /**
   * Detect version conflict for a local event
   * @private
   */
  _detectVersionConflict(local, remoteEvents) {
    if (local.baseVersion === null || local.baseVersion === undefined) {
      return null; // No OCC tracking for this event
    }

    // Get current entity version from local tracking
    const currentVersion = this.outbox.getEntityVersion(
      local.tenantId,
      local.storeId,
      local.entityType,
      local.entityId
    );

    if (currentVersion === null) {
      return null; // Entity doesn't exist locally yet
    }

    // Check if our base version is behind
    if (local.baseVersion < currentVersion) {
      return {
        id: crypto.randomUUID(),
        type: 'version',
        localEvent: local,
        remoteEvent: remoteEvents[remoteEvents.length - 1] || null,
        entityType: local.entityType,
        entityId: local.entityId,
        description: `Local event based on version ${local.baseVersion}, but entity is now at version ${currentVersion}`,
        suggestedStrategy: this._suggestStrategy('version', local),
        detectedAt: new Date(),
      };
    }

    return null;
  }

  /**
   * Detect concurrent modification conflict
   * @private
   */
  _detectConcurrentConflict(local, remoteEvents) {
    // Look for remote events from different agents affecting the same entity
    // within a short time window (potential race condition)
    const timeWindow = 5000; // 5 seconds
    const localTime = local.createdAt.getTime();

    for (const remote of remoteEvents) {
      const remoteTime = new Date(remote.sequenced_at || remote.sequencedAt).getTime();
      const timeDiff = Math.abs(localTime - remoteTime);

      // Different agent, similar time, same entity = potential conflict
      const remoteAgent = remote.source_agent || remote.sourceAgent;
      if (remoteAgent !== local.sourceAgent && timeDiff < timeWindow) {
        return {
          id: crypto.randomUUID(),
          type: 'concurrent',
          localEvent: local,
          remoteEvent: remote,
          entityType: local.entityType,
          entityId: local.entityId,
          description: `Concurrent modification by agents ${local.sourceAgent.substring(0, 8)} and ${remoteAgent.substring(0, 8)} within ${timeDiff}ms`,
          suggestedStrategy: this._suggestStrategy('concurrent', local),
          detectedAt: new Date(),
        };
      }
    }

    return null;
  }

  /**
   * Suggest a resolution strategy based on conflict type and entity
   * @private
   */
  _suggestStrategy(conflictType, localEvent) {
    // Entity-specific suggestions
    const entityStrategies = {
      inventory: 'merge', // Inventory adjustments can often be merged
      order: 'remote-wins', // Order state should follow sequencer
      customer: 'merge', // Customer updates can be field-merged
      product: 'remote-wins', // Product catalog follows master
      cart: 'local-wins', // Cart belongs to local agent
    };

    // Type-based defaults
    const typeDefaults = {
      version: 'remote-wins',
      concurrent: 'remote-wins',
      invariant: 'manual',
    };

    return entityStrategies[localEvent.entityType] || typeDefaults[conflictType] || this.defaultStrategy;
  }

  /**
   * Store a detected conflict
   * @private
   */
  _storeConflict(conflict) {
    const stmt = this.outbox.db.prepare(`
      INSERT OR REPLACE INTO _ves_conflicts (
        id, conflict_type, local_event_seq, remote_event_seq,
        entity_type, entity_id, description, suggested_strategy,
        status, detected_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'unresolved', ?)
    `);

    stmt.run(
      conflict.id,
      conflict.type,
      conflict.localEvent.localSeq,
      conflict.remoteEvent?.sequence_number || conflict.remoteEvent?.sequenceNumber || null,
      conflict.entityType,
      conflict.entityId,
      conflict.description,
      conflict.suggestedStrategy,
      conflict.detectedAt.toISOString()
    );
  }

  /**
   * Get all unresolved conflicts
   * @returns {Array<ConflictInfo>}
   */
  getUnresolvedConflicts() {
    const stmt = this.outbox.db.prepare(`
      SELECT c.*, o.*
      FROM _ves_conflicts c
      LEFT JOIN _ves_outbox o ON c.local_event_seq = o.local_seq
      WHERE c.status = 'unresolved'
      ORDER BY c.detected_at ASC
    `);

    const rows = stmt.all();
    return rows.map(row => this._rowToConflict(row));
  }

  /**
   * Get conflict by ID
   * @param {string} conflictId
   * @returns {ConflictInfo|null}
   */
  getConflict(conflictId) {
    const stmt = this.outbox.db.prepare(`
      SELECT c.*, o.*
      FROM _ves_conflicts c
      LEFT JOIN _ves_outbox o ON c.local_event_seq = o.local_seq
      WHERE c.id = ?
    `);

    const row = stmt.get(conflictId);
    return row ? this._rowToConflict(row) : null;
  }

  /**
   * Get conflict count
   * @returns {number}
   */
  getConflictCount() {
    const result = this.outbox.db.prepare(`
      SELECT COUNT(*) as count FROM _ves_conflicts WHERE status = 'unresolved'
    `).get();
    return result.count;
  }

  /**
   * Resolve a conflict with a specific strategy
   * @param {ConflictInfo|string} conflictOrId - Conflict or conflict ID
   * @param {ResolutionStrategy} [strategy] - Resolution strategy (uses suggested if not provided)
   * @returns {Promise<Resolution>}
   */
  async resolve(conflictOrId, strategy) {
    const conflict = typeof conflictOrId === 'string'
      ? this.getConflict(conflictOrId)
      : conflictOrId;

    if (!conflict) {
      return {
        conflictId: typeof conflictOrId === 'string' ? conflictOrId : 'unknown',
        strategy: strategy || 'unknown',
        success: false,
        error: 'Conflict not found',
      };
    }

    const resolveStrategy = strategy || conflict.suggestedStrategy;

    try {
      let result;
      switch (resolveStrategy) {
        case 'remote-wins':
          result = await this._resolveRemoteWins(conflict);
          break;
        case 'local-wins':
          result = await this._resolveLocalWins(conflict);
          break;
        case 'merge':
          result = await this._resolveMerge(conflict);
          break;
        case 'manual':
          return {
            conflictId: conflict.id,
            strategy: 'manual',
            success: false,
            error: 'Manual resolution required - use resolve with explicit strategy',
          };
        default:
          throw new Error(`Unknown resolution strategy: ${resolveStrategy}`);
      }

      // Mark conflict as resolved
      this._markResolved(conflict.id, resolveStrategy, result);

      return {
        conflictId: conflict.id,
        strategy: resolveStrategy,
        success: true,
        result,
      };
    } catch (error) {
      return {
        conflictId: conflict.id,
        strategy: resolveStrategy,
        success: false,
        error: error.message,
      };
    }
  }

  /**
   * Resolve using remote-wins strategy
   * Accept remote state, discard local event
   * @private
   */
  async _resolveRemoteWins(conflict) {
    // Mark local event as rejected
    this.outbox.markRejected(
      conflict.localEvent.localSeq,
      `Conflict resolved: remote-wins (${conflict.description})`
    );

    // If we have the remote event, update local entity version
    if (conflict.remoteEvent) {
      const remoteVersion = conflict.remoteEvent.base_version || conflict.remoteEvent.baseVersion;
      if (remoteVersion !== undefined) {
        this.outbox.updateEntityVersion(
          conflict.localEvent.tenantId,
          conflict.localEvent.storeId,
          conflict.entityType,
          conflict.entityId,
          remoteVersion + 1
        );
      }
    }

    return {
      action: 'local_event_rejected',
      localSeq: conflict.localEvent.localSeq,
      eventId: conflict.localEvent.eventId,
    };
  }

  /**
   * Resolve using local-wins strategy
   * Re-push local event with updated base version
   * @private
   */
  async _resolveLocalWins(conflict) {
    // Get current entity version
    const currentVersion = this.outbox.getEntityVersion(
      conflict.localEvent.tenantId,
      conflict.localEvent.storeId,
      conflict.entityType,
      conflict.entityId
    ) || 0;

    // Create a new event with updated base version
    const newEventId = crypto.randomUUID();
    const newSeq = this.outbox.append({
      eventId: newEventId,
      commandId: conflict.localEvent.commandId, // Keep same command ID for idempotency
      tenantId: conflict.localEvent.tenantId,
      storeId: conflict.localEvent.storeId,
      entityType: conflict.localEvent.entityType,
      entityId: conflict.localEvent.entityId,
      eventType: conflict.localEvent.eventType,
      payload: conflict.localEvent.payload,
      sourceAgent: conflict.localEvent.sourceAgent,
      baseVersion: currentVersion, // Updated base version
    });

    // Mark original event as rejected
    this.outbox.markRejected(
      conflict.localEvent.localSeq,
      `Conflict resolved: local-wins, rebased as event ${newEventId}`
    );

    return {
      action: 'event_rebased',
      originalSeq: conflict.localEvent.localSeq,
      newSeq,
      newEventId,
      newBaseVersion: currentVersion,
    };
  }

  /**
   * Resolve using merge strategy
   * Combine changes from both local and remote
   * @private
   */
  async _resolveMerge(conflict) {
    // Entity-specific merge logic
    const mergeResult = this._mergeChanges(conflict);

    if (!mergeResult.canMerge) {
      // Fall back to remote-wins if merge not possible
      return this._resolveRemoteWins(conflict);
    }

    // Create merged event
    const mergedEventId = crypto.randomUUID();
    const currentVersion = this.outbox.getEntityVersion(
      conflict.localEvent.tenantId,
      conflict.localEvent.storeId,
      conflict.entityType,
      conflict.entityId
    ) || 0;

    const newSeq = this.outbox.append({
      eventId: mergedEventId,
      commandId: conflict.localEvent.commandId,
      tenantId: conflict.localEvent.tenantId,
      storeId: conflict.localEvent.storeId,
      entityType: conflict.localEvent.entityType,
      entityId: conflict.localEvent.entityId,
      eventType: conflict.localEvent.eventType,
      payload: mergeResult.mergedPayload,
      sourceAgent: conflict.localEvent.sourceAgent,
      baseVersion: currentVersion,
    });

    // Mark original as rejected
    this.outbox.markRejected(
      conflict.localEvent.localSeq,
      `Conflict resolved: merge, combined as event ${mergedEventId}`
    );

    return {
      action: 'events_merged',
      originalSeq: conflict.localEvent.localSeq,
      newSeq,
      newEventId: mergedEventId,
      mergedPayload: mergeResult.mergedPayload,
    };
  }

  /**
   * Attempt to merge changes from local and remote events
   * @private
   */
  _mergeChanges(conflict) {
    const local = conflict.localEvent.payload;
    const remote = conflict.remoteEvent?.payload
      ? (typeof conflict.remoteEvent.payload === 'string'
        ? JSON.parse(conflict.remoteEvent.payload)
        : conflict.remoteEvent.payload)
      : null;

    if (!remote) {
      return { canMerge: false };
    }

    // Entity-specific merge strategies
    switch (conflict.entityType) {
      case 'inventory':
        return this._mergeInventory(local, remote);
      case 'customer':
        return this._mergeCustomer(local, remote);
      default:
        return this._mergeGeneric(local, remote);
    }
  }

  /**
   * Merge inventory changes (adjustments are commutative)
   * @private
   */
  _mergeInventory(local, remote) {
    // Inventory adjustments can be summed
    if (local.adjustment !== undefined && remote.adjustment !== undefined) {
      return {
        canMerge: true,
        mergedPayload: {
          ...local,
          adjustment: local.adjustment + remote.adjustment,
          merged: true,
          mergeNote: `Combined adjustments: local(${local.adjustment}) + remote(${remote.adjustment})`,
        },
      };
    }

    // Quantity set operations - take the most recent
    if (local.quantity !== undefined && remote.quantity !== undefined) {
      return {
        canMerge: true,
        mergedPayload: {
          ...remote,
          merged: true,
          mergeNote: 'Took remote quantity as more recent',
        },
      };
    }

    return { canMerge: false };
  }

  /**
   * Merge customer changes (field-level merge for non-conflicting fields)
   * @private
   */
  _mergeCustomer(local, remote) {
    const merged = { ...remote };
    const conflicts = [];

    // Field-level merge
    for (const [key, value] of Object.entries(local)) {
      if (remote[key] === undefined) {
        // Field only in local - keep it
        merged[key] = value;
      } else if (remote[key] !== value) {
        // Both have the field with different values - conflict
        conflicts.push(key);
      }
      // Same value - already in merged from remote spread
    }

    if (conflicts.length > 0) {
      // Some fields conflict - can't auto-merge
      return {
        canMerge: false,
        conflictingFields: conflicts,
      };
    }

    return {
      canMerge: true,
      mergedPayload: {
        ...merged,
        merged: true,
        mergeNote: 'Field-level merge, no conflicts',
      },
    };
  }

  /**
   * Generic merge - only works if payloads are identical or one is subset
   * @private
   */
  _mergeGeneric(local, remote) {
    const localStr = JSON.stringify(local, Object.keys(local).sort());
    const remoteStr = JSON.stringify(remote, Object.keys(remote).sort());

    if (localStr === remoteStr) {
      return {
        canMerge: true,
        mergedPayload: local,
      };
    }

    // Can't auto-merge different generic payloads
    return { canMerge: false };
  }

  /**
   * Mark a conflict as resolved
   * @private
   */
  _markResolved(conflictId, strategy, result) {
    const stmt = this.outbox.db.prepare(`
      UPDATE _ves_conflicts
      SET status = 'resolved',
          resolved_at = datetime('now'),
          resolution_strategy = ?,
          resolution_data = ?
      WHERE id = ?
    `);

    stmt.run(strategy, JSON.stringify(result), conflictId);
  }

  /**
   * Skip a conflict (mark as skipped without resolving)
   * @param {string} conflictId
   * @param {string} [reason]
   */
  skipConflict(conflictId, reason) {
    const stmt = this.outbox.db.prepare(`
      UPDATE _ves_conflicts
      SET status = 'skipped',
          resolved_at = datetime('now'),
          resolution_data = ?
      WHERE id = ?
    `);

    stmt.run(JSON.stringify({ skipped: true, reason }), conflictId);
  }

  /**
   * Resolve all conflicts with default/suggested strategies
   * @returns {Promise<{resolved: number, failed: number, errors: Array}>}
   */
  async resolveAll() {
    const conflicts = this.getUnresolvedConflicts();
    let resolved = 0;
    let failed = 0;
    const errors = [];

    for (const conflict of conflicts) {
      const result = await this.resolve(conflict, conflict.suggestedStrategy);
      if (result.success) {
        resolved++;
      } else {
        failed++;
        errors.push({ conflictId: conflict.id, error: result.error });
      }
    }

    return { resolved, failed, errors };
  }

  /**
   * Convert database row to ConflictInfo
   * @private
   */
  _rowToConflict(row) {
    return {
      id: row.id,
      type: row.conflict_type,
      localEvent: row.local_seq ? this.outbox._rowToEvent(row) : null,
      remoteEvent: row.remote_event_seq ? { sequenceNumber: row.remote_event_seq } : null,
      entityType: row.entity_type,
      entityId: row.entity_id,
      description: row.description,
      suggestedStrategy: row.suggested_strategy,
      status: row.status,
      detectedAt: new Date(row.detected_at),
      resolvedAt: row.resolved_at ? new Date(row.resolved_at) : null,
      resolutionStrategy: row.resolution_strategy,
      resolutionData: row.resolution_data ? JSON.parse(row.resolution_data) : null,
    };
  }
}

/**
 * Create a conflict resolver
 * @param {import('./outbox.js').Outbox} outbox
 * @param {Object} [options]
 * @returns {ConflictResolver}
 */
export function createConflictResolver(outbox, options = {}) {
  return new ConflictResolver(outbox, options);
}
