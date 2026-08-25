#!/usr/bin/env node
// IUT adapter — reference implementation
//
// Wraps the same identity/signing logic as `icp-spec/examples/01-aid-and-sign/demo.mjs`,
// but in deterministic mode: keypairs are derived from seed bytes provided as input
// rather than freshly generated.
//
// Protocol: see ../iut.protocol.md
//
// Usage:
//   echo '{"test":"01-aid-derivation",...}' | node reference-demo.mjs 01-aid-derivation
//
// Behavior:
//   - Reads one JSON object from stdin.
//   - Performs the requested test.
//   - Writes one JSON object to stdout. Any other stdout output is a protocol violation.
//   - Logs (if any) go to stderr.

import { readFileSync } from 'node:fs';
import {
  createPrivateKey,
  createPublicKey,
  sign,
  verify,
  createHash,
} from 'node:crypto';

// ===========================================================================
// Helpers
// ===========================================================================

// PKCS#8 DER prefix for Ed25519 from raw 32-byte seed.
const ED25519_PKCS8_PREFIX = Buffer.from(
  '302e020100300506032b657004220420',
  'hex',
);
// PKCS#8 DER prefix for X25519 from raw 32-byte seed.
const X25519_PKCS8_PREFIX = Buffer.from(
  '302e020100300506032b656e04220420',
  'hex',
);

function privateKeyFromSeed(alg, seed) {
  const prefix = alg === 'ed25519' ? ED25519_PKCS8_PREFIX : X25519_PKCS8_PREFIX;
  const der = Buffer.concat([prefix, seed]);
  return createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
}

function extractRawPublicKey(keyObject) {
  // Both Ed25519 and X25519 SPKI exports are 44 bytes: 12-byte header + 32-byte raw key.
  const spki = keyObject.export({ format: 'der', type: 'spki' });
  if (spki.length !== 44) throw new Error(`unexpected SPKI length ${spki.length}`);
  return spki.subarray(12, 44);
}

function base58btcEncode(buf) {
  const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let n = 0n;
  for (const b of buf) n = (n << 8n) | BigInt(b);
  let out = '';
  while (n > 0n) {
    const r = Number(n % 58n);
    n = n / 58n;
    out = ALPHABET[r] + out;
  }
  for (const b of buf) {
    if (b === 0) out = '1' + out;
    else break;
  }
  return out;
}

function canonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  const keys = Object.keys(value).sort();
  return '{' + keys.map((k) => JSON.stringify(k) + ':' + canonicalJson(value[k])).join(',') + '}';
}

// ===========================================================================
// Test handlers
// ===========================================================================

function run01AidDerivation(input) {
  const edSeed = Buffer.from(input.agent.ed25519_seed_hex, 'hex');
  const xSeed = Buffer.from(input.agent.x25519_seed_hex, 'hex');

  if (edSeed.length !== 32) throw new Error('ed25519_seed must be 32 bytes');
  if (xSeed.length !== 32) throw new Error('x25519_seed must be 32 bytes');

  const edPriv = privateKeyFromSeed('ed25519', edSeed);
  const xPriv = privateKeyFromSeed('x25519', xSeed);
  const edPub = createPublicKey(edPriv);
  const xPub = createPublicKey(xPriv);

  const edPubRaw = extractRawPublicKey(edPub);
  const xPubRaw = extractRawPublicKey(xPub);

  // AID per ICP-1.0 §4.2.
  const aidPayload = Buffer.concat([edPubRaw, Buffer.from([0x00]), xPubRaw]);
  const aidDigest = createHash('sha256').update(aidPayload).digest();
  const aid = `aid:v1:z${base58btcEncode(aidDigest)}`;

  // Build the Intent from the deterministic fields supplied in the input.
  // The adapter MUST plug the computed AID into `buyer` and into the
  // PrincipalBinding's `agent` field (since this Agent is self-binding for
  // the test).
  const intent = {
    ...input.intent,
    buyer: aid,
    principal_binding: { ...input.intent.principal_binding, agent: aid },
  };

  // Canonicalize and sign.
  const canonical = canonicalJson(intent);
  const sigBytes = sign(null, Buffer.from(canonical), edPriv);

  const out = {
    ed25519_pubkey_hex: edPubRaw.toString('hex'),
    x25519_pubkey_hex: xPubRaw.toString('hex'),
    aid,
    intent_canonical_string: canonical,
    intent_canonical_bytes_hex: Buffer.from(canonical).toString('hex'),
    intent_signature_hex: sigBytes.toString('hex'),
  };

  if (input.params?.verify_tamper_rejected) {
    const tampered = canonical.replace('29.99', '0.01');
    const ok = verify(null, Buffer.from(tampered), edPub, sigBytes);
    out.tamper_rejected = !ok;
  }

  return out;
}

