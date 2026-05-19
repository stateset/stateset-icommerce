// ICPIP-0005 §2 — EventEnvelope emission to registered webhook channels.
//
// Single-attempt HTTP delivery for the reference implementation. The
// spec's full retry semantics (exponential backoff, 8 attempts, DLQ
// on terminal 4xx) will land in a follow-up tick — this module
// establishes the envelope + signing wire format so the spec is
// concretely buildable today.
//
// Responsibilities:
//   1. Maintain monotonic `sequence` per channel.
//   2. Track per-channel `previous_event_id` chain.
//   3. Build a canonical `EventEnvelope` per ICPIP-0005 §2.
//   4. Sign the envelope (Ed25519, source's signing key).
//   5. POST to the webhook URL with `X-ICP-Signature` (Ed25519 over
//      `timestamp.method.path.body`) for HTTP-layer defense-in-depth.

import { canonicalJson, signEd25519, newId } from './codec.mjs';

// channel_id → { sequence: number, last_event_id: string|null }
const channelEmitState = new Map();

// Per-channel ring buffer of recent signed envelopes for ICPIP-0005 §5
// recovery. Each entry is `{envelope, signature}` exactly as it was
// POSTed to the receiver — verbatim canonical bytes so the receiver can
// re-verify with the same algorithm. Default retention is 1000 events
// per channel (override via `RECOVERY_LIMIT`).
const RECOVERY_LIMIT = 1000;
const channelEventLog = new Map(); // channel_id → Array<{envelope, signature}>

function recordForRecovery(channelId, envelope, signature) {
  let log = channelEventLog.get(channelId);
  if (!log) {
    log = [];
    channelEventLog.set(channelId, log);
  }
  log.push({ envelope, signature });
  // Trim from the head if we exceed retention. Production handlers
  // would persist this to durable storage; the reference impl keeps
  // it in-memory and bounded.
  while (log.length > RECOVERY_LIMIT) log.shift();
}

/**
 * Fetch signed events for a channel with `sequence > since`. Used by
 * agents that observed a gap in the live stream to backfill missed
 * deliveries. Returns events in ascending sequence order.
 *
 * Returns `null` if the channel has no recorded events (or has been
 * pruned past `since`). Callers should treat that as
 * `channel.sequence_gap` and re-register.
 */
export function fetchChannelEvents(channelId, since) {
  const log = channelEventLog.get(channelId);
  if (!log) return null;
  // Sanity: oldest retained sequence. If `since` is before that, the
  // caller has missed events we no longer have — surface as a gap.
  if (log.length === 0) return [];
  const oldest = log[0].envelope.sequence;
  if (since < oldest - 1) return null;
  return log.filter((e) => e.envelope.sequence > since);
}

// ICPIP-0005 §4.1 default retry schedule: 8 attempts, exponential
// backoff starting at 5s and doubling. Total horizon ≈ 20 minutes.
// Tests override via `opts.retryPolicy` with tighter values.
const DEFAULT_RETRY_POLICY = {
  max_attempts: 8,
  initial_delay_ms: 5_000,
  backoff: 'exponential',  // future: 'linear' | 'constant'
};

// HTTP status codes that should NOT be retried per ICPIP-0005 §4.1:
// 4xx is a terminal client error EXCEPT 408 (Request Timeout) and 429
// (Too Many Requests). 2xx are obviously not retried (success).
function isTerminal(status) {
  if (status === 408 || status === 429) return false;
  return status >= 400 && status < 500;
}

function nextDelayMs(policy, attempt) {
  if (policy.backoff === 'exponential') {
    // attempt is 1-indexed; first retry waits initial_delay_ms * 2^0.
    return policy.initial_delay_ms * Math.pow(2, attempt - 1);
  }
  if (policy.backoff === 'linear') {
    return policy.initial_delay_ms * attempt;
  }
  // 'constant'
  return policy.initial_delay_ms;
}

// Pluggable timer for tests (replaceable via opts.scheduler).
// The real scheduler calls `.unref()` on each timer so pending retries
// don't keep the Node event loop alive past handler shutdown — graceful
// shutdown should not block on background webhook retries. Receivers
// will see the dropped delivery as a sequence gap and recover via §5.
const realScheduler = {
  setTimeout: (fn, delay) => {
    const t = globalThis.setTimeout(fn, delay);
    if (typeof t?.unref === 'function') t.unref();
    return t;
  },
};

