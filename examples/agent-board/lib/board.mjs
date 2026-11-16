/**
 * The message board: post signed board events, read them back verified.
 *
 * A board message is one VES event:
 *
 *   entityType  'board_thread'
 *   entityId    the thread id
 *   eventType   'board.post'
 *   payload     { author, text, replyTo?, refs?, state? }
 *
 * Posting goes through the CLI's outbox (`Outbox#append` signs it with the
 * agent's Ed25519 key) and `SequencerClient#push`. Reading goes through
 * `BoardSequencerClient` (the CLI client with its read methods pointed at the
 * VES routes), then every event is checked with the CLI's own receive-side
 * machinery: `PeerKeyDirectory` (signed key directory via
 * `getAgentSigningKeys`, TTL cache, trust-on-first-use pins),
 * `verifyEventSignature`, and, where the sequencer has committed the range,
 * `verifyInclusionDetailed` with the tree size taken from the commitment.
 */

import fs from 'node:fs';
import { randomUUID } from 'node:crypto';

import { createOutbox } from '../../../cli/src/sync/outbox.js';
import { createPeerKeyDirectory } from '../../../cli/src/sync/key-directory.js';
import { verifyPayloadIntegrity } from '../../../cli/src/sync/payload-integrity.js';
import { assertOutgoingScope, matchesReceiveScope } from '../../../cli/src/sync/receive-scope.js';
import { normalizeSequencerPublicKey } from '../../../cli/src/sync/config.js';
import { KEY_ALGORITHM_ED25519 } from '../../../cli/src/sync/pqc.js';
import { clientConfig } from './config.mjs';
import { loadDatabase } from './deps.mjs';
import { BoardSequencerClient, requestStatus } from './ves-client.mjs';

export const ENTITY_TYPE = 'board_thread';
export const EVENT_TYPE = 'board.post';

// The VES read route caps entity_id at 256 characters; keep threads well inside.
const THREAD_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const MAX_TEXT = 8000;
const MAX_REFS = 32;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Key-resolution failures a trusted roster may stand in for. */
const ROSTER_FALLBACK = new Set(['key_unresolved', 'sequencer_key_not_configured']);

export function validateThreadId(thread) {
  if (typeof thread !== 'string' || !THREAD_ID.test(thread)) {
    throw new Error(
      'Thread id must be 1-128 characters of letters, digits, ".", "_", ":" or "-", starting with a letter or digit',
    );
  }
  return thread;
}

/**
 * @param {{ author: string, text: string, replyTo?: number, refs?: string[], state?: unknown }} input
 */
export function buildPayload({ author, text, replyTo, refs, state }) {
  if (typeof text !== 'string' || !text.trim()) throw new Error('Message text must not be empty');
  if (text.length > MAX_TEXT) throw new Error(`Message text is longer than ${MAX_TEXT} characters`);
  const payload = { author, text };
  if (replyTo !== undefined) {
    if (!Number.isSafeInteger(replyTo) || replyTo < 1) {
      throw new Error('--reply-to must be a sequence number (a positive integer)');
    }
    payload.replyTo = replyTo;
  }
  if (refs !== undefined && refs.length) {
    if (refs.length > MAX_REFS) throw new Error(`At most ${MAX_REFS} --ref values`);
    for (const ref of refs) {
      if (typeof ref !== 'string' || !ref || ref.length > 512) {
        throw new Error('--ref values must be 1-512 characters');
      }
    }
    payload.refs = refs;
  }
  if (state !== undefined) payload.state = state;
  return payload;
}