function run02CanonicalJson(input) {
  if (!Array.isArray(input.cases)) throw new Error('input.cases must be an array');
  const canonical_strings = input.cases.map((c) => canonicalJson(c.value));
  return { canonical_strings, names: input.cases.map((c) => c.name) };
}

function run03SignatureVerification(input) {
  if (!Array.isArray(input.cases)) throw new Error('input.cases must be an array');
  const verifications = input.cases.map((c) => {
    try {
      const sigBytes = Buffer.from(c.signature_hex, 'hex');
      if (sigBytes.length !== 64) return false;
      const pubRaw = Buffer.from(c.pubkey_hex, 'hex');
      if (pubRaw.length !== 32) return false;
      // Reconstruct the SPKI envelope to make node:crypto accept the raw pubkey.
      const spkiPrefix = Buffer.from('302a300506032b6570032100', 'hex');
      const pubKey = createPublicKey({
        key: Buffer.concat([spkiPrefix, pubRaw]),
        format: 'der',
        type: 'spki',
      });
      return verify(null, Buffer.from(c.canonical), pubKey, sigBytes);
    } catch (_) {
      return false;
    }
  });
  return { verifications, names: input.cases.map((c) => c.name) };
}

// ===========================================================================
// 04-escrow-lifecycle — ICP-1.0 §8 state machine + event replay
// ===========================================================================

// The normative §8 transition table, encoded directly.
const ESCROW_TRANSITIONS = {
  'pending|payment_confirmed': 'funded',
  'funded|fulfillment_confirmed_window_elapsed': 'released',
  'funded|dispute_raised': 'disputed',
  'disputed|resolution_favors_merchant': 'released',
  'disputed|resolution_favors_buyer': 'refunded',
  'funded|merchant_cancel_or_expiry': 'refunded',
};

function escrowStep(state, trigger) {
  const next = ESCROW_TRANSITIONS[`${state}|${trigger}`];
  if (next) return { state: next };
  if (state === 'funded' && trigger === 'payment_confirmed') {
    return { error: 'escrow.already_funded' };
  }
  return { error: 'escrow.wrong_state' };
}

function escrowReplay(events) {
  let state = 'pending';
  for (let i = 0; i < events.length; i++) {
    if (events[i].seq !== i) return { error: 'escrow.seq_out_of_order' };
    const step = escrowStep(state, events[i].trigger);
    if (step.error) return { error: step.error };
    state = step.state;
  }
  return { final_state: state };
}

function run04EscrowLifecycle(input) {
  const transitions = {};
  for (const c of input.transition_cases) {
    transitions[c.id] = escrowStep(c.from, c.trigger);
  }
  const replays = {};
  for (const c of input.replay_cases) {
    replays[c.id] = escrowReplay(c.events);
  }
  return { transitions, replays };
}

// ===========================================================================
// 05-intent-validation — ICP-1.0 §6 intent envelope validation
// ===========================================================================

const AID_RE = /^aid:v1:z[1-9A-HJ-NP-Za-km-z]{40,60}$/;
const SETTLER_RE = /^settler:[a-z0-9]+(\.[a-z0-9]+)*$/;
const MONEY_RE = /^-?[0-9]+(\.[0-9]{1,18})?$/;

