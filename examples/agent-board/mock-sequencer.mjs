#!/usr/bin/env node
/**
 * MOCK SEQUENCER -- NOT THE REAL STATESET SEQUENCER.
 *
 * A minimal in-process HTTP stand-in for tests and local dry runs of the
 * agent board. It implements only the routes the board uses, with request and
 * response shapes taken from stateset-sequencer `src/api/` (main @ e97829b,
 * Sep 2026) so that the board exercises the same wire format it will meet in
 * production:
 *
 *   GET  /health
 *   POST /api/v1/agents/keys                      admin only; duplicate key id -> 500
 *   GET  /api/v1/agents/:agent_id/signing-keys    signed key directory -- NOT on the
 *                                                 real sequencer's main branch yet; shape
 *                                                 from stateset-sequencer PR #6
 *                                                 (disable with keyDirectory: false)
 *   POST /api/v1/ves/events/ingest                VES v1 signature + payload-hash checks,
 *                                                 monotonic per-(tenant, store) sequence
 *   GET  /api/v1/ves/events                       paged stream read
 *   GET  /api/v1/ves/head
 *   GET  /api/v1/ves/entities/:type/:id           paged entity history (100 per page)
 *   GET  /api/v1/ves/commitments                  list
 *   POST /api/v1/ves/commitments                  contiguous range commitment (write)
 *   GET  /api/v1/ves/commitments/:batch_id
 *   GET  /api/v1/ves/proofs/:sequence_number      inclusion proof (padded VES tree)
 *   GET  /api/v1/events, /api/v1/head,
 *        /api/v1/entities/:type/:id               the deprecated pre-VES routes: they read
 *                                                 the legacy event table, which VES ingest
 *                                                 never writes, so they are always empty
 *
 * Deliberately NOT modelled: persistence, JWT auth, rate limits, agent
 * policies, schema registry, encrypted payloads, VES v2 signing, hybrid/PQC
 * keys, anchoring, the real commitment state-root chain (the mock's state
 * roots are a labelled stand-in), and every route not listed above.
 *
 * Run it:   node mock-sequencer.mjs [--port 8787] [--host 127.0.0.1] [--no-key-directory]
 * It prints throwaway credentials for the run. Use it from code with
 * `startMockSequencer()`.
 */

import http from 'node:http';
import { createHash, generateKeyPairSync, randomBytes, randomUUID, sign } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import {
  DOMAIN,
  canonicalizeJson,
  computeEventSigningHash,
  computeLeafHash,
  computeNodeHash,
  computePadLeaf,
  computePayloadPlainHash,
  computeReceiptHash,
  verifyEventSignature,
} from '../../cli/src/sync/crypto.js';

export const MOCK_VERSION = '0.1.0-mock';

const NIL_UUID = '00000000-0000-0000-0000-000000000000';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const RFC3339 = /^\d{4}-\d{2}-\d{2}[Tt ]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/;
const ZERO_HASH_HEX = `0x${'00'.repeat(32)}`;
const MAX_BODY_BYTES = 4 * 1024 * 1024;
const ENTITY_PAGE = 100;

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

class HttpError extends Error {
  /**
   * @param {number} status
   * @param {string} message
   * @param {'text'|'api'|'auth'} [shape] - axum `(StatusCode, String)` plain
   *   text, `ApiError` JSON (src/api/error.rs), or the auth middleware's JSON
   * @param {string} [code]
   */
  constructor(status, message, shape = 'text', code) {
    super(message);
    this.status = status;
    this.shape = shape;
    this.code = code;
  }
}

const apiErrorCode = (status) =>
  ({ 400: 'INVALID_FIELD_VALUE', 401: 'AUTH_REQUIRED', 403: 'INSUFFICIENT_PERMISSIONS' })[status] ??
  'INTERNAL_ERROR';

function send(res, status, body, headers = {}) {
  if (body === undefined) {
    res.writeHead(status, headers);
    res.end();
    return;
  }
  const isText = typeof body === 'string';
  res.writeHead(status, {
    'content-type': isText ? 'text/plain; charset=utf-8' : 'application/json',
    ...headers,
  });
  res.end(isText ? body : JSON.stringify(body));
}

function sendError(res, error) {
  if (error.shape === 'auth') {
    send(res, error.status, { error: error.message, code: error.code });
  } else if (error.shape === 'api') {
    const code = error.code ?? apiErrorCode(error.status);
    send(res, error.status, { error: { code, message: error.message } }, { 'x-error-code': code });
  } else {
    send(res, error.status, error.message);
  }
}

function hexBytes(value, length) {
  if (typeof value !== 'string') return null;
  const hex = value.replace(/^0x/, '');
  if (hex.length !== length * 2 || !/^[0-9a-fA-F]*$/.test(hex)) return null;
  return Buffer.from(hex, 'hex');
}

const hex0x = (buffer) => `0x${buffer.toString('hex')}`;

function isU32(value) {
  return Number.isSafeInteger(value) && value >= 0 && value <= 0xffffffff;
}

function parseTimestamp(value, field) {
  if (value === undefined || value === null) return null;
  if (typeof value !== 'string' || !RFC3339.test(value) || Number.isNaN(Date.parse(value))) {
    throw new HttpError(400, `Invalid ${field} timestamp: input is not RFC 3339`);
  }
  return new Date(value).toISOString();
}