/**
 * Emit a signed EventEnvelope to a registered channel with retry semantics
 * per ICPIP-0005 §4.1.
 *
 * The function awaits the FIRST attempt and returns its outcome. If that
 * attempt fails AND retries are configured AND the failure is non-terminal,
 * subsequent attempts are scheduled in the background — the caller doesn't
 * await them. Each retry re-signs the envelope with `delivery_attempt`
 * incremented, since the spec mandates `delivery_attempt` is part of the
 * canonical bytes and therefore changes the signature on every attempt.
 *
 * Recovery is unaffected: the FIRST attempt's signed envelope is what
 * appears in the recovery log, since that's the canonical "delivery_attempt=1"
 * form receivers will dedupe against.
 *
 * @param {object} channel
 * @param {string} eventType
 * @param {object} payload
 * @param {object} opts
 * @param {*} opts.signingKey
 * @param {string} opts.sourceAid
 * @param {function} [opts.fetchImpl]
 * @param {{max_attempts:number, initial_delay_ms:number, backoff?:string}} [opts.retryPolicy]
 * @param {{setTimeout: function}} [opts.scheduler]  — tests inject a fake.
 * @param {function} [opts.onAttempt]  — invoked per attempt (test observability).
 * @returns {Promise<{ok:boolean,status?:number,event_id:string,sequence:number,attempts?:number,error?:string}>}
 */
export async function emitEvent(channel, eventType, payload, opts) {
  if (channel.channel_type !== 'webhook') {
    return { ok: false, error: 'sse_delivery_not_implemented', event_id: null, sequence: null };
  }

  // 1. Maintain per-channel state (sequence + last_event_id chain).
  let st = channelEmitState.get(channel.channel_id);
  if (!st) {
    st = { sequence: 0, last_event_id: null };
    channelEmitState.set(channel.channel_id, st);
  }
  st.sequence += 1;

  // 2. Build the EventEnvelope per ICPIP-0005 §2.
  const eventId = newId('icp_evt');
  const originatedAt = new Date().toISOString();
  const baseEnvelope = {
    v: 'icp-1.0',
    event_id: eventId,
    event_type: eventType,
    channel_id: channel.channel_id,
    sequence: st.sequence,
    originated_at: originatedAt,
    source: opts.sourceAid,
    target: channel.agent,
    payload,
    previous_event_id: st.last_event_id,
  };

  // Record the FIRST-attempt envelope (delivery_attempt=1) for recovery,
  // before any network attempt. This is what receivers will see on the
  // canonical recovery path.
  const firstEnvelope = { ...baseEnvelope, delivery_attempt: 1 };
  const firstCanonical = canonicalJson(firstEnvelope);
  const firstSig = signEd25519(firstCanonical, opts.signingKey);
  const firstSignature = { alg: 'ed25519', kid: opts.sourceAid, sig: firstSig };
  recordForRecovery(channel.channel_id, firstEnvelope, firstSignature);

  const policy = { ...DEFAULT_RETRY_POLICY, ...(opts.retryPolicy ?? {}) };
  const scheduler = opts.scheduler ?? realScheduler;
  const fetchImpl = opts.fetchImpl ?? fetch;

  // attemptDelivery: build the per-attempt signed body + headers, fire
  // the POST, and return `{status, error}`. Re-signs each attempt because
  // delivery_attempt is in the canonical bytes.
  async function attemptDelivery(attempt) {
    const envelope = { ...baseEnvelope, delivery_attempt: attempt };
    const envelopeCanonical = canonicalJson(envelope);
    const envelopeSig = signEd25519(envelopeCanonical, opts.signingKey);
    const signature = { alg: 'ed25519', kid: opts.sourceAid, sig: envelopeSig };
    const body = JSON.stringify({ envelope, signature });
    const url = new URL(channel.webhook_url);
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const httpMaterial = `${timestamp}.POST.${url.pathname}${url.search}.${body}`;
    const httpSig = signEd25519(httpMaterial, opts.signingKey);
    try {
      const resp = await fetchImpl(channel.webhook_url, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'x-icp-timestamp': timestamp,
          'x-icp-signature': `ed25519=${httpSig}`,
          'x-icp-channel-id': channel.channel_id,
          'x-icp-event-id': eventId,
          'x-icp-sequence': String(st.sequence),
          'x-icp-delivery-attempt': String(attempt),
        },
        body,
      });
      return { status: resp.status };
    } catch (err) {
      return { error: `transport: ${err.message}` };
    }
  }

  // Schedule retries in the background — caller does NOT await these.
  function scheduleRetries(startAttempt) {
    if (startAttempt > policy.max_attempts) return;
    const delay = nextDelayMs(policy, startAttempt - 1);
    scheduler.setTimeout(async () => {
      const r = await attemptDelivery(startAttempt);
      if (opts.onAttempt) opts.onAttempt({ attempt: startAttempt, ...r });
      const delivered = r.status != null && r.status >= 200 && r.status < 300;
      if (delivered) {
        st.last_event_id = eventId;
        return;
      }
      if (r.status != null && isTerminal(r.status)) return;
      scheduleRetries(startAttempt + 1);
    }, delay);
  }

  // First attempt is awaited synchronously by the caller — gives them
  // immediate feedback on whether the live delivery worked.
  const first = await attemptDelivery(1);
  if (opts.onAttempt) opts.onAttempt({ attempt: 1, ...first });
  const delivered = first.status != null && first.status >= 200 && first.status < 300;
  if (delivered) {
    st.last_event_id = eventId;
    return { ok: true, status: first.status, event_id: eventId, sequence: st.sequence, attempts: 1 };
  }

  // First attempt failed. Schedule retries unless terminal.
  if (first.status != null && isTerminal(first.status)) {
    return {
      ok: false,
      status: first.status,
      event_id: eventId,
      sequence: st.sequence,
      attempts: 1,
      error: `terminal_status:${first.status}`,
    };
  }

  if (policy.max_attempts > 1) {
    scheduleRetries(2);
  }

  return {
    ok: false,
    status: first.status,
    event_id: eventId,
    sequence: st.sequence,
    attempts: 1,
    error: first.error ?? `non_2xx:${first.status}`,
  };
}