// Per-verb: AID-typed fields, top-level Money fields, whether `items` is
// required, and the full required-field list from the §6 schemas.
const INTENT_VERBS = {
  'purchase.create': { aids: ['buyer', 'merchant'], money: ['max_total'], itemsRequired: true,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'items', 'max_total', 'expiry', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'inventory.query': { aids: ['buyer', 'merchant'], money: [], itemsRequired: false,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'quote.request': { aids: ['buyer', 'merchant'], money: [], itemsRequired: true,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'items', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'payout.request': { aids: ['seller', 'platform'], money: ['amount'], itemsRequired: false,
    required: ['v', 'verb', 'intent_id', 'seller', 'platform', 'settler', 'amount', 'destination', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'subscription.create': { aids: ['buyer', 'merchant'], money: ['max_total_per_period'], itemsRequired: false,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'service_id', 'cadence', 'max_total_per_period', 'first_charge_at', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'subscription.cancel': { aids: ['buyer', 'merchant'], money: [], itemsRequired: false,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'subscription_id', 'effective', 'principal_binding', 'nonce', 'iat', 'exp'] },
  'purchase.return': { aids: ['buyer', 'merchant'], money: [], itemsRequired: true,
    required: ['v', 'verb', 'intent_id', 'buyer', 'merchant', 'settler', 'original_settlement_id', 'items', 'desired_outcome', 'principal_binding', 'nonce', 'iat', 'exp'] },
};

function has(obj, key) {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

function validateIntent(intent) {
  if (typeof intent !== 'object' || intent === null) return { error: 'format.bad_schema' };
  if (!has(intent, 'v')) return { error: 'format.missing_field' };
  if (intent.v !== 'icp-1.0') return { error: 'version.unsupported' };
  if (!has(intent, 'verb')) return { error: 'format.missing_field' };
  const spec = INTENT_VERBS[intent.verb];
  if (!spec) return { error: 'format.unknown_verb' };
  for (const field of spec.required) {
    if (!has(intent, field)) return { error: 'format.missing_field' };
  }
  for (const field of spec.aids) {
    if (!AID_RE.test(String(intent[field]))) return { error: 'format.bad_aid' };
  }
  if (!SETTLER_RE.test(String(intent.settler))) return { error: 'format.bad_settler_id' };
  for (const field of spec.money) {
    const m = intent[field];
    if (typeof m !== 'object' || m === null || !MONEY_RE.test(String(m.amount)))
      return { error: 'format.bad_money' };
  }
  if (spec.itemsRequired) {
    if (!Array.isArray(intent.items) || intent.items.length < 1) return { error: 'format.bad_schema' };
  }
  return { valid: true };
}

function run05IntentValidation(input) {
  const validations = {};
  for (const c of input.cases) {
    validations[c.id] = validateIntent(c.intent);
  }
  return { validations };
}

// ===========================================================================
// 06-quote-binding — ICP-1.0 §11.4 max_total ceiling (exact decimal compare)
// ===========================================================================

// Compare two non-negative decimal strings (^[0-9]+(\.[0-9]{1,18})?$).
// Returns -1, 0, or 1. Exact — no Number/float conversion.
function cmpAmount(a, b) {
  const [ra, fa = ''] = a.split('.');
  const [rb, fb = ''] = b.split('.');
  const ia = ra.replace(/^0+/, '') || '0';
  const ib = rb.replace(/^0+/, '') || '0';
  if (ia.length !== ib.length) return ia.length < ib.length ? -1 : 1;
  if (ia !== ib) return ia < ib ? -1 : 1; // equal length → lexicographic == numeric
  const n = Math.max(fa.length, fb.length);
  const pa = fa.padEnd(n, '0');
  const pb = fb.padEnd(n, '0');
  if (pa === pb) return 0;
  return pa < pb ? -1 : 1;
}

function run06QuoteBinding(input) {
  const decisions = {};
  for (const c of input.cases) {
    const exceeds = cmpAmount(c.quote_total.amount, c.intent_max_total.amount) > 0;
    decisions[c.id] = exceeds ? { error: 'policy.quote.exceeds_max_total' } : { valid: true };
  }
  return { decisions };
}

// ===========================================================================
// 07-settlement-receipts — ICP-1.0 §9 co-signed receipt verification
// ===========================================================================

// Verify a raw-hex Ed25519 signature over `canonical` (same envelope trick as
// run03: wrap the raw pubkey in an SPKI header so node:crypto accepts it).
function verifyEd25519Hex(canonical, sigHex, pubkeyHex) {
  try {
    const sigBytes = Buffer.from(sigHex, 'hex');
    if (sigBytes.length !== 64) return false;
    const pubRaw = Buffer.from(pubkeyHex, 'hex');
    if (pubRaw.length !== 32) return false;
    const spkiPrefix = Buffer.from('302a300506032b6570032100', 'hex');
    const pubKey = createPublicKey({
      key: Buffer.concat([spkiPrefix, pubRaw]),
      format: 'der',
      type: 'spki',
    });
    return verify(null, Buffer.from(canonical), pubKey, sigBytes);
  } catch (_) {
    return false;
  }
}

function verifyReceipt(receipt, merchantPubkeyHex, settlerPubkeyHex) {
  if (typeof receipt !== 'object' || receipt === null) {
    return { error: 'format.missing_field' };
  }
  const ms = receipt.merchant_signature;
  if (!ms || !ms.sig) return { error: 'format.missing_field' };
  const ss = receipt.settler_signature;
  if (!ss || !ss.sig) return { error: 'format.missing_field' };

  // Strip both signature fields and re-canonicalize; the signer signed the
  // canonical bytes of the unsigned receipt body (§9).
  const { merchant_signature, settler_signature, ...unsigned } = receipt; // eslint-disable-line no-unused-vars
  const canonical = canonicalJson(unsigned);

  if (!verifyEd25519Hex(canonical, ms.sig, merchantPubkeyHex)) {
    return { error: 'signature.invalid' };
  }
  if (!verifyEd25519Hex(canonical, ss.sig, settlerPubkeyHex)) {
    return { error: 'settlement.settler_signature_invalid' };
  }
  return { valid: true };
}

function run07SettlementReceipts(input) {
  const verifications = {};
  for (const c of input.cases) {
    verifications[c.id] = verifyReceipt(c.receipt, c.merchant_pubkey_hex, c.settler_pubkey_hex);
  }
  return { verifications };
}

// ===========================================================================
// 08-timing — ICP-1.0 §5.3 replay window (strict parse + shared epoch algo)
// ===========================================================================

const TIMING_WINDOW_MAX = 600; // §5.3 intent window ceiling, seconds
const TS_RE = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$/;

// Howard Hinnant's days_from_civil — exact, no leap seconds, positive years.
function daysFromCivil(y, m, d) {
  const y2 = m <= 2 ? y - 1 : y;
  const era = Math.floor((y2 >= 0 ? y2 : y2 - 399) / 400);
  const yoe = y2 - era * 400;
  const doy = Math.floor((153 * (m > 2 ? m - 3 : m + 9) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

// Strict RFC-3339 second-precision UTC parser. Returns epoch seconds or null.
// Deliberately not Date.parse (lenient — accepts "2026-07-14" etc.).
function parseEpoch(s) {
  if (typeof s !== 'string') return null;
  const m = TS_RE.exec(s);
  if (!m) return null;
  const [y, mo, d, h, mi, se] = m.slice(1).map(Number);
  if (!(mo >= 1 && mo <= 12 && d >= 1 && d <= 31 && h <= 23 && mi <= 59 && se <= 59)) return null;
  return daysFromCivil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se;
}

function validateTiming(iat, exp, now) {
  const ti = parseEpoch(iat);
  const te = parseEpoch(exp);
  const tn = parseEpoch(now);
  if (ti === null || te === null || tn === null) return { error: 'replay.timestamp_malformed' };
  if (te - ti > TIMING_WINDOW_MAX) return { error: 'replay.window_too_long' };
  if (te < tn) return { error: 'replay.expired' };
  return { valid: true };
}

function run08Timing(input) {
  const validations = {};
  for (const c of input.cases) {
    validations[c.id] = validateTiming(c.iat, c.exp, c.now);
  }
  return { validations };
}

// ===========================================================================
// 09-ceilings — refund/payout authoritative ceilings (reuses cmpAmount)
// ===========================================================================

const CEILING_CODE = {
  return: 'policy.return.exceeds_max_refund',
  payout: 'policy.payout.exceeds_max_per_payout',
};

function run09Ceilings(input) {
  const decisions = {};
  for (const c of input.cases) {
    const exceeds = cmpAmount(c.value.amount, c.ceiling.amount) > 0;
    decisions[c.id] = exceeds ? { error: CEILING_CODE[c.kind] } : { valid: true };
  }
  return { decisions };
}


// ===========================================================================
// 10-commerce-invariants — economic invariants (reuses cmpAmount: no floats)
// ===========================================================================

const MINOR_UNITS = { USD: 2, EUR: 2, JPY: 0, USDC: 6 };

// Exact decimal addition on the string forms cmpAmount already understands.
function addAmount(a, b) {
  const [ra, fa = ''] = String(a).split('.');
  const [rb, fb = ''] = String(b).split('.');
  const n = Math.max(fa.length, fb.length);
  const scaled = BigInt(ra + fa.padEnd(n, '0')) + BigInt(rb + fb.padEnd(n, '0'));
  const digits = scaled.toString().padStart(n + 1, '0');
  return n === 0 ? digits : `${digits.slice(0, -n)}.${digits.slice(-n)}`;
}

// Exact decimal subtraction; returns "-x" when b > a so cmpAmount's caller can
// still order it against a non-negative request (a negative available always
// loses the >= comparison because the caller checks requested > available).
function subAmount(a, b) {
  const [ra, fa = ''] = String(a).split('.');
  const [rb, fb = ''] = String(b).split('.');
  const n = Math.max(fa.length, fb.length);
  const scaled = BigInt(ra + fa.padEnd(n, '0')) - BigInt(rb + fb.padEnd(n, '0'));
  const neg = scaled < 0n;
  const digits = (neg ? -scaled : scaled).toString().padStart(n + 1, '0');
  const body = n === 0 ? digits : `${digits.slice(0, -n)}.${digits.slice(-n)}`;
  return neg ? `-${body}` : body;
}

function run10CommerceInvariants(input) {
  const decisions = {};
  for (const c of input.cases) {
    let d;
    switch (c.kind) {
      case 'refund': {
        const used = addAmount(c.completed_refunds, c.inflight_refunds);
        d = cmpAmount(addAmount(used, c.requested), c.captured) > 0
          ? { error: 'commerce.refund.exceeds_captured' } : { valid: true };
        break;
      }
      case 'capture': {
        const used = addAmount(c.completed_captures, c.inflight_captures);
        d = cmpAmount(addAmount(used, c.requested), c.order_total) > 0
          ? { error: 'commerce.capture.exceeds_order_total' } : { valid: true };
        break;
      }
      case 'return_quantity': {
        if (c.shipped <= 0) d = { error: 'commerce.return.order_not_shipped' };
        else if (c.already_returned + c.requested > c.shipped) d = { error: 'commerce.return.exceeds_shipped' };
        else d = { valid: true };
        break;
      }
      case 'reserve': {
        const available = subAmount(c.on_hand, c.allocated);
        const insufficient =
          available.startsWith('-') || cmpAmount(c.requested, available) > 0;
        d = insufficient
          ? { error: 'commerce.inventory.insufficient_available' } : { valid: true };
        break;
      }
      case 'journal_entry': {
        const twoSided = c.lines.some(
          (l) => cmpAmount(l.debit, '0') > 0 && cmpAmount(l.credit, '0') > 0,
        );
        if (twoSided) { d = { error: 'commerce.ledger.line_not_single_sided' }; break; }
        const debits = c.lines.reduce((acc, l) => addAmount(acc, l.debit), '0');
        const credits = c.lines.reduce((acc, l) => addAmount(acc, l.credit), '0');
        d = cmpAmount(debits, credits) === 0
          ? { valid: true } : { error: 'commerce.ledger.entry_unbalanced' };
        break;
      }
      case 'money_scale': {
        const frac = String(c.amount).includes('.') ? String(c.amount).split('.')[1].length : 0;
        d = frac > MINOR_UNITS[c.currency]
          ? { error: 'commerce.money.scale_exceeds_currency' } : { valid: true };
        break;
      }
      default:
        throw new Error(`unknown case kind: ${c.kind}`);
    }
    decisions[c.id] = d;
  }
  return { decisions };
}

// ===========================================================================
// Main
// ===========================================================================

const testName = process.argv[2];
if (!testName) {
  process.stderr.write('FATAL: missing test name argument\n');
  process.exit(2);
}

let input;
try {
  input = JSON.parse(readFileSync(0, 'utf8'));
} catch (err) {
  process.stderr.write(`FATAL: invalid JSON on stdin: ${err.message}\n`);
  process.exit(2);
}

let output;
try {
  switch (testName) {
    case '01-aid-derivation':
      output = run01AidDerivation(input);
      break;
    case '02-canonical-json':
      output = run02CanonicalJson(input);
      break;
    case '03-signature-verification':
      output = run03SignatureVerification(input);
      break;
    case '04-escrow-lifecycle':
      output = run04EscrowLifecycle(input);
      break;
    case '05-intent-validation':
      output = run05IntentValidation(input);
      break;
    case '06-quote-binding':
      output = run06QuoteBinding(input);
      break;
    case '07-settlement-receipts':
      output = run07SettlementReceipts(input);
      break;
    case '08-timing':
      output = run08Timing(input);
      break;
    case '09-ceilings':
      output = run09Ceilings(input);
      break;
    case '10-commerce-invariants':
      output = run10CommerceInvariants(input);
      break;
    default:
      process.stderr.write(JSON.stringify({ error: 'unsupported', reason: `no handler for ${testName}` }) + '\n');
      process.exit(2);
  }
} catch (err) {
  process.stderr.write(`FATAL: adapter error: ${err.message}\n${err.stack}\n`);
  process.exit(1);
}

process.stdout.write(JSON.stringify(output, null, 2) + '\n');