function queryScope(url, extra = []) {
  const out = {};
  for (const key of ['tenant_id', 'store_id', ...extra]) {
    const value = url.searchParams.get(key);
    if (!value || !UUID.test(value)) {
      throw new HttpError(
        400,
        `Failed to deserialize query string: missing or invalid field \`${key}\``,
      );
    }
    out[key] = value.toLowerCase();
  }
  return out;
}

function optionalUint(url, key) {
  const raw = url.searchParams.get(key);
  if (raw === null) return undefined;
  if (!/^[0-9]+$/.test(raw)) {
    throw new HttpError(400, `Failed to deserialize query string: invalid digit in \`${key}\``);
  }
  return Number(raw);
}

function nextPowerOfTwo(n) {
  let p = 1;
  while (p < n) p *= 2;
  return p;
}

/** Padded VES tree (VES v1.0 section 10): leaves, then pad leaves to a power of two. */
function paddedLayer(leaves) {
  const layer = leaves.map((leaf) => Buffer.from(leaf));
  const pad = computePadLeaf();
  while (layer.length < nextPowerOfTwo(leaves.length)) layer.push(pad);
  return layer;
}

function merkleRoot(leaves) {
  let layer = paddedLayer(leaves);
  while (layer.length > 1) {
    const next = [];
    for (let i = 0; i < layer.length; i += 2) next.push(computeNodeHash(layer[i], layer[i + 1]));
    layer = next;
  }
  return layer[0];
}

function merkleProof(leaves, leafIndex) {
  let layer = paddedLayer(leaves);
  let index = leafIndex;
  const path = [];
  const directions = [];
  while (layer.length > 1) {
    path.push(layer[index ^ 1]);
    directions.push(index % 2 === 0); // true = the current node is the left child
    const next = [];
    for (let i = 0; i < layer.length; i += 2) next.push(computeNodeHash(layer[i], layer[i + 1]));
    layer = next;
    index = Math.floor(index / 2);
  }
  return { path, directions };
}

function signingHashOf(env) {
  return computeEventSigningHash({
    vesVersion: env.ves_version,
    tenantId: env.tenant_id,
    storeId: env.store_id,
    eventId: env.event_id,
    sourceAgentId: env.source_agent_id,
    agentKeyId: env.agent_key_id,
    entityType: env.entity_type,
    entityId: env.entity_id,
    eventType: env.event_type,
    createdAt: env.created_at,
    payloadKind: env.payload_kind,
    payloadPlainHash: Buffer.from(env.payload_plain_hash.slice(2), 'hex'),
    payloadCipherHash: Buffer.from(env.payload_cipher_hash.slice(2), 'hex'),
  });
}

function leafOf(env) {
  return computeLeafHash({
    tenantId: env.tenant_id,
    storeId: env.store_id,
    sequenceNumber: env.sequence_number,
    eventSigningHash: signingHashOf(env),
    agentSignature: Buffer.from(env.agent_signature.slice(2), 'hex'),
  });
}

/** The fields an exact replay must match (everything the agent supplied). */
function replayFingerprint(env) {
  const { sequence_number: _seq, sequenced_at: _at, ...supplied } = env;
  return canonicalizeJson(supplied);
}

// ---------------------------------------------------------------------------
// The mock
// ---------------------------------------------------------------------------

/**
 * Start a mock sequencer on an ephemeral loopback port.
 *
 * @param {object} [options]
 * @param {string} [options.host='127.0.0.1']
 * @param {number} [options.port=0]
 * @param {string} [options.tenantId] - generated when omitted
 * @param {string} [options.storeId] - generated when omitted
 * @param {boolean} [options.keyDirectory=true] - serve the PR #6 key directory route
 */
