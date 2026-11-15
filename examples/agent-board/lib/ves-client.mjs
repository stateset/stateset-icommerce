/**
 * The CLI's SequencerClient, with its READ methods pointed at the VES surface.
 *
 * Everything that works against the real sequencer is inherited unchanged:
 * `push` (POST /api/v1/ves/events/ingest), `registerAgentKey`
 * (POST /api/v1/agents/keys), `getAgentSigningKeys` (signed key directory),
 * `verifyEventSignature`, `verifyInclusionDetailed`, the auth headers, the
 * request deadline and the lifecycle abort.
 *
 * The overrides exist because the inherited read methods do not read the VES
 * stream at all (checked against stateset-sequencer `src/api/mod.rs` and
 * `src/api/handlers/ves/events.rs`):
 *
 *  - `pull()` calls GET /api/v1/events and `getHead()` GET /api/v1/head. Those
 *    are the deprecated pre-VES routes; they read the legacy `events` table,
 *    never `ves_events`, so events ingested through /api/v1/ves/events/ingest
 *    are invisible to them. The VES routes are /api/v1/ves/events and
 *    /api/v1/ves/head.
 *  - `pull()` maps `envelope.source_agent`; the VES envelope field is
 *    `source_agent_id`, so `sourceAgent` comes back undefined and no signature
 *    can verify. It also reads `sequenced_at` and `receipt_hash` from the
 *    record, where VES puts `sequenced_at` inside the envelope and serves no
 *    receipt hash; and it fills a missing signature with zeros.
 *  - `getEntityHistory()` calls GET /api/v1/entities/... (legacy table) and
 *    maps flat records, while GET /api/v1/ves/entities/... returns
 *    `{ envelope: {...} }` records, paged 100 at a time. It also does not
 *    URL-encode the entity id.
 *  - There is no REST method for commitments or inclusion proofs on the VES
 *    surface (GET /api/v1/ves/commitments, GET /api/v1/ves/proofs/:seq).
 *
 * Each override returns exactly the shape the inherited method documents, so
 * the rest of the CLI's sync code (validatePullPage, verifyEventSignature,
 * verifyInclusionDetailed) consumes it unchanged.
 */

import { SequencerClient } from '../../../cli/src/sync/client.js';
import { validatePullPage, validatePullRequest } from '../../../cli/src/sync/pull-page.js';

const HASH32 = /^(?:0x)?[0-9a-fA-F]{64}$/;
const ENTITY_PAGE = 100; // MAX_ENTITY_HISTORY_PAGE in stateset-sequencer src/domain/mod.rs
const MAX_HISTORY_EVENTS = 100_000;

/** HTTP status of a SequencerClient request failure, or null. */
export function requestStatus(error) {
  const match = /Sequencer request failed: (\d{3})\b/.exec(error?.message ?? '');
  return match ? Number(match[1]) : null;
}

function signatureBundleFromWire(bundle) {
  if (!bundle) return null;
  return {
    ed25519Signature: bundle.ed25519_signature ?? bundle.ed25519Signature ?? null,
    mlDsa65Signature: bundle.ml_dsa_65_signature ?? bundle.mlDsa65Signature ?? null,
  };
}

/**
 * Map one VES wire record (`SequencedVesEvent`: `{ envelope: VesEventEnvelope }`
 * serialized by serde, snake_case) to the CLI's SequencedEvent shape.
 *
 * Missing signature material stays missing -- it is never defaulted -- so an
 * incomplete record fails verification instead of carrying a placeholder.
 *
 * @param {any} record
 */
export function sequencedEventFromVesWire(record) {
  const env = record?.envelope;
  if (!env || typeof env !== 'object') {
    throw new Error('Invalid VES event record: no envelope');
  }
  return {
    envelope: {
      eventId: env.event_id,
      commandId: env.command_id ?? null,
      tenantId: env.tenant_id,
      storeId: env.store_id,
      entityType: env.entity_type,
      entityId: env.entity_id,
      eventType: env.event_type,
      payload: env.payload,
      vesVersion: env.ves_version ?? 1,
      payloadKind: env.payload_kind ?? 0,
      payloadEncrypted: env.payload_encrypted ?? null,
      payloadPlainHash: env.payload_plain_hash,
      payloadCipherHash: env.payload_cipher_hash,
      agentKeyId: env.agent_key_id,
      agentSignature: env.agent_signature,
      agentSignatureScheme: env.agent_signature_scheme ?? 0,
      agentSignatureBundle: signatureBundleFromWire(env.agent_signature_bundle),
      baseVersion: env.base_version ?? null,
      createdAt: env.created_at,
      sourceAgent: env.source_agent_id,
    },
    sequenceNumber: env.sequence_number,
    sequencedAt: env.sequenced_at ?? null,
  };
}

function normalizeRoot(value, label) {
  if (typeof value !== 'string' || !HASH32.test(value)) {
    throw new Error(`Invalid ${label}: expected a 32-byte hex hash`);
  }
  return value.replace(/^0x/, '').toLowerCase();
}