/**
 * Verify the HTTP-layer signature on a received webhook. Receivers call
 * this with the merchant's published Ed25519 pubkey before processing
 * the envelope. Returns true if valid; false otherwise.
 *
 * Provided here for symmetry with the emit path — production receivers
 * will reimplement this in their own language, but the algorithm is
 * normative.
 */
export function buildHttpSigningMaterial({ timestamp, method, path, body }) {
  return `${timestamp}.${method}.${path}.${body}`;
}

/**
 * Fan out a single event to every channel in `channelStore` that
 * (a) is a webhook (SSE is a future tick), (b) has not expired,
 * and (c) has `eventType` in its `events_registered` filter set.
 *
 * Delivers in parallel; logs but does not throw on per-channel
 * failures. Returns an array of `{channel_id, ok, status, error?}`
 * for caller introspection — typically only used by tests.
 */
export async function publishToSubscribers(channelStore, eventType, payload, opts) {
  const matches = [];
  for (const ch of channelStore.values()) {
    if (ch.channel_type !== 'webhook') continue;
    if (!Array.isArray(ch.events_registered) || !ch.events_registered.includes(eventType)) continue;
    if (Date.parse(ch.expires_at) < Date.now()) continue;
    matches.push(ch);
  }
  if (matches.length === 0) return [];
  const results = await Promise.all(
    matches.map(async (ch) => {
      try {
        const r = await emitEvent(ch, eventType, payload, opts);
        return { channel_id: ch.channel_id, ...r };
      } catch (e) {
        return { channel_id: ch.channel_id, ok: false, error: e?.message ?? String(e) };
      }
    }),
  );
  return results;
}

/** Reset per-channel sequence state. Tests only — never call in production. */
export function _resetEmitState() {
  channelEmitState.clear();
  channelEventLog.clear();
}

/** Inspect per-channel sequence state. Tests only. */
export function _getEmitState(channelId) {
  return channelEmitState.get(channelId);
}
