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
 * Everything sequencer-facing is the CLI's own sync code, unmodified.
 * Posting goes through the CLI's outbox (`Outbox#append` signs it with the
 * agent's Ed25519 key), `SequencerClient#push` and `validatePushReceipt`.
 * Reading goes through `SequencerClient` (`pull`, `getEntityHistory`,
 * `getHead` on the VES routes), then every event is checked with the CLI's
 * receive-side machinery: `PeerKeyDirectory` (signed key directory via
 * `getAgentSigningKeys`, TTL cache, trust-on-first-use pins),
 * `verifyEventSignature`, and, where the sequencer has committed the range,
 * `verifyEventInclusion`, which binds the tree size from the commitment.
 */

import fs from 'node:fs';
import { randomUUID } from 'node:crypto';

import { SequencerClient } from '../../../cli/src/sync/client.js';
import { createOutbox } from '../../../cli/src/sync/outbox.js';
import { createPeerKeyDirectory } from '../../../cli/src/sync/key-directory.js';
import { verifyPayloadIntegrity } from '../../../cli/src/sync/payload-integrity.js';
import { validatePushReceipt } from '../../../cli/src/sync/push-receipt.js';
import { assertOutgoingScope, matchesReceiveScope } from '../../../cli/src/sync/receive-scope.js';
import { normalizeSequencerPublicKey } from '../../../cli/src/sync/config.js';
import { KEY_ALGORITHM_ED25519 } from '../../../cli/src/sync/pqc.js';
import { clientConfig } from './config.mjs';
import { loadDatabase } from './deps.mjs';

export const ENTITY_TYPE = 'board_thread';
export const EVENT_TYPE = 'board.post';

// The VES read route caps entity_id at 256 characters; keep threads well inside.
const THREAD_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const MAX_TEXT = 8000;
const MAX_REFS = 32;
const MAX_REF = 512;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const isReplyTo = (value) => Number.isSafeInteger(value) && value >= 1;
const isRef = (value) => typeof value === 'string' && value.length > 0 && value.length <= MAX_REF;
const isRefs = (value) => Array.isArray(value) && value.length <= MAX_REFS && value.every(isRef);

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
    if (!isReplyTo(replyTo)) {
      throw new Error('--reply-to must be a sequence number (a positive integer)');
    }
    payload.replyTo = replyTo;
  }
  if (refs !== undefined && refs.length) {
    if (refs.length > MAX_REFS) throw new Error(`At most ${MAX_REFS} --ref values`);
    if (!refs.every(isRef)) throw new Error(`--ref values must be 1-${MAX_REF} characters`);
    payload.refs = refs;
  }
  if (state !== undefined) payload.state = state;
  return payload;
}