export function createClient(settings, agentId = null) {
  return new BoardSequencerClient(clientConfig(settings, agentId));
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/**
 * POST /api/v1/agents/keys for the agent's current signing key. On the real
 * sequencer this is an ADMIN operation (`ensure_admin` in
 * src/api/handlers/agent_keys.rs), and registering the same
 * (tenant, agent, key id) twice fails with HTTP 500.
 */
export async function registerKey({ agent, settings }) {
  const client = createClient(settings, agent.manifest.agentId);
  const result = await client.registerAgentKey({
    agentId: agent.manifest.agentId,
    keyId: agent.signingKey.keyId,
    keyAlgorithm: agent.manifest.keyAlgorithm ?? KEY_ALGORITHM_ED25519,
    publicKey: agent.signingKey.publicKey,
  });
  if (!result.success) throw new Error('The sequencer did not accept the key registration');
  return { agentId: agent.manifest.agentId, keyId: agent.signingKey.keyId };
}

// ---------------------------------------------------------------------------
// Posting
// ---------------------------------------------------------------------------

/** Outbox row -> push envelope, exactly as SyncEngine#push builds it. */
function toPushEnvelope(e) {
  return {
    eventId: e.eventId,
    commandId: e.commandId,
    tenantId: e.tenantId,
    storeId: e.storeId,
    entityType: e.entityType,
    entityId: e.entityId,
    eventType: e.eventType,
    payload: e.payload,
    vesVersion: e.vesVersion || 1,
    payloadKind: e.payloadKind || 0,
    payloadEncrypted: e.payloadEncrypted,
    payloadPlainHash: e.payloadPlainHash,
    payloadCipherHash: e.payloadCipherHash,
    agentKeyId: e.agentKeyId,
    agentSignature: e.agentSignature,
    agentSignatureScheme: e.agentSignatureScheme || 0,
    agentSignatureBundle: e.agentSignatureBundle || null,
    baseVersion: e.baseVersion,
    createdAt: e.createdAtRaw ?? e.createdAt.toISOString(),
    sourceAgent: e.sourceAgent,
  };
}

/**
 * Settle every submitted event against the ingest response.
 *
 * Deliberately NOT the CLI's `validatePushReceipt`, which does not fit the
 * real response (stateset-sequencer src/api/types.rs, ves_sequencer.rs):
 *  - rejections are serialized `{ event_id, reason, message }` (snake_case
 *    inside a camelCase body); validatePushReceipt reads `rejection.eventId`,
 *    so any rejection makes it throw instead of recording the rejection;
 *  - an exact replay of an already-sequenced event (a retry after a lost
 *    response) returns its stored receipt but is counted in neither
 *    eventsAccepted nor eventsRejected, so accepted + rejected != submitted.
 * Per-event receipts are authoritative here: each submitted event must have
 * exactly one receipt or exactly one rejection.
 */
export function settlePush(response, pending) {
  const submitted = new Map(pending.map((event) => [event.eventId, event]));
  const fail = (why) => {
    throw new Error(`Invalid ingest response: ${why}`);
  };
  const sequenced = new Map();
  for (const receipt of response.receipts ?? []) {
    const eventId = receipt?.eventId ?? receipt?.event_id;
    const sequenceNumber = receipt?.sequenceNumber ?? receipt?.sequence_number;
    if (!submitted.has(eventId)) fail(`receipt for an event that was not submitted (${eventId})`);
    if (sequenced.has(eventId)) fail(`two receipts for event ${eventId}`);
    if (!Number.isSafeInteger(sequenceNumber) || sequenceNumber < 1) {
      fail(`receipt for ${eventId} has no valid sequence number`);
    }
    sequenced.set(eventId, sequenceNumber);
  }
  const rejected = new Map();
  for (const rejection of response.rejections ?? []) {
    const eventId = rejection?.eventId ?? rejection?.event_id;
    const reason = rejection?.reason;
    if (!submitted.has(eventId)) fail(`rejection for an event that was not submitted (${eventId})`);
    if (rejected.has(eventId) || sequenced.has(eventId)) fail(`event ${eventId} settled twice`);
    if (typeof reason !== 'string' || !reason.trim())
      fail(`rejection for ${eventId} has no reason`);
    rejected.set(eventId, { reason, message: rejection.message ?? null });
  }
  return pending.map((event) => {
    if (sequenced.has(event.eventId)) {
      return { event, status: 'sequenced', sequenceNumber: sequenced.get(event.eventId) };
    }
    if (rejected.has(event.eventId)) {
      return { event, status: 'rejected', ...rejected.get(event.eventId) };
    }
    return fail(`event ${event.eventId} was neither sequenced nor rejected`);
  });
}

/**
 * Push the pending outbox events for the configured tenant/store (this post
 * and any earlier unsent ones). Events stay pending when the request itself
 * fails, so the next post retries them; a per-event rejection is final and
 * recorded on the outbox row. Pending events signed for another tenant/store
 * are left alone until the configuration points there again.
 */
export async function flushOutbox({ agent, client, batchSize = 100 }) {
  const scope = { tenantId: client.config.tenantId, storeId: client.config.storeId };
  const pending = agent.outbox.getPending(batchSize).filter((e) => matchesReceiveScope(e, scope));
  if (!pending.length) return [];
  assertOutgoingScope(pending, client.config);
  const response = await client.push({
    agentId: agent.manifest.agentId,
    events: pending.map(toPushEnvelope),
  });
  const settlements = settlePush(response, pending);
  agent.db
    .transaction(() => {
      const acks = [];
      for (const s of settlements) {
        if (s.status === 'rejected') {
          agent.outbox.markRejected(s.event.localSeq, s.reason);
        } else {
          acks.push({ localSeq: s.event.localSeq, remoteSeq: s.sequenceNumber });
        }
      }
      if (acks.length) agent.outbox.markSynced(acks);
    })
    .immediate();
  return settlements.map((s) => ({
    eventId: s.event.eventId,
    thread: s.event.entityId,
    status: s.status,
    sequenceNumber: s.sequenceNumber ?? null,
    reason: s.reason ?? null,
    message: s.message ?? null,
  }));
}

/**
 * Sign a board message into the outbox and push it.
 *
 * @returns {Promise<{ eventId: string, thread: string, status: 'sequenced'|'rejected'|'pending',
 *   sequenceNumber: number|null, reason: string|null, error?: string, others: object[] }>}
 */
export async function postMessage({ agent, settings, thread, text, replyTo, refs, state }) {
  validateThreadId(thread);
  const payload = buildPayload({ author: agent.manifest.name, text, replyTo, refs, state });
  const eventId = randomUUID();
  await agent.outbox.append({
    eventId,
    commandId: randomUUID(),
    tenantId: settings.tenantId,
    storeId: settings.storeId,
    entityType: ENTITY_TYPE,
    entityId: thread,
    eventType: EVENT_TYPE,
    payload,
    sourceAgent: agent.manifest.agentId,
  });
  const client = createClient(settings, agent.manifest.agentId);
  let settlements;
  try {
    settlements = await flushOutbox({ agent, client });
  } catch (error) {
    return {
      eventId,
      thread,
      status: 'pending',
      sequenceNumber: null,
      reason: null,
      error: error.message,
      others: [],
    };
  }
  const own = settlements.find((s) => s.eventId === eventId) ?? {
    eventId,
    thread,
    status: 'pending',
    sequenceNumber: null,
    reason: null,
    error: 'queued behind earlier unsent posts; post again to flush the rest',
  };
  return { ...own, others: settlements.filter((s) => s.eventId !== eventId) };
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/**
 * A roster: operator-distributed public keys, `{ version: 1, agents: [{ name,
 * agentId, keyId, publicKey }] }`. Used only when the sequencer's signed key
 * directory cannot answer (not deployed, or SEQUENCER_PUBLIC_KEY unset) --
 * never to override a directory finding such as a key conflict.
 */
export function loadRoster(rosterPath) {
  let parsed;
  try {
    parsed = JSON.parse(fs.readFileSync(rosterPath, 'utf8'));
  } catch (error) {
    throw new Error(`Cannot read roster ${rosterPath}: ${error.code ?? 'invalid JSON'}`);
  }
  if (parsed?.version !== 1 || !Array.isArray(parsed.agents)) {
    throw new Error(`Roster ${rosterPath} must be { "version": 1, "agents": [...] }`);
  }
  const entries = new Map();
  const names = new Map();
  for (const agent of parsed.agents) {
    if (!UUID.test(agent?.agentId ?? '') || !Number.isSafeInteger(agent.keyId)) {
      throw new Error(`Roster ${rosterPath} has an entry without a valid agentId and keyId`);
    }
    const agentId = agent.agentId.toLowerCase();
    entries.set(`${agentId}:${agent.keyId}`, normalizeSequencerPublicKey(agent.publicKey));
    if (typeof agent.name === 'string') names.set(agentId, agent.name);
  }
  return {
    publicKeyFor: (agentId, keyId) => entries.get(`${String(agentId).toLowerCase()}:${keyId}`),
    nameFor: (agentId) => names.get(String(agentId).toLowerCase()) ?? null,
  };
}

/** An outbox used only as the peer key cache when no reader agent is given. */
function ephemeralOutbox() {
  const Database = loadDatabase();
  const db = new Database(':memory:');
  const noSigning = { getCurrentSigningKey: async () => null };
  return { outbox: createOutbox(db, { keyManager: noSigning }), close: () => db.close() };
}

function describeKeyFailure(resolution) {
  if (
    resolution.error === 'key_unresolved' &&
    requestStatus({ message: resolution.detail }) === 404
  ) {
    return (
      'the sequencer has no GET /api/v1/agents/:id/signing-keys route ' +
      '(stateset-sequencer PR #6 is not deployed); use --roster'
    );
  }
  return resolution.detail ?? null;
}

async function verifyEvent(record, { client, directory, roster, scope }) {
  const env = record.envelope;
  if (!matchesReceiveScope(env, scope)) {
    return {
      verified: false,
      reason: 'scope_mismatch',
      detail: 'event is for another tenant/store',
    };
  }
  let resolution;
  try {
    resolution = await directory.resolve(env.sourceAgent, env.agentKeyId, env.createdAt);
  } catch (error) {
    resolution = { error: 'key_unresolved', detail: error?.message };
  }
  let keySource = 'directory';
  if (resolution.error && ROSTER_FALLBACK.has(resolution.error) && roster) {
    const publicKey = roster.publicKeyFor(env.sourceAgent, env.agentKeyId);
    if (publicKey) {
      resolution = { publicKey, publicKeyBundle: null };
      keySource = 'roster';
    }
  }
  if (resolution.error) {
    return { verified: false, reason: resolution.error, detail: describeKeyFailure(resolution) };
  }
  if (!verifyPayloadIntegrity(env)) {
    return {
      verified: false,
      reason: 'signature_invalid',
      detail: 'payload does not match its signed payload hash',
      keySource,
    };
  }
  let valid = false;
  try {
    valid = client.verifyEventSignature(env, resolution.publicKeyBundle ?? resolution.publicKey);
  } catch {
    valid = false;
  }
  if (!valid) {
    return {
      verified: false,
      reason: 'signature_invalid',
      detail: `signature does not verify under agent ${env.sourceAgent} key_id ${env.agentKeyId}`,
      keySource,
    };
  }
  return { verified: true, reason: null, detail: null, keySource };
}

async function checkInclusion(record, commitments, client) {
  const seq = record.sequenceNumber;
  const commitment = commitments.find((c) => seq >= c.sequenceStart && seq <= c.sequenceEnd);
  if (!commitment) return { status: 'not_committed' };
  // The tree size is bound by the commitment record, not by the proof.
  const leafCount = commitment.sequenceEnd - commitment.sequenceStart + 1;
  const base = { batchId: commitment.batchId, leafCount, anchored: commitment.isAnchored };
  if (commitment.leafCount !== leafCount) {
    return { status: 'failed', ...base, reason: 'commitment leaf_count disagrees with its range' };
  }
  let proof;
  try {
    proof = await client.getInclusionProof(seq, commitment.batchId);
  } catch (error) {
    return { status: 'unavailable', ...base, reason: error.message };
  }
  if (proof.sequenceNumber !== seq || proof.batchId !== commitment.batchId) {
    return { status: 'failed', ...base, reason: 'proof is for a different event or batch' };
  }
  if (proof.leafIndex !== seq - commitment.sequenceStart) {
    return { status: 'failed', ...base, reason: 'proof leaf index does not match the sequence' };
  }
  let verdict;
  try {
    verdict = client.verifyInclusionDetailed(
      record.envelope,
      { leafIndex: proof.leafIndex, proofHashes: proof.proofHashes },
      commitment.merkleRoot,
      { sequenceNumber: seq, leafCount },
    );
  } catch (error) {
    // A malformed envelope (bad UUID, bad hex) cannot be hashed into a leaf.
    return { status: 'failed', ...base, reason: `cannot compute the leaf: ${error.message}` };
  }
  if (verdict.valid && verdict.pathBound) return { status: 'verified', ...base };
  return { status: 'failed', ...base, reason: verdict.reason ?? 'path does not reach the root' };
}

async function pullAll(client, from) {
  const records = [];
  let cursor = from;
  for (let pages = 0; ; pages += 1) {
    if (pages > 10_000) throw new Error('Stream too long to read in one pass; use --from');
    const page = await client.pull(cursor, 500);
    records.push(...page.events);
    if (!page.hasMore || page.events.length === 0) return { records, head: page.headSequence };
    cursor = page.nextSequence;
  }
}

function toMessage(record, verification, inclusion, roster) {
  const env = record.envelope;
  const payload = env.payload && typeof env.payload === 'object' ? env.payload : {};
  const wellFormed =
    env.eventType === EVENT_TYPE &&
    typeof payload.author === 'string' &&
    typeof payload.text === 'string';
  const signerName = roster?.nameFor(env.sourceAgent) ?? null;
  return {
    sequenceNumber: record.sequenceNumber,
    eventId: env.eventId,
    thread: env.entityId,
    eventType: env.eventType,
    author: wellFormed ? payload.author : null,
    text: wellFormed ? payload.text : null,
    replyTo: payload.replyTo ?? null,
    refs: payload.refs ?? null,
    state: payload.state ?? null,
    wellFormed,
    signer: { agentId: env.sourceAgent, keyId: env.agentKeyId, rosterName: signerName },
    authorMatchesRoster: signerName === null ? null : signerName === payload.author,
    createdAt: env.createdAt,
    sequencedAt: record.sequencedAt,
    verified: verification.verified,
    verification,
    inclusion,
  };
}

/**
 * Read the board back from the sequencer and verify every message.
 *
 * @param {{ settings: object, thread?: string, readerAgent?: object, roster?: object,
 *   proofs?: boolean, from?: number }} options
 */
export async function readBoard({
  settings,
  thread,
  readerAgent,
  roster,
  proofs = true,
  from = 1,
}) {
  if (thread !== undefined) validateThreadId(thread);
  const client = createClient(settings, readerAgent?.manifest.agentId ?? null);
  const scope = { tenantId: settings.tenantId, storeId: settings.storeId };

  let records;
  let head;
  if (thread !== undefined) {
    records = await client.getEntityHistory(ENTITY_TYPE, thread);
    head = (await client.getHead()).headSequence;
  } else {
    ({ records, head } = await pullAll(client, from));
    records = records.filter((r) => r.envelope.entityType === ENTITY_TYPE);
  }
  // Belt and braces: the history route is per entity, but trust nothing.
  records = records
    .filter((r) => r.envelope.entityType === ENTITY_TYPE)
    .filter((r) => thread === undefined || r.envelope.entityId === thread)
    .sort((a, b) => a.sequenceNumber - b.sequenceNumber);

  const cache = readerAgent ? { outbox: readerAgent.outbox, close() {} } : ephemeralOutbox();
  try {
    const directory = createPeerKeyDirectory(cache.outbox, client, client.config);
    let commitments = null;
    let commitmentsError = null;
    if (proofs && records.length) {
      try {
        commitments = await client.listCommitments();
      } catch (error) {
        commitmentsError = error.message;
      }
    }
    const messages = [];
    for (const record of records) {
      const verification = await verifyEvent(record, { client, directory, roster, scope });
      let inclusion;
      if (!proofs) inclusion = { status: 'skipped' };
      else if (commitmentsError) inclusion = { status: 'unavailable', reason: commitmentsError };
      else inclusion = await checkInclusion(record, commitments, client);
      messages.push(toMessage(record, verification, inclusion, roster));
    }
    const verified = messages.filter((m) => m.verified).length;
    return {
      sequencer: settings.url,
      tenantId: settings.tenantId,
      storeId: settings.storeId,
      thread: thread ?? null,
      headSequence: head,
      counts: { total: messages.length, verified, unverified: messages.length - verified },
      messages,
    };
  } finally {
    cache.close();
  }
}

/**
 * Commit the next uncommitted range [lastEnd + 1, head] so inclusion proofs
 * become available (POST /api/v1/ves/commitments; write permission).
 */
export async function commitNext({ settings }) {
  const client = createClient(settings);
  const head = (await client.getHead()).headSequence;
  const commitments = await client.listCommitments();
  const lastEnd = commitments.reduce((max, c) => Math.max(max, c.sequenceEnd), 0);
  if (head <= lastEnd) return { created: false, headSequence: head, lastCommitted: lastEnd };
  const commitment = await client.createCommitment(lastEnd + 1, head);
  return { created: true, headSequence: head, commitment };
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function verificationLabel(m) {
  if (m.verified)
    return m.verification.keySource === 'roster' ? '[verified: roster key]' : '[verified]';
  const detail = m.verification.detail ? ` - ${m.verification.detail}` : '';
  return `[UNVERIFIED: ${m.verification.reason}${detail}]`;
}

function inclusionLabel(m) {
  switch (m.inclusion.status) {
    case 'verified':
      return m.inclusion.anchored ? '[included, anchored]' : '[included]';
    case 'not_committed':
      return '[not committed yet]';
    case 'skipped':
      return '';
    case 'failed':
      return `[INCLUSION FAILED: ${m.inclusion.reason}]`;
    default:
      return `[inclusion unavailable: ${m.inclusion.reason}]`;
  }
}

export function renderBoard(board) {
  const lines = [];
  const threads = new Map();
  for (const m of board.messages) {
    if (!threads.has(m.thread)) threads.set(m.thread, []);
    threads.get(m.thread).push(m);
  }
  if (!threads.size) {
    lines.push(
      board.thread
        ? `Thread "${board.thread}" has no messages (stream head #${board.headSequence}).`
        : `No board messages (stream head #${board.headSequence}).`,
    );
  }
  for (const [thread, messages] of threads) {
    const seen = new Set(messages.map((m) => m.sequenceNumber));
    lines.push(
      `Thread "${thread}" - ${messages.length} message(s), ` +
        `${messages.filter((m) => m.verified).length} verified, stream head #${board.headSequence}`,
    );
    for (const m of messages) {
      const who = m.wellFormed ? m.author : '(not a board post)';
      const reply = m.replyTo
        ? ` re #${m.replyTo}${seen.has(m.replyTo) ? '' : ' (not in thread)'}`
        : '';
      const mismatch =
        m.authorMatchesRoster === false
          ? ` [author label "${m.author}" but roster names the signer "${m.signer.rosterName}"]`
          : '';
      const labels = [verificationLabel(m), inclusionLabel(m)].filter(Boolean).join(' ');
      lines.push(
        `#${m.sequenceNumber}  ${m.createdAt}  ${who} (${String(m.signer.agentId).slice(0, 8)})${reply}  ${labels}${mismatch}`,
      );
      if (m.text !== null) {
        for (const textLine of m.text.split('\n')) lines.push(`    ${textLine}`);
      } else {
        lines.push(`    (${m.eventType})`);
      }
      if (m.refs) lines.push(`    refs: ${m.refs.join(', ')}`);
      if (m.state !== null) lines.push(`    state: ${JSON.stringify(m.state)}`);
    }
  }
  return lines.join('\n');
}