export async function startMockSequencer(options = {}) {
  const tenantId = (options.tenantId ?? randomUUID()).toLowerCase();
  const storeId = (options.storeId ?? randomUUID()).toLowerCase();
  const settings = { keyDirectory: options.keyDirectory !== false, acceptUnregistered: false };

  const sequencerKeys = generateKeyPairSync('ed25519');
  const sequencerPublicKey = hex0x(
    sequencerKeys.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32),
  );
  const sequencerId = randomUUID();
  const signHash = (hash) => sign(null, hash, sequencerKeys.privateKey);

  /** @type {Map<string, {tenantId: string, storeIds: Set<string>|null, read: boolean, write: boolean, admin: boolean, agentId: string|null}>} */
  const apiKeys = new Map();
  function addApiKey({
    tenant = tenantId,
    stores = null,
    write = true,
    admin = false,
    agentId = null,
    label = 'key',
  } = {}) {
    const key = `ss_mock${label}_${randomBytes(16).toString('hex')}`;
    apiKeys.set(key, {
      tenantId: tenant,
      storeIds: stores ? new Set(stores) : null,
      read: true,
      write: write || admin,
      admin,
      agentId,
    });
    return key;
  }
  const adminApiKey = addApiKey({ admin: true, label: 'admin' });
  const writeApiKey = addApiKey({ label: 'agent' });
  const readApiKey = addApiKey({ write: false, label: 'read' });

  /** (tenant|agent|keyId) -> key entry */
  const agentKeys = new Map();
  /** (tenant|store) -> stream */
  const streams = new Map();
  /** event_id -> { streamKey, sequenceNumber } across every stream */
  const eventIndex = new Map();
  const requests = [];

  function stream(tenant, store) {
    const key = `${tenant}|${store}`;
    if (!streams.has(key)) {
      streams.set(key, {
        key,
        events: [],
        receipts: new Map(),
        commandIds: new Map(),
        commitments: [],
      });
    }
    return streams.get(key);
  }

  // --- auth (src/auth/middleware.rs, src/api/auth_helpers.rs) --------------

  function authenticate(req) {
    const header = req.headers.authorization ?? req.headers['x-api-key'];
    if (!header) throw new HttpError(401, 'Missing authentication', 'auth', 'missing_auth');
    if (header.startsWith('Bearer ')) {
      throw new HttpError(401, 'Invalid JWT', 'auth', 'invalid_jwt'); // the mock has no JWT
    }
    const key = header.startsWith('ApiKey ') ? header.slice('ApiKey '.length) : header;
    if (!key.startsWith('ss_'))
      throw new HttpError(401, 'Missing authentication', 'auth', 'missing_auth');
    const context = apiKeys.get(key);
    if (!context) throw new HttpError(401, 'Invalid API key', 'auth', 'invalid_api_key');
    return context;
  }

  function authScheme(req) {
    if (req.headers.authorization) {
      const h = req.headers.authorization;
      return h.startsWith('ApiKey ') ? 'ApiKey' : h.startsWith('Bearer ') ? 'Bearer' : 'raw';
    }
    return req.headers['x-api-key'] ? 'x-api-key' : null;
  }

  function ensureTenantStore(auth, tenant, store, shape) {
    if (auth.tenantId !== tenant) throw new HttpError(403, 'Tenant access denied', shape);
    if (store !== NIL_UUID && auth.storeIds && !auth.storeIds.has(store)) {
      throw new HttpError(403, 'Store access denied', shape);
    }
  }
  const ensureRead = (auth, tenant, store, shape = 'text') => {
    if (!auth.read) throw new HttpError(403, 'Read permission required', shape);
    ensureTenantStore(auth, tenant, store, shape);
  };
  const ensureWrite = (auth, tenant, store, shape = 'text') => {
    if (!auth.write) throw new HttpError(403, 'Write permission required', shape);
    ensureTenantStore(auth, tenant, store, shape);
  };
  const ensureAdmin = (auth, tenant, store, shape = 'text') => {
    if (!auth.admin) throw new HttpError(403, 'Admin permission required', shape);
    ensureTenantStore(auth, tenant, store, shape);
  };

  // --- agent keys (src/api/handlers/agent_keys.rs) --------------------------

  function registerAgentKey(auth, body) {
    if (
      !UUID.test(body?.tenantId ?? '') ||
      !UUID.test(body?.agentId ?? '') ||
      !isU32(body?.keyId) ||
      typeof body?.publicKey !== 'string'
    ) {
      throw new HttpError(422, 'Failed to deserialize the JSON body into the target type');
    }
    const tenant = body.tenantId.toLowerCase();
    const agent = body.agentId.toLowerCase();
    ensureAdmin(auth, tenant, NIL_UUID);
    const keyAlgorithm = body.keyAlgorithm ?? 0;
    if (keyAlgorithm !== 0 && keyAlgorithm !== 1) {
      throw new HttpError(
        400,
        `key_algorithm ${keyAlgorithm} is not supported by the mock (Ed25519 only)`,
      );
    }
    const publicKey = hexBytes(body.publicKey, 32);
    if (!publicKey) throw new HttpError(400, 'Public key must be 32 bytes');
    const validFrom = parseTimestamp(body.validFrom, 'valid_from');
    const validTo = parseTimestamp(body.validTo, 'valid_to');
    const lookup = `${tenant}|${agent}|${body.keyId}`;
    if (agentKeys.has(lookup)) {
      // KeyAlreadyExists is mapped through internal_error() upstream.
      throw new HttpError(500, 'Internal server error');
    }
    agentKeys.set(lookup, {
      keyId: body.keyId,
      keyAlgorithm,
      publicKey,
      validFrom,
      validTo,
      revokedAt: null,
    });
    return {
      success: true,
      tenantId: tenant,
      agentId: agent,
      keyId: body.keyId,
      keyAlgorithm,
      message: 'Agent key registered successfully',
    };
  }

  function keyDirectory(auth, agentId, url) {
    if (!UUID.test(agentId)) throw new HttpError(400, 'Invalid URL: Cannot parse `agent_id`');
    const tenantRaw = url.searchParams.get('tenant_id');
    if (!tenantRaw || !UUID.test(tenantRaw)) {
      throw new HttpError(400, 'Failed to deserialize query string: missing field `tenant_id`');
    }
    const tenant = tenantRaw.toLowerCase();
    ensureRead(auth, tenant, NIL_UUID);
    const agent = agentId.toLowerCase();
    const keys = [...agentKeys.entries()]
      .filter(([lookup]) => lookup.startsWith(`${tenant}|${agent}|`))
      .map(([, entry]) => ({
        keyId: entry.keyId,
        algorithm: entry.keyAlgorithm === 1 ? 'ed25519' : 'unspecified',
        publicKey: hex0x(entry.publicKey),
        publicKeyBundle: null,
        validFrom: entry.validFrom,
        validTo: entry.validTo,
        revokedAt: entry.revokedAt,
      }))
      .sort((a, b) => a.keyId - b.keyId);
    const body = { agentId: agent, tenantId: tenant, keys, signedAt: new Date().toISOString() };
    const hash = createHash('sha256')
      .update(Buffer.concat([DOMAIN.KEYDIR, Buffer.from(canonicalizeJson(body))]))
      .digest();
    return { ...body, directorySignature: hex0x(signHash(hash)) };
  }

  // --- ingest (src/api/handlers/ingest.rs, src/infra/postgres/ves_sequencer.rs) -

  /** Deserialize one wire envelope the way serde would, or 422. */
  function readEnvelope(raw) {
    const bad = () => {
      throw new HttpError(
        422,
        'Failed to deserialize the JSON body into the target type: events',
        'api',
      );
    };
    if (!raw || typeof raw !== 'object') bad();
    for (const key of ['event_id', 'tenant_id', 'store_id', 'source_agent_id']) {
      if (!UUID.test(raw[key] ?? '')) bad();
    }
    for (const key of ['entity_type', 'entity_id', 'event_type', 'created_at']) {
      if (typeof raw[key] !== 'string') bad();
    }
    if (!isU32(raw.ves_version) || !isU32(raw.agent_key_id)) bad();
    if (raw.payload_kind !== 0 && raw.payload_kind !== 1) bad();
    const plain = hexBytes(raw.payload_plain_hash, 32);
    const cipher = hexBytes(raw.payload_cipher_hash, 32);
    const signature = hexBytes(raw.agent_signature, 64);
    if (!plain || !cipher || !signature) bad();
    if (raw.command_id !== undefined && raw.command_id !== null && !UUID.test(raw.command_id))
      bad();
    const env = {
      ves_version: raw.ves_version,
      event_id: raw.event_id.toLowerCase(),
      tenant_id: raw.tenant_id.toLowerCase(),
      store_id: raw.store_id.toLowerCase(),
      source_agent_id: raw.source_agent_id.toLowerCase(),
      agent_key_id: raw.agent_key_id,
      entity_type: raw.entity_type,
      entity_id: raw.entity_id,
      event_type: raw.event_type,
      created_at: raw.created_at,
      payload_kind: raw.payload_kind,
    };
    if (raw.payload !== undefined && raw.payload !== null) env.payload = raw.payload;
    if (raw.payload_encrypted !== undefined && raw.payload_encrypted !== null) {
      env.payload_encrypted = raw.payload_encrypted;
    }
    env.payload_plain_hash = hex0x(plain);
    env.payload_cipher_hash = hex0x(cipher);
    env.agent_signature = hex0x(signature);
    if (raw.agent_signature_scheme !== undefined && raw.agent_signature_scheme !== null) {
      env.agent_signature_scheme = raw.agent_signature_scheme;
    }
    if (raw.agent_signature_bundle) env.agent_signature_bundle = raw.agent_signature_bundle;
    if (raw.command_id) env.command_id = raw.command_id.toLowerCase();
    if (raw.base_version !== undefined && raw.base_version !== null) {
      env.base_version = raw.base_version;
    }
    return env;
  }

  /** validate_event(): null when acceptable, else a rejection. */
  function validateEvent(env) {
    const reject = (reason, message) => ({ event_id: env.event_id, reason, message });
    if (env.ves_version !== 1) {
      return reject(
        'unsupported_version',
        `The mock implements VES v1 signing only, got ${env.ves_version}`,
      );
    }
    if (!env.entity_type || env.entity_type.length > 128) {
      return reject('schema_validation: entity_type', 'entity_type must be 1-128 characters');
    }
    if (!env.entity_id || env.entity_id.length > 512) {
      return reject('schema_validation: entity_id', 'entity_id must be 1-512 characters');
    }
    if (!env.event_type || env.event_type.length > 256) {
      return reject('schema_validation: event_type', 'event_type must be 1-256 characters');
    }
    if (env.payload_kind === 1) {
      return reject(
        'schema_validation: payload_encrypted',
        'The mock does not support encrypted payloads',
      );
    }
    if (env.payload === undefined) {
      return reject('schema_validation: payload', 'payload is required for plaintext events');
    }
    if (env.payload_encrypted !== undefined) {
      return reject(
        'schema_validation: payload_encrypted',
        'payload_encrypted must be omitted for plaintext events',
      );
    }
    if (env.payload_cipher_hash !== ZERO_HASH_HEX) {
      return reject(
        'schema_validation: payload_cipher_hash',
        'payload_cipher_hash must be zero for plaintext events',
      );
    }
    if (!RFC3339.test(env.created_at) || Number.isNaN(Date.parse(env.created_at))) {
      return reject('schema_validation: created_at', 'created_at must be RFC3339');
    }
    let payloadHash;
    try {
      payloadHash = hex0x(computePayloadPlainHash(env.payload));
    } catch {
      payloadHash = null;
    }
    if (payloadHash !== env.payload_plain_hash) {
      return reject('invalid_payload_hash', 'Payload hash verification failed');
    }
    const entry = agentKeys.get(`${env.tenant_id}|${env.source_agent_id}|${env.agent_key_id}`);
    const now = Date.now();
    let keyProblem = null;
    if (!entry) keyProblem = 'Agent key not found';
    else if (entry.revokedAt) keyProblem = 'Agent key has been revoked';
    else if (entry.validFrom && Date.parse(entry.validFrom) > now)
      keyProblem = 'Agent key is not yet valid';
    else if (entry.validTo && Date.parse(entry.validTo) < now) keyProblem = 'Agent key has expired';
    if (keyProblem && !(settings.acceptUnregistered && !entry)) {
      return reject(`agent_key_invalid: ${keyProblem}`, keyProblem);
    }
    const scheme = env.agent_signature_scheme ?? 0;
    if (scheme !== 0 && scheme !== 1) {
      return reject(
        'invalid_signature',
        `Agent signature scheme ${scheme} is not supported by the mock`,
      );
    }
    if (entry) {
      const ok = verifyEventSignature(
        signingHashOf(env),
        Buffer.from(env.agent_signature.slice(2), 'hex'),
        entry.publicKey,
      );
      if (!ok) return reject('invalid_signature', 'Agent signature verification failed');
    }
    return null;
  }

  function receiptFor(env) {
    const receiptHash = computeReceiptHash({
      tenantId: env.tenant_id,
      storeId: env.store_id,
      eventId: env.event_id,
      sequenceNumber: env.sequence_number,
      eventSigningHash: signingHashOf(env),
    });
    return {
      sequencerId,
      eventId: env.event_id,
      sequenceNumber: env.sequence_number,
      sequencedAt: env.sequenced_at,
      receiptHash: receiptHash.toString('hex'),
      signatureAlg: 'ed25519',
      sequencerSignature: signHash(receiptHash).toString('hex'),
      sequencerKeyVersion: 1,
    };
  }

  function ingest(auth, body) {
    if (!UUID.test(body?.agentId ?? '') || !Array.isArray(body?.events)) {
      throw new HttpError(422, 'Failed to deserialize the JSON body into the target type', 'api');
    }
    const agentId = body.agentId.toLowerCase();
    const events = body.events.map(readEnvelope);
    if (!events.length) throw new HttpError(400, 'events must not be empty', 'api');
    const { tenant_id: tenant, store_id: store } = events[0];
    if (events.some((e) => e.tenant_id !== tenant || e.store_id !== store)) {
      throw new HttpError(
        400,
        'All events in a batch must share the same tenantId and storeId',
        'api',
      );
    }
    if (auth.agentId && auth.agentId !== agentId) throw new HttpError(403, 'Agent mismatch', 'api');
    if (events.some((e) => e.source_agent_id !== agentId)) {
      throw new HttpError(400, 'event source_agent_id must match request agent_id', 'api');
    }
    ensureWrite(auth, tenant, store, 'api');

    const s = stream(tenant, store);
    const rejections = [];
    const receipts = [];
    const seenEvents = new Set();
    const seenCommands = new Set();
    let accepted = 0;
    let sequenceStart = null;
    let sequenceEnd = null;
    for (const env of events) {
      if (seenEvents.has(env.event_id)) {
        rejections.push({
          event_id: env.event_id,
          reason: 'duplicate_event_id',
          message: `Event ${env.event_id} duplicated in batch`,
        });
        continue;
      }
      seenEvents.add(env.event_id);
      const existing = eventIndex.get(env.event_id);
      if (existing) {
        const stored = streams.get(existing.streamKey).events[existing.sequenceNumber - 1];
        if (existing.streamKey !== s.key || replayFingerprint(stored) !== replayFingerprint(env)) {
          rejections.push({
            event_id: env.event_id,
            reason: 'duplicate_event_id',
            message: 'Event ID already exists with different contents',
          });
        } else {
          // Exact replay: the stored receipt, counted as neither accepted nor rejected.
          receipts.push(s.receipts.get(env.event_id));
        }
        continue;
      }
      const rejection = validateEvent(env);
      if (rejection) {
        rejections.push(rejection);
        continue;
      }
      if (env.command_id) {
        if (seenCommands.has(env.command_id)) {
          rejections.push({
            event_id: env.event_id,
            reason: 'duplicate_command_id',
            message: `Command ${env.command_id} duplicated in batch`,
          });
          continue;
        }
        seenCommands.add(env.command_id);
        if (s.commandIds.has(env.command_id)) {
          rejections.push({
            event_id: env.event_id,
            reason: 'duplicate_command_id',
            message: 'Command already processed',
          });
          continue;
        }
      }
      const sequenced = {
        ...env,
        sequence_number: s.events.length + 1,
        sequenced_at: new Date().toISOString(),
      };
      s.events.push(sequenced);
      eventIndex.set(env.event_id, { streamKey: s.key, sequenceNumber: sequenced.sequence_number });
      if (env.command_id) s.commandIds.set(env.command_id, env.event_id);
      const receipt = receiptFor(sequenced);
      s.receipts.set(env.event_id, receipt);
      receipts.push(receipt);
      accepted += 1;
      sequenceStart ??= sequenced.sequence_number;
      sequenceEnd = sequenced.sequence_number;
    }
    const response = {
      batchId: randomUUID(),
      eventsAccepted: accepted,
      eventsRejected: rejections.length,
      sequenceStart,
      sequenceEnd,
      headSequence: s.events.length,
    };
    if (rejections.length) response.rejections = rejections; // skip_serializing_if = Vec::is_empty
    if (receipts.length) response.receipts = receipts;
    return response;
  }

  // --- reads (src/api/handlers/ves/events.rs) -------------------------------

  function listEvents(auth, url) {
    const { tenant_id: tenant, store_id: store } = queryScope(url);
    const from = Math.max(1, optionalUint(url, 'from') ?? 1);
    const requested = optionalUint(url, 'limit') ?? 100;
    if (requested === 0) throw new HttpError(400, 'limit must be greater than 0');
    ensureRead(auth, tenant, store);
    const limit = Math.min(requested, 1000);
    const s = stream(tenant, store);
    const events = s.events.slice(from - 1, from - 1 + limit).map((envelope) => ({ envelope }));
    const nextSequence = events.length
      ? events[events.length - 1].envelope.sequence_number + 1
      : from;
    const head = s.events.length;
    return {
      events,
      count: events.length,
      next_sequence: nextSequence,
      head_sequence: head,
      has_more: nextSequence <= head,
    };
  }

  function head(auth, url) {
    const { tenant_id: tenant, store_id: store } = queryScope(url);
    ensureRead(auth, tenant, store);
    return { head_sequence: stream(tenant, store).events.length };
  }

  function entityHistory(auth, url, entityType, entityId) {
    if (!entityType || entityType.length > 64) {
      throw new HttpError(400, 'entity_type must be between 1 and 64 characters');
    }
    if (!entityId || entityId.length > 256) {
      throw new HttpError(400, 'entity_id must be between 1 and 256 characters');
    }
    const { tenant_id: tenant, store_id: store } = queryScope(url);
    ensureRead(auth, tenant, store);
    const offset = optionalUint(url, 'from') ?? 0;
    const requested = optionalUint(url, 'limit') ?? ENTITY_PAGE;
    const limit = requested === 0 ? ENTITY_PAGE : Math.min(requested, ENTITY_PAGE);
    const all = stream(tenant, store).events.filter(
      (e) => e.entity_type === entityType && e.entity_id === entityId,
    );
    const events = all.slice(offset, offset + limit).map((envelope) => ({ envelope }));
    return {
      entity_type: entityType,
      entity_id: entityId,
      events,
      count: events.length,
      total: all.length,
      current_version: all.length,
      has_more: offset + events.length < all.length,
    };
  }

  // --- commitments and proofs (handlers/ves/commitments.rs, inclusion_proofs.rs)

  function commitmentJson(c) {
    return {
      batch_id: c.batchId,
      tenant_id: c.tenantId,
      store_id: c.storeId,
      ves_version: 1,
      tree_depth: Math.log2(c.paddedLeafCount),
      leaf_count: c.leafCount,
      padded_leaf_count: c.paddedLeafCount,
      merkle_root: c.merkleRoot.toString('hex'),
      prev_state_root: c.prevStateRoot.toString('hex'),
      new_state_root: c.newStateRoot.toString('hex'),
      sequence_start: c.sequenceStart,
      sequence_end: c.sequenceEnd,
      committed_at: c.committedAt,
      is_anchored: false,
    };
  }

  function findCommitment(batchId) {
    for (const s of streams.values()) {
      const c = s.commitments.find((x) => x.batchId === batchId);
      if (c) return { s, c };
    }
    throw new HttpError(404, 'Commitment not found');
  }

  function createCommitment(auth, body) {
    if (
      !UUID.test(body?.tenant_id ?? '') ||
      !UUID.test(body?.store_id ?? '') ||
      !Number.isSafeInteger(body?.sequence_start) ||
      !Number.isSafeInteger(body?.sequence_end)
    ) {
      throw new HttpError(422, 'Failed to deserialize the JSON body into the target type');
    }
    const tenant = body.tenant_id.toLowerCase();
    const store = body.store_id.toLowerCase();
    ensureWrite(auth, tenant, store);
    const s = stream(tenant, store);
    const last = s.commitments.at(-1);
    const expectedStart = last ? last.sequenceEnd + 1 : 1;
    if (body.sequence_start !== expectedStart) {
      throw new HttpError(
        400,
        `Commitment range must start at ${expectedStart} (contiguous with the previous commitment)`,
      );
    }
    if (body.sequence_end < body.sequence_start) {
      throw new HttpError(400, 'sequence_end must be >= sequence_start');
    }
    if (body.sequence_end > s.events.length) {
      throw new HttpError(
        400,
        `sequence_end ${body.sequence_end} is beyond the head ${s.events.length}`,
      );
    }
    const leaves = s.events.slice(body.sequence_start - 1, body.sequence_end).map(leafOf);
    const root = merkleRoot(leaves);
    const prevStateRoot = last ? last.newStateRoot : Buffer.alloc(32);
    const commitment = {
      batchId: randomUUID(),
      tenantId: tenant,
      storeId: store,
      leafCount: leaves.length,
      paddedLeafCount: nextPowerOfTwo(leaves.length),
      merkleRoot: root,
      prevStateRoot,
      // A labelled stand-in, not the real commitment-chain state root.
      newStateRoot: createHash('sha256')
        .update('MOCK_STATE_ROOT_V1')
        .update(prevStateRoot)
        .update(root)
        .digest(),
      sequenceStart: body.sequence_start,
      sequenceEnd: body.sequence_end,
      committedAt: new Date().toISOString(),
    };
    s.commitments.push(commitment);
    return commitmentJson(commitment);
  }

  function listCommitments(auth, url) {
    const { tenant_id: tenant, store_id: store } = queryScope(url);
    ensureRead(auth, tenant, store);
    const commitments = stream(tenant, store).commitments.map(commitmentJson);
    return { commitments, count: commitments.length };
  }

  function getCommitment(auth, batchId) {
    const { c } = findCommitment(batchId);
    ensureRead(auth, c.tenantId, c.storeId);
    return {
      ...commitmentJson(c),
      chain_id: null,
      chain_tx_hash: null,
      chain_block_number: null,
      anchored_at: null,
    };
  }

  function inclusionProof(auth, url, sequenceRaw) {
    if (!/^[0-9]+$/.test(sequenceRaw))
      throw new HttpError(400, 'Invalid URL: Cannot parse `sequence_number`');
    const sequenceNumber = Number(sequenceRaw);
    const { tenant_id: tenant, store_id: store, batch_id: batchId } = queryScope(url, ['batch_id']);
    ensureRead(auth, tenant, store);
    const { s, c } = findCommitment(batchId);
    if (c.tenantId !== tenant || c.storeId !== store)
      throw new HttpError(403, 'Commitment access denied');
    if (sequenceNumber < c.sequenceStart || sequenceNumber > c.sequenceEnd) {
      throw new HttpError(400, 'Sequence number not in commitment range');
    }
    // Like the real handler, rebuild from the stored events and refuse to
    // serve a proof when they no longer reproduce the committed root.
    const leaves = s.events.slice(c.sequenceStart - 1, c.sequenceEnd).map(leafOf);
    if (!merkleRoot(leaves).equals(c.merkleRoot)) {
      throw new HttpError(500, 'Commitment merkle_root does not match ves_events');
    }
    const leafIndex = sequenceNumber - c.sequenceStart;
    const proof = merkleProof(leaves, leafIndex);
    return {
      sequence_number: sequenceNumber,
      batch_id: c.batchId,
      merkle_root: c.merkleRoot.toString('hex'),
      leaf_hash: leaves[leafIndex].toString('hex'),
      leaf_index: leafIndex,
      proof_path: proof.path.map((h) => h.toString('hex')),
      directions: proof.directions,
    };
  }

  // --- legacy pre-VES reads: the legacy table is never written by VES ingest

  const LEGACY_HEADERS = {
    deprecation: 'true',
    sunset: 'Mon, 01 Mar 2027 00:00:00 GMT',
  };

  // --- router ---------------------------------------------------------------

  const routes = [
    ['POST', /^\/api\/v1\/agents\/keys$/, (a, _m, _u, b) => registerAgentKey(a, b)],
    [
      'GET',
      /^\/api\/v1\/agents\/([^/]+)\/signing-keys$/,
      (a, m, u) => {
        if (!settings.keyDirectory) return { notFound: true };
        return keyDirectory(a, decodeURIComponent(m[1]), u);
      },
    ],
    ['POST', /^\/api\/v1\/ves\/events\/ingest$/, (a, _m, _u, b) => ingest(a, b)],
    ['GET', /^\/api\/v1\/ves\/events$/, (a, _m, u) => listEvents(a, u)],
    ['GET', /^\/api\/v1\/ves\/head$/, (a, _m, u) => head(a, u)],
    [
      'GET',
      /^\/api\/v1\/ves\/entities\/([^/]+)\/([^/]+)$/,
      (a, m, u) => entityHistory(a, u, decodeURIComponent(m[1]), decodeURIComponent(m[2])),
    ],
    ['GET', /^\/api\/v1\/ves\/commitments$/, (a, _m, u) => listCommitments(a, u)],
    ['POST', /^\/api\/v1\/ves\/commitments$/, (a, _m, _u, b) => createCommitment(a, b)],
    [
      'GET',
      /^\/api\/v1\/ves\/commitments\/([^/]+)$/,
      (a, m) => getCommitment(a, decodeURIComponent(m[1])),
    ],
    ['GET', /^\/api\/v1\/ves\/proofs\/([^/]+)$/, (a, m, u) => inclusionProof(a, u, m[1])],
    [
      'GET',
      /^\/api\/v1\/events$/,
      (a, _m, u) => {
        const { tenant_id: t, store_id: st } = queryScope(u);
        ensureRead(a, t, st);
        return { legacy: true, body: { events: [], count: 0 } };
      },
    ],
    [
      'GET',
      /^\/api\/v1\/head$/,
      (a, _m, u) => {
        const { tenant_id: t, store_id: st } = queryScope(u);
        ensureRead(a, t, st);
        return { legacy: true, body: { head_sequence: 0 } };
      },
    ],
    [
      'GET',
      /^\/api\/v1\/entities\/([^/]+)\/([^/]+)$/,
      (a, m, u) => {
        const { tenant_id: t, store_id: st } = queryScope(u);
        ensureRead(a, t, st);
        return {
          legacy: true,
          body: {
            entity_type: decodeURIComponent(m[1]),
            entity_id: decodeURIComponent(m[2]),
            events: [],
            count: 0,
            total: 0,
            has_more: false,
          },
        };
      },
    ],
  ];

  async function readBody(req) {
    const chunks = [];
    let size = 0;
    for await (const chunk of req) {
      size += chunk.length;
      if (size > MAX_BODY_BYTES) throw new HttpError(413, 'Request body too large');
      chunks.push(chunk);
    }
    if (!size) return undefined;
    try {
      return JSON.parse(Buffer.concat(chunks).toString('utf8'));
    } catch {
      throw new HttpError(400, 'Failed to parse the request body as JSON');
    }
  }

  async function handle(req, res) {
    const url = new URL(req.url, 'http://mock.invalid');
    requests.push({ method: req.method, path: url.pathname, auth: authScheme(req) });
    try {
      if (req.method === 'GET' && url.pathname === '/health') {
        send(res, 200, {
          status: 'healthy',
          service: 'stateset-sequencer-mock',
          version: MOCK_VERSION,
          mock: true,
        });
        return;
      }
      const route = routes.find(
        ([method, pattern]) => method === req.method && pattern.test(url.pathname),
      );
      if (!route) {
        send(res, 404, undefined);
        return;
      }
      const auth = authenticate(req);
      const body = req.method === 'POST' ? await readBody(req) : undefined;
      const result = route[2](auth, url.pathname.match(route[1]), url, body);
      if (result?.notFound) send(res, 404, undefined);
      else if (result?.legacy) send(res, 200, result.body, LEGACY_HEADERS);
      else send(res, 200, result);
    } catch (error) {
      if (error instanceof HttpError) sendError(res, error);
      else send(res, 500, 'Internal server error');
    }
  }

  const server = http.createServer((req, res) => {
    handle(req, res);
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(options.port ?? 0, options.host ?? '127.0.0.1', resolve);
  });
  const address = server.address();
  const host = address.family === 'IPv6' ? `[${address.address}]` : address.address;

  function storedEvent(sequenceNumber, tenant = tenantId, store = storeId) {
    const event = stream(tenant, store).events[sequenceNumber - 1];
    if (!event) throw new Error(`No event #${sequenceNumber}`);
    return event;
  }

  return {
    url: `http://${host}:${address.port}`,
    tenantId,
    storeId,
    adminApiKey,
    writeApiKey,
    readApiKey,
    sequencerPublicKey,
    /** Every request seen: method, path, and which auth header form was used. */
    requests,
    addApiKey,
    /** Toggle the PR #6 key directory route (off = a sequencer without it: 404). */
    setKeyDirectory(enabled) {
      settings.keyDirectory = Boolean(enabled);
    },
    /**
     * Simulate a sequencer that sequences events from agents with no
     * registered key (the real one refuses them with agent_key_invalid).
     */
    setAcceptUnregistered(enabled) {
      settings.acceptUnregistered = Boolean(enabled);
    },
    /** Simulate tampering at rest: mutate the stored wire envelope of event #n. */
    tamperEvent(sequenceNumber, mutate) {
      mutate(storedEvent(sequenceNumber));
    },
    /** Replace event #n's payload; with rehash, also its payload_plain_hash. */
    tamperPayload(sequenceNumber, payload, { rehash = false } = {}) {
      const event = storedEvent(sequenceNumber);
      event.payload = payload;
      if (rehash) event.payload_plain_hash = hex0x(computePayloadPlainHash(payload));
    },
    events: (tenant = tenantId, store = storeId) =>
      stream(tenant, store).events.map((e) => ({ ...e })),
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections?.();
        server.close(() => resolve());
      }),
  };
}

