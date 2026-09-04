/**
 * Durable sequencer-to-kernel bridge for autonomous commerce.
 *
 * The sequencer is an ordered message transport. It never grants commerce
 * authority. This bridge verifies each marketplace message, derives commands
 * from trusted local configuration, and submits those commands to the embedded
 * kernel under operator-owned policy.
 */

import crypto from 'node:crypto';

const AWARD_EVENT = 'marketplace.award.created';
const MARKETPLACE_ENTITY = 'marketplace.negotiation';
const MARKETPLACE_PROTOCOL = 'stateset.marketplace.v1';
const TERMINAL = new Set(['completed', 'rejected']);

function requiredString(value, field) {
  if (typeof value !== 'string' || value.trim() !== value || value.length === 0) {
    throw new Error(`${field} must be a non-empty, trimmed string`);
  }
  return value;
}

function positiveDecimal(value, field) {
  requiredString(value, field);
  if (!/^(?:0|[1-9]\d*)(?:\.\d+)?$/.test(value) || /^0(?:\.0+)?$/.test(value)) {
    throw new Error(`${field} must be a positive exact decimal string`);
  }
  return value;
}

function canonicalize(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalize).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalize(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

export function canonicalMarketplaceMessage(message) {
  const unsigned = { ...message };
  delete unsigned.signature;
  return canonicalize(unsigned);
}

export function signMarketplaceMessage(message, privateKey, keyId = 'default') {
  const signature = crypto.sign(
    null,
    Buffer.from(canonicalMarketplaceMessage(message)),
    privateKey,
  );
  return {
    ...message,
    signature: { scheme: 'ed25519', key_id: keyId, value: signature.toString('hex') },
  };
}

function normalizeEd25519PublicKey(publicKey) {
  if (typeof publicKey === 'string' && /^[0-9a-f]{64}$/i.test(publicKey)) {
    return crypto.createPublicKey({
      key: Buffer.concat([
        Buffer.from('302a300506032b6570032100', 'hex'),
        Buffer.from(publicKey, 'hex'),
      ]),
      format: 'der',
      type: 'spki',
    });
  }
  if (Buffer.isBuffer(publicKey) && publicKey.length === 32) {
    return normalizeEd25519PublicKey(publicKey.toString('hex'));
  }
  return publicKey;
}

export function verifyMarketplaceMessage(message, publicKey) {
  const signature = message?.signature;
  if (
    signature?.scheme !== 'ed25519' ||
    typeof signature.value !== 'string' ||
    !/^[0-9a-f]{128}$/i.test(signature.value)
  ) {
    return false;
  }
  try {
    return crypto.verify(
      null,
      Buffer.from(canonicalMarketplaceMessage(message)),
      normalizeEd25519PublicKey(publicKey),
      Buffer.from(signature.value, 'hex'),
    );
  } catch {
    return false;
  }
}

function eventFields(sequenced) {
  const envelope = sequenced?.envelope ?? sequenced;
  return {
    eventId: envelope.eventId ?? envelope.event_id,
    tenantId: envelope.tenantId ?? envelope.tenant_id,
    storeId: envelope.storeId ?? envelope.store_id,
    entityType: envelope.entityType ?? envelope.entity_type,
    entityId: envelope.entityId ?? envelope.entity_id,
    eventType: envelope.eventType ?? envelope.event_type,
    sourceAgent: envelope.sourceAgent ?? envelope.source_agent,
    createdAt: envelope.createdAt ?? envelope.created_at,
    payload: envelope.payload,
    sequenceNumber: sequenced.sequenceNumber ?? envelope.sequence_number,
  };
}

function eventDigest(event) {
  return crypto.createHash('sha256').update(canonicalize(event)).digest('hex');
}

function deterministicUuid(namespace) {
  const bytes = crypto.createHash('sha256').update(namespace).digest().subarray(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function commandEnvelope({
  event,
  identity,
  policyVersion,
  commandType,
  suffix,
  payload,
  commitment,
}) {
  const key = `sequencer:${event.eventId}:${suffix}`;
  return {
    contract_version: '1.0',
    command_id: deterministicUuid(key),
    idempotency_key: key,
    command_type: commandType,
    principal: {
      id: identity.id,
      kind: 'agent',
      tenant_id: identity.tenantId,
      delegated_by: identity.principalId,
      capabilities: [...identity.capabilities],
    },
    store_id: identity.storeId,
    correlation_id: null,
    causation_id: /^[0-9a-f-]{36}$/i.test(event.eventId) ? event.eventId : null,
    expected_version: null,
    policy_version: policyVersion,
    approval: null,
    authority: null,
    mandate: identity.mandate ?? null,
    commitment,
    deadline: event.payload.expires_at ?? null,
    trace_id: `marketplace:${event.entityId}`,
    mode: 'apply',
    payload,
    issued_at: event.createdAt,
  };
}

/**
 * Build the commands one local economic actor should execute for an award.
 * Buyer and merchant runtimes intentionally execute different commands under
 * different identities and policies.
 */
export function createAwardCommandPlanner({ side, network = 'set_chain', escrowSeconds = 86_400 }) {
  if (!['buyer', 'merchant'].includes(side)) throw new Error('side must be buyer or merchant');

  return async ({ event, identity, policy }) => {
    const award = event.payload;
    const commitment = award.commitment;
    const amount = commitment?.amount;
    positiveDecimal(amount?.amount, 'award.commitment.amount.amount');
    requiredString(amount?.currency, 'award.commitment.amount.currency');
    positiveDecimal(commitment?.quantity, 'award.commitment.quantity');
    requiredString(commitment?.asset, 'award.commitment.asset');
    requiredString(commitment?.counterparty_id, 'award.commitment.counterparty_id');
    if (award.winner !== commitment.counterparty_id) {
      throw new Error('award winner must equal the committed counterparty');
    }

    if (side === 'merchant') {
      if (![award.winner, ...(award.to ?? [])].includes(identity.id)) return [];
      return [
        commandEnvelope({
          event,
          identity,
          policyVersion: policy.version,
          commandType: 'inventory.reserve',
          suffix: 'merchant-reservation',
          commitment: {
            budget_id: null,
            amount: null,
            asset_amount: null,
            counterparty_id: event.sourceAgent,
            quantity: commitment.quantity,
            evidence: [event.eventId, award.bid_id],
          },
          payload: {
            sku: commitment.asset,
            location_id: null,
            quantity: commitment.quantity,
            reference_type: 'marketplace_award',
            reference_id: event.eventId,
            expires_in_seconds: escrowSeconds,
          },
        }),
      ];
    }

    if (event.sourceAgent !== identity.id) return [];
    const settlement = award.settlement;
    const assetAmount = settlement?.asset_amount;
    positiveDecimal(assetAmount?.amount, 'award.settlement.asset_amount.amount');
    requiredString(assetAmount?.asset, 'award.settlement.asset_amount.asset');
    if (assetAmount.amount !== amount.amount) {
      throw new Error('settlement amount must exactly equal the awarded amount');
    }
    const buyerAddress = requiredString(
      settlement?.buyer_address,
      'award.settlement.buyer_address',
    );
    const sellerAddress = requiredString(
      settlement?.seller_address,
      'award.settlement.seller_address',
    );
    return [
      commandEnvelope({
        event,
        identity,
        policyVersion: policy.version,
        commandType: 'a2a.escrow.create',
        suffix: 'buyer-escrow',
        commitment: {
          budget_id: null,
          amount: null,
          asset_amount: assetAmount,
          counterparty_id: commitment.counterparty_id,
          quantity: commitment.quantity,
          evidence: [event.eventId, award.bid_id],
        },
        payload: {
          quote_id: award.bid_id,
          payment_id: null,
          buyer_address: buyerAddress,
          seller_address: sellerAddress,
          amount: assetAmount.amount,
          asset: assetAmount.asset,
          network: settlement.network ?? network,
          release_conditions: [
            { kind: 'inventory_reserved', award_id: event.eventId },
            { kind: 'delivery_confirmed', conversation_id: award.conversation_id },
          ],
          expires_at: award.expires_at,
          auto_release_after: null,
          metadata: {
            marketplace_event_id: event.eventId,
            conversation_id: award.conversation_id,
          },
        },
      }),
    ];
  };
}

/** In-memory state adapter useful for ephemeral sandboxes and tests. */
export class MemoryBridgeStore {
  constructor() {
    this.cursors = new Map();
    this.events = new Map();
  }

  getCursor(bridgeId) {
    return this.cursors.get(bridgeId) ?? 1;
  }

  claim(bridgeId, eventId, sequenceNumber, digest) {
    const key = `${bridgeId}:${eventId}`;
    const current = this.events.get(key);
    if (current && current.digest !== digest)
      throw new Error(`event ${eventId} changed after receipt`);
    if (current && TERMINAL.has(current.status)) return { execute: false, record: current };
    const record = {
      eventId,
      sequenceNumber,
      digest,
      status: 'processing',
      attempts: (current?.attempts ?? 0) + 1,
    };
    this.events.set(key, record);
    return { execute: true, record };
  }

  complete(bridgeId, eventId, status, result) {
    const key = `${bridgeId}:${eventId}`;
    this.events.set(key, { ...this.events.get(key), status, result });
  }

  fail(bridgeId, eventId, error) {
    const key = `${bridgeId}:${eventId}`;
    this.events.set(key, { ...this.events.get(key), status: 'failed', error: error.message });
  }

  advance(bridgeId, nextSequence) {
    this.cursors.set(bridgeId, nextSequence);
  }
}

/** Durable inbox/cursor state backed by a caller-owned better-sqlite3 handle. */
export class SqliteBridgeStore {
  constructor(db) {
    if (typeof db?.prepare !== 'function' || typeof db?.transaction !== 'function') {
      throw new Error('SqliteBridgeStore requires a better-sqlite3 Database');
    }
    this.db = db;
    db.exec(`
      CREATE TABLE IF NOT EXISTS _stateset_marketplace_bridge_cursors (
        bridge_id TEXT PRIMARY KEY,
        next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1),
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS _stateset_marketplace_bridge_inbox (
        bridge_id TEXT NOT NULL,
        event_id TEXT NOT NULL,
        sequence_number INTEGER NOT NULL,
        event_digest TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('processing', 'failed', 'completed', 'rejected')),
        attempts INTEGER NOT NULL DEFAULT 0,
        result_json TEXT,
        last_error TEXT,
        updated_at TEXT NOT NULL,
        PRIMARY KEY (bridge_id, event_id)
      );
      CREATE UNIQUE INDEX IF NOT EXISTS idx_marketplace_bridge_sequence
        ON _stateset_marketplace_bridge_inbox (bridge_id, sequence_number);
    `);
    this.claimTransaction = db.transaction((bridgeId, eventId, sequenceNumber, digest) => {
      const existing = db
        .prepare(
          `SELECT event_digest AS digest, status, attempts, result_json AS resultJson
             FROM _stateset_marketplace_bridge_inbox
            WHERE bridge_id = ? AND event_id = ?`,
        )
        .get(bridgeId, eventId);
      if (existing && existing.digest !== digest) {
        throw new Error(`event ${eventId} changed after receipt`);
      }
      if (existing && TERMINAL.has(existing.status)) {
        return {
          execute: false,
          record: {
            ...existing,
            result: existing.resultJson ? JSON.parse(existing.resultJson) : null,
          },
        };
      }
      const now = new Date().toISOString();
      db.prepare(
        `INSERT INTO _stateset_marketplace_bridge_inbox
           (bridge_id, event_id, sequence_number, event_digest, status, attempts, updated_at)
         VALUES (?, ?, ?, ?, 'processing', 1, ?)
         ON CONFLICT (bridge_id, event_id) DO UPDATE SET
           status = 'processing', attempts = attempts + 1, last_error = NULL,
           updated_at = excluded.updated_at`,
      ).run(bridgeId, eventId, sequenceNumber, digest, now);
      return {
        execute: true,
        record: {
          eventId,
          sequenceNumber,
          digest,
          status: 'processing',
          attempts: (existing?.attempts ?? 0) + 1,
        },
      };
    });
  }

  getCursor(bridgeId) {
    return (
      this.db
        .prepare(
          'SELECT next_sequence AS nextSequence FROM _stateset_marketplace_bridge_cursors WHERE bridge_id = ?',
        )
        .get(bridgeId)?.nextSequence ?? 1
    );
  }

  claim(bridgeId, eventId, sequenceNumber, digest) {
    return this.claimTransaction(bridgeId, eventId, sequenceNumber, digest);
  }

  complete(bridgeId, eventId, status, result) {
    this.db
      .prepare(
        `UPDATE _stateset_marketplace_bridge_inbox
            SET status = ?, result_json = ?, last_error = NULL, updated_at = ?
          WHERE bridge_id = ? AND event_id = ?`,
      )
      .run(status, JSON.stringify(result), new Date().toISOString(), bridgeId, eventId);
  }

  fail(bridgeId, eventId, error) {
    this.db
      .prepare(
        `UPDATE _stateset_marketplace_bridge_inbox
            SET status = 'failed', last_error = ?, updated_at = ?
          WHERE bridge_id = ? AND event_id = ?`,
      )
      .run(error.message, new Date().toISOString(), bridgeId, eventId);
  }

  advance(bridgeId, nextSequence) {
    this.db
      .prepare(
        `INSERT INTO _stateset_marketplace_bridge_cursors (bridge_id, next_sequence, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT (bridge_id) DO UPDATE SET
           next_sequence = MAX(next_sequence, excluded.next_sequence),
           updated_at = excluded.updated_at`,
      )
      .run(bridgeId, nextSequence, new Date().toISOString());
  }
}

/**
 * Ordered, fail-closed marketplace consumer.
 *
 * `verify` must authenticate the application-level message signature against
 * an operator-owned registry. A sequencer receipt proves ordering, not agent
 * authority, so verification is mandatory even on authenticated transports.
 */
export class KernelMarketplaceBridge {
  constructor({
    id,
    sequencer,
    commerce,
    store = new MemoryBridgeStore(),
    identity,
    policy,
    registry,
    planner,
    publishReceipt,
    batchSize = 100,
  }) {
    this.id = requiredString(id, 'bridge.id');
    this.sequencer = sequencer;
    this.commerce = commerce;
    this.store = store;
    this.identity = identity;
    this.policy = policy;
    this.registry = registry instanceof Map ? registry : new Map(Object.entries(registry ?? {}));
    this.planner = planner;
    this.publishReceipt = publishReceipt;
    this.batchSize = batchSize;
    if (typeof sequencer?.pull !== 'function') throw new Error('sequencer.pull is required');
    if (typeof commerce?.executeKernelCommand !== 'function') {
      throw new Error('commerce.executeKernelCommand is required');
    }
    if (typeof planner !== 'function') throw new Error('planner is required');
    if (typeof publishReceipt !== 'function') throw new Error('publishReceipt is required');
    for (const field of ['id', 'principalId', 'tenantId', 'storeId']) {
      requiredString(identity?.[field], `identity.${field}`);
    }
    if (!Array.isArray(identity.capabilities))
      throw new Error('identity.capabilities must be an array');
    requiredString(policy?.version, 'policy.version');
  }

  authenticate(event) {
    if (event.tenantId !== this.identity.tenantId || event.storeId !== this.identity.storeId) {
      throw new Error('event is outside the bridge tenant/store scope');
    }
    const actor = this.registry.get(event.sourceAgent);
    const keyId = event.payload?.signature?.key_id;
    const publicKey =
      actor?.keys instanceof Map
        ? actor.keys.get(keyId)
        : (actor?.keys?.[keyId] ?? actor?.publicKey);
    if (!publicKey)
      throw new Error(`unregistered marketplace agent key ${event.sourceAgent}:${keyId}`);
    if (event.payload?.from !== actor.name && event.payload?.from !== actor.id) {
      throw new Error('message sender does not match the registered source agent');
    }
    if (
      event.payload?.message_id !== event.eventId ||
      event.payload?.conversation_id !== event.entityId ||
      event.payload?.sent_at !== event.createdAt ||
      event.payload?.kind !== 'award'
    ) {
      throw new Error('marketplace message does not bind its sequencer envelope');
    }
    const expiry = Date.parse(event.payload?.expires_at ?? '');
    if (!Number.isFinite(expiry) || expiry <= Date.now()) {
      throw new Error('marketplace award is missing an unexpired deadline');
    }
    if (!verifyMarketplaceMessage(event.payload, publicKey)) {
      throw new Error('invalid marketplace message signature');
    }
  }

  async process(sequenced) {
    const event = eventFields(sequenced);
    const sequence = Number(event.sequenceNumber);
    if (!Number.isSafeInteger(sequence) || sequence < 1)
      throw new Error('invalid sequencer position');
    requiredString(event.eventId, 'event.event_id');

    if (event.entityType !== MARKETPLACE_ENTITY || event.eventType !== AWARD_EVENT) {
      return { sequence, status: 'ignored', eventId: event.eventId };
    }
    if (event.payload?.protocol !== MARKETPLACE_PROTOCOL) {
      throw new Error('unsupported marketplace protocol');
    }
    const digest = eventDigest(event);
    const claim = this.store.claim(this.id, event.eventId, sequence, digest);
    if (!claim.execute) return { sequence, status: claim.record.status, eventId: event.eventId };

    try {
      this.authenticate(event);
      const commands = await this.planner({
        event,
        identity: this.identity,
        policy: this.policy,
      });
      const receipts = [];
      for (const command of commands) {
        if (
          command.principal?.id !== this.identity.id ||
          command.principal?.tenant_id !== this.identity.tenantId ||
          command.store_id !== this.identity.storeId
        ) {
          throw new Error('planner produced a command outside the trusted identity scope');
        }
        const receipt = await this.commerce.executeKernelCommand(command, this.policy);
        if (!['succeeded', 'rejected'].includes(receipt?.status)) {
          throw new Error(
            `kernel returned non-terminal apply status ${receipt?.status ?? 'unknown'}`,
          );
        }
        receipts.push(receipt);
        await this.publishReceipt({
          publicationId: deterministicUuid(`marketplace-receipt:${command.idempotency_key}`),
          source: event,
          command,
          receipt,
        });
      }
      const status = receipts.some((receipt) => receipt.status === 'rejected')
        ? 'rejected'
        : 'completed';
      const result = { commands: commands.length, receipts };
      this.store.complete(this.id, event.eventId, status, result);
      return { sequence, status, eventId: event.eventId, ...result };
    } catch (error) {
      this.store.fail(this.id, event.eventId, error);
      throw error;
    }
  }

  async pollOnce() {
    let cursor = this.store.getCursor(this.id);
    const from = cursor;
    const batch = await this.sequencer.pull(cursor, this.batchSize);
    const events = [...(batch.events ?? [])].sort(
      (left, right) => eventFields(left).sequenceNumber - eventFields(right).sequenceNumber,
    );
    const outcomes = [];
    for (const event of events) {
      const observed = Number(eventFields(event).sequenceNumber);
      if (observed !== cursor) {
        throw new Error(`sequencer gap: expected ${cursor}, received ${observed}`);
      }
      const outcome = await this.process(event);
      outcomes.push(outcome);
      cursor = outcome.sequence + 1;
      this.store.advance(this.id, cursor);
    }
    return { from, nextSequence: cursor, outcomes };
  }
}