/** Map a VES commitment record (handlers/ves/commitments.rs) to camelCase. */
export function commitmentFromWire(record) {
  return {
    batchId: record.batch_id,
    merkleRoot: normalizeRoot(record.merkle_root, 'merkle_root'),
    leafCount: record.leaf_count,
    paddedLeafCount: record.padded_leaf_count,
    treeDepth: record.tree_depth,
    sequenceStart: record.sequence_start,
    sequenceEnd: record.sequence_end,
    committedAt: record.committed_at,
    isAnchored: record.is_anchored === true,
  };
}

export class BoardSequencerClient extends SequencerClient {
  _scope(extra = {}) {
    return new URLSearchParams({
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      ...extra,
    });
  }

  /**
   * GET /api/v1/ves/events -- one bounded page of the store's VES stream.
   * Same contract as SequencerClient#pull. `from` is inclusive and >= 1.
   */
  async pull(fromSequence, limit = 100) {
    validatePullRequest(fromSequence, limit);
    if (fromSequence < 1) throw new Error('VES sequence numbers start at 1');
    const params = this._scope({ from: String(fromSequence), limit: String(limit) });
    const response = await this._request('GET', `/api/v1/ves/events?${params}`);
    const events = (response?.events ?? []).map(sequencedEventFromVesWire);
    const max = events.reduce((m, e) => Math.max(m, e.sequenceNumber), fromSequence - 1);
    const page = {
      events,
      nextSequence: events.length ? max + 1 : fromSequence,
      hasMore: response?.has_more === true,
      headSequence: response?.head_sequence,
    };
    if (
      events.length &&
      response?.next_sequence !== undefined &&
      response.next_sequence !== page.nextSequence
    ) {
      throw new Error('Invalid pull page: next_sequence disagrees with the events served');
    }
    validatePullPage(page, fromSequence, limit);
    return page;
  }

  /** GET /api/v1/ves/head. Same contract as SequencerClient#getHead. */
  async getHead() {
    const response = await this._request('GET', `/api/v1/ves/head?${this._scope()}`);
    if (!Number.isSafeInteger(response?.head_sequence) || response.head_sequence < 0) {
      throw new Error('Invalid remote head: expected a safe nonnegative sequence number');
    }
    return {
      tenantId: this.config.tenantId,
      storeId: this.config.storeId,
      headSequence: response.head_sequence,
    };
  }

  /**
   * GET /api/v1/ves/entities/:type/:id, every page, in sequence order.
   * Same contract as SequencerClient#getEntityHistory.
   */
  async getEntityHistory(entityType, entityId) {
    const path = `/api/v1/ves/entities/${encodeURIComponent(entityType)}/${encodeURIComponent(entityId)}`;
    const events = [];
    let offset = 0;
    for (;;) {
      const params = this._scope({ from: String(offset), limit: String(ENTITY_PAGE) });
      const response = await this._request('GET', `${path}?${params}`);
      const page = (response?.events ?? []).map(sequencedEventFromVesWire);
      events.push(...page);
      offset += page.length;
      if (response?.has_more !== true || page.length === 0) break;
      if (offset > MAX_HISTORY_EVENTS) throw new Error('Entity history too long to read');
    }
    let previous = 0;
    for (const event of events) {
      if (!Number.isSafeInteger(event.sequenceNumber) || event.sequenceNumber <= previous) {
        throw new Error('Invalid entity history: sequence numbers must strictly increase');
      }
      previous = event.sequenceNumber;
    }
    return events;
  }

  /** GET /api/v1/ves/commitments for this store. */
  async listCommitments() {
    const response = await this._request('GET', `/api/v1/ves/commitments?${this._scope()}`);
    return (response?.commitments ?? []).map(commitmentFromWire);
  }

  /** POST /api/v1/ves/commitments over [sequenceStart, sequenceEnd] (write permission). */
  async createCommitment(sequenceStart, sequenceEnd) {
    const response = await this._request('POST', '/api/v1/ves/commitments', {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      sequence_start: sequenceStart,
      sequence_end: sequenceEnd,
    });
    return commitmentFromWire(response);
  }

  /**
   * GET /api/v1/ves/proofs/:seq. The response carries no leaf count; callers
   * bind the tree size from the commitment record instead (see board.mjs).
   */
  async getInclusionProof(sequenceNumber, batchId) {
    const params = this._scope({ batch_id: batchId });
    const response = await this._request('GET', `/api/v1/ves/proofs/${sequenceNumber}?${params}`);
    return {
      sequenceNumber: response?.sequence_number,
      batchId: response?.batch_id,
      merkleRoot: response?.merkle_root,
      leafHash: response?.leaf_hash,
      leafIndex: response?.leaf_index,
      proofHashes: response?.proof_path,
      directions: response?.directions,
    };
  }
}