// ---------------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------------

async function runCli() {
  const { values } = parseArgs({
    options: {
      host: { type: 'string', default: '127.0.0.1' },
      port: { type: 'string', default: '8787' },
      'tenant-id': { type: 'string' },
      'store-id': { type: 'string' },
      'no-key-directory': { type: 'boolean' },
    },
  });
  const mock = await startMockSequencer({
    host: values.host,
    port: Number(values.port),
    tenantId: values['tenant-id'],
    storeId: values['store-id'],
    keyDirectory: !values['no-key-directory'],
  });
  process.stdout.write(
    [
      '*** MOCK SEQUENCER - NOT the real StateSet sequencer. In-memory; data is lost on exit. ***',
      `listening on ${mock.url}${values['no-key-directory'] ? ' (key directory route disabled)' : ''}`,
      '',
      '# Throwaway credentials for this run only:',
      `export SEQUENCER_URL=${mock.url}`,
      `export SEQUENCER_TENANT_ID=${mock.tenantId}`,
      `export SEQUENCER_STORE_ID=${mock.storeId}`,
      `export SEQUENCER_PUBLIC_KEY=${mock.sequencerPublicKey}`,
      `export SEQUENCER_API_KEY=${mock.writeApiKey}        # agents: read + write`,
      `# admin key (needed for "register"): ${mock.adminApiKey}`,
      `# read-only key: ${mock.readApiKey}`,
      '',
      'Ctrl-C to stop.',
      '',
    ].join('\n'),
  );
  const stop = async () => {
    await mock.close();
    process.exit(0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  runCli().catch((error) => {
    console.error(`mock-sequencer: ${error.message}`);
    process.exitCode = 1;
  });
}