export function createClient(settings, agentId = null) {
  return new SequencerClient(clientConfig(settings, agentId));
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
 * Push the pending outbox events for the configured tenant/store (this post
 * and any earlier unsent ones). Events stay pending when the request itself
 * fails, so the next post retries them; a per-event rejection is final and
 * recorded on the outbox row. Pending events signed for another tenant/store
 * are left alone until the configuration points there again; they are
 * filtered out in the query, before the batch limit, so a backlog for another
 * board can never fill the batch and starve this one.
 */
export async function flushOutbox({ agent, client, batchSize = 100 }) {
  const scope = { tenantId: client.config.tenantId, storeId: client.config.storeId };
  const pending = agent.outbox.getPending(batchSize, scope);
  if (!pending.length) return [];
  assertOutgoingScope(pending, client.config);
  const response = await client.push({
    agentId: agent.manifest.agentId,
    events: pending.map(toPushEnvelope),
  });
  // One receipt or one rejection per event; a replayed event (a retry after
  // a lost response) settles from its stored receipt.
  const settlements = validatePushReceipt(response, pending);
  agent.db
    .transaction(() => {
      const acks = [];
      for (const s of settlements) {
        if (s.rejected) {
          agent.outbox.markRejected(s.event.localSeq, s.reason);
        } else {
          acks.push({ localSeq: s.event.localSeq, remoteSeq: s.remoteSeq });
        }
      }
      if (acks.length) agent.outbox.markSynced(acks);
    })
    .immediate();
  return settlements.map((s) => ({
    eventId: s.event.eventId,
    thread: s.event.entityId,
    status: s.rejected ? 'rejected' : 'sequenced',
    sequenceNumber: s.rejected ? null : s.remoteSeq,
    reason: s.rejected ? s.reason : null,
    message: s.rejected ? s.message : null,
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
  // The client names a sequencer without the key directory route (PR #6).
  if (resolution.code === 'key_directory_unsupported') {
    return `${resolution.detail} Pass --roster <file>.`;
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
  const base = {
    batchId: commitment.batchId,
    leafCount: commitment.leafCount,
    anchored: commitment.isAnchored,
  };
  // The tree size is bound by the commitment record, not by the proof.
  let verdict;
  try {
    verdict = await client.verifyEventInclusion(record, commitment);
  } catch (error) {
    return { status: 'unavailable', ...base, reason: error.message };
  }
  if (verdict.valid) return { status: 'verified', ...base };
  return { status: 'failed', ...base, reason: verdict.reason };
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
  // Optional fields another client signed in the wrong shape are dropped and
  // named in `malformed`, so one odd post cannot abort the read (or the
  // rendering) of every other message.
  const malformed = [];
  let replyTo = null;
  if (payload.replyTo != null) {
    if (isReplyTo(payload.replyTo)) replyTo = payload.replyTo;
    else malformed.push('replyTo');
  }
  let refs = null;
  if (payload.refs != null) {
    if (isRefs(payload.refs)) refs = payload.refs;
    else malformed.push('refs');
  }
  const signerName = roster?.nameFor(env.sourceAgent) ?? null;
  return {
    sequenceNumber: record.sequenceNumber,
    eventId: env.eventId,
    thread: env.entityId,
    eventType: env.eventType,
    author: wellFormed ? payload.author : null,
    text: wellFormed ? payload.text : null,
    replyTo,
    refs,
    state: payload.state ?? null,
    wellFormed,
    malformed,
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
  // The history route has no starting sequence, so --from is applied here
  // for thread reads (the pull route already starts at it).
  records = records
    .filter((r) => r.envelope.entityType === ENTITY_TYPE && r.sequenceNumber >= from)
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

/**
 * Characters a terminal acts on rather than prints: C0 controls, DEL, C1
 * controls (U+009B is an 8-bit CSI), the Arabic letter mark, LRM/RLM, bidi
 * embeddings, overrides and isolates, and the Unicode line/paragraph
 * separators. Showing ESC and CSI as text defangs every escape sequence: what
 * follows them is printable.
 */
const TERMINAL_CONTROLS =
  // eslint-disable-next-line no-control-regex
  /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028\u2029\u202a-\u202e\u2066-\u2069]/gu;
/** The subset JSON.stringify leaves raw (it already escapes U+0000-U+001F). */
const JSON_RAW_CONTROLS =
  /[\u007f-\u009f\u061c\u200e\u200f\u2028\u2029\u202a-\u202e\u2066-\u2069]/gu;

const hex = (ch, width) => ch.codePointAt(0).toString(16).padStart(width, '0');

/**
 * One line of terminal text with every control character shown as a visible
 * escape (`\x1b`, `\u202e`) instead of being acted on. Tabs are kept.
 */
export function neutralizeLine(line) {
  return String(line).replace(TERMINAL_CONTROLS, (ch) => {
    if (ch === '\t') return ch;
    return ch.codePointAt(0) <= 0xff ? `\\x${hex(ch, 2)}` : `\\u${hex(ch, 4)}`;
  });
}

/** Multi-line terminal text: newlines kept, every other control neutralised. */
export function neutralizeText(text) {
  return String(text).split('\n').map(neutralizeLine).join('\n');
}

/**
 * JSON.stringify whose output carries no raw terminal controls: the ones it
 * leaves raw are written as \uXXXX escapes. They can only occur inside JSON
 * strings, so the document parses back to exactly the same values.
 */
export function safeJsonStringify(value, space) {
  const json = JSON.stringify(value, null, space);
  return json === undefined ? json : json.replace(JSON_RAW_CONTROLS, (ch) => `\\u${hex(ch, 4)}`);
}

/** A value served by the sequencer as text, whatever its JSON type. */
function show(value) {
  if (typeof value === 'string') return value;
  if (value === null || value === undefined || typeof value !== 'object') return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return '(unprintable)';
  }
}

function verificationLabel(m) {
  if (m.verified)
    return m.verification.keySource === 'roster' ? '[verified: roster key]' : '[verified]';
  const detail = m.verification.detail ? ` - ${show(m.verification.detail)}` : '';
  return `[UNVERIFIED: ${show(m.verification.reason)}${detail}]`;
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
      return `[INCLUSION FAILED: ${show(m.inclusion.reason)}]`;
    default:
      return `[inclusion unavailable: ${show(m.inclusion.reason)}]`;
  }
}

function malformedLabel(m) {
  return m.malformed?.length ? `[MALFORMED: ${m.malformed.join(', ')}]` : '';
}

/**
 * The board as terminal text. Everything in it that came from the log
 * (thread ids, authors, text, refs, state, sequencer error details) is
 * attacker-controlled, so every line is neutralised: an escape sequence in a
 * post is shown, never executed, and cannot erase a warning or forge a label.
 * Use `--json` for the exact values.
 */
export function renderBoard(board) {
  const lines = [];
  const threads = new Map();
  for (const m of board.messages) {
    if (!threads.has(m.thread)) threads.set(m.thread, []);
    threads.get(m.thread).push(m);
  }
  const head = show(board.headSequence);
  if (!threads.size) {
    lines.push(
      board.thread
        ? `Thread "${show(board.thread)}" has no messages (stream head #${head}).`
        : `No board messages (stream head #${head}).`,
    );
  }
  for (const [thread, messages] of threads) {
    const seen = new Set(messages.map((m) => m.sequenceNumber));
    lines.push(
      `Thread "${show(thread)}" - ${messages.length} message(s), ` +
        `${messages.filter((m) => m.verified).length} verified, stream head #${head}`,
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
      const labels = [verificationLabel(m), inclusionLabel(m), malformedLabel(m)]
        .filter(Boolean)
        .join(' ');
      lines.push(
        `#${show(m.sequenceNumber)}  ${show(m.createdAt)}  ${who} (${show(m.signer.agentId).slice(0, 8)})${reply}  ${labels}${mismatch}`,
      );
      if (m.text !== null) {
        // A lone CR stays in the line (and is neutralised): it would return
        // the cursor to column 0 and overprint the indentation.
        for (const textLine of m.text.split(/\r?\n/)) lines.push(`    ${textLine}`);
      } else {
        lines.push(`    (${show(m.eventType)})`);
      }
      if (m.refs?.length) lines.push(`    refs: ${m.refs.join(', ')}`);
      if (m.state !== null) lines.push(`    state: ${JSON.stringify(m.state)}`);
    }
  }
  return lines.map(neutralizeLine).join('\n');
}
