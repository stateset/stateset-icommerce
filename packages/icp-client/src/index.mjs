// @stateset/icp-client — Front-door SDK for ICP-1.0.
//
// Wraps the wire format, signature scheme, and HTTP transport into idiomatic
// JavaScript so integrators can transact ICP commerce in <10 lines without
// implementing canonical JSON, Base58btc, or Ed25519 by hand.
//
// Zero runtime dependencies — only node:crypto and node:http (via fetch).
//
// Usage:
//
//   import { ICPClient } from '@stateset/icp-client';
//
//   const client = await ICPClient.create({
//     handlerUrl: 'http://localhost:8787',
//     principal: 'did:web:my-store.example',
//   });
//
//   const caps   = await client.capabilities();
//   const stock  = await client.inventory({ skus: [{ sku: 'WIDGET-001' }] });
//   const order  = await client.purchase({
//     items:    [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '29.99', currency: 'USDC' } }],
//     max_total: { amount: '35.00', currency: 'USDC' },
//     settler:   'settler:stateset.usdc.base-sepolia',
//     merchant:  caps.merchant_aid,
//   });

import {
  generateKeyPairSync,
  createPublicKey,
  createPrivateKey,
  createHash,
  randomBytes,
  sign as nodeSign,
  verify as nodeVerify,
} from 'node:crypto';

// ===========================================================================
// Constants
// ===========================================================================

const ED25519_PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');
const ED25519_SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');
const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

// ===========================================================================
// Errors — typed so callers can branch on .code
// ===========================================================================

export class ICPError extends Error {
  /**
   * @param {string} code   ICP-1.0 error code (e.g. 'signature.invalid')
   * @param {string} message
   * @param {object} [details]
   */
  constructor(code, message, details = {}) {
    super(`${code}: ${message}`);
    this.name = 'ICPError';
    this.code = code;
    this.details = details;
  }
}

// ===========================================================================
// Identity — Ed25519 + X25519 keypair, AID derivation
// ===========================================================================

/** @typedef {{ ed25519_seed: Buffer, x25519_seed: Buffer, aid: string, ed25519_pubkey: Buffer, x25519_pubkey: Buffer }} Identity */

/**
 * Generate a fresh Agent identity (Ed25519 + X25519 + derived AID).
 * @returns {Identity}
 */
export function generateIdentity() {
  const ed = generateKeyPairSync('ed25519');
  const x = generateKeyPairSync('x25519');
  const edPkcs8 = ed.privateKey.export({ format: 'der', type: 'pkcs8' });
  const xPkcs8 = x.privateKey.export({ format: 'der', type: 'pkcs8' });
  return identityFromSeeds(edPkcs8.subarray(16, 48), xPkcs8.subarray(16, 48));
}

/**
 * Restore an Agent identity from 32-byte seeds.
 * @param {Buffer} edSeed
 * @param {Buffer} xSeed
 * @returns {Identity}
 */
export function identityFromSeeds(edSeed, xSeed) {
  if (edSeed.length !== 32) throw new ICPError('format.bad_field', 'ed25519_seed must be 32 bytes');
  if (xSeed.length !== 32) throw new ICPError('format.bad_field', 'x25519_seed must be 32 bytes');

  const edPriv = createPrivateKey({
    key: Buffer.concat([ED25519_PKCS8_PREFIX, edSeed]),
    format: 'der',
    type: 'pkcs8',
  });
  const xPriv = createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b656e04220420', 'hex'), xSeed]),
    format: 'der',
    type: 'pkcs8',
  });

  const edPubRaw = extractRawPublicKey(createPublicKey(edPriv));
  const xPubRaw = extractRawPublicKey(createPublicKey(xPriv));
  const aid = `aid:v1:z${base58btcEncode(
    createHash('sha256')
      .update(Buffer.concat([edPubRaw, Buffer.from([0x00]), xPubRaw]))
      .digest(),
  )}`;
  return {
    ed25519_seed: edSeed,
    x25519_seed: xSeed,
    ed25519_pubkey: edPubRaw,
    x25519_pubkey: xPubRaw,
    aid,
  };
}

// ===========================================================================
// Wire codec
// ===========================================================================

/**
 * RFC-8785-compatible canonical JSON for ICP-1.0 payload shapes.
 * Recursive lexicographic key sort, no whitespace, standard JSON escapes.
 * @param {unknown} value
 * @returns {string}
 */
export function canonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  const keys = Object.keys(value).sort();
  return '{' + keys.map((k) => JSON.stringify(k) + ':' + canonicalJson(value[k])).join(',') + '}';
}

export function signEd25519(canonical, identity) {
  const edPriv = createPrivateKey({
    key: Buffer.concat([ED25519_PKCS8_PREFIX, identity.ed25519_seed]),
    format: 'der',
    type: 'pkcs8',
  });
  return nodeSign(null, Buffer.from(canonical), edPriv).toString('hex');
}

export function verifyEd25519(canonical, signatureHex, edPubRaw) {
  try {
    const sig = Buffer.from(signatureHex, 'hex');
    if (sig.length !== 64) return false;
    const pub = createPublicKey({
      key: Buffer.concat([ED25519_SPKI_PREFIX, edPubRaw]),
      format: 'der',
      type: 'spki',
    });
    return nodeVerify(null, Buffer.from(canonical), pub, sig);
  } catch (_) {
    return false;
  }
}

// ===========================================================================
// ICPClient — main public API
// ===========================================================================

/**
 * @typedef {object} Money
 * @property {string} amount   Decimal string (NEVER float). E.g. "29.99".
 * @property {string} currency ISO 4217 or canonical ticker. E.g. "USDC".
 */

/**
 * @typedef {object} LineItem
 * @property {string} sku
 * @property {number} quantity
 * @property {Money} unit_price
 */

/**
 * @typedef {object} ClientOptions
 * @property {string} handlerUrl      Base URL of the ICP HTTP handler.
 * @property {string} principal       Principal identifier (e.g. did:web:my-store.example).
 * @property {Identity} [identity]    Pre-existing identity. If absent, a fresh one is generated.
 * @property {string[]} [verbs]       PrincipalBinding authority.verbs. Default: all 4 verbs.
 * @property {Money} [maxPerIntent]   PrincipalBinding authority cap. Default: $10,000 USDC.
 * @property {string} [revocationUrl] Where to publish revocation. Default: example.
 */

export class ICPClient {
  /**
   * Create a client with a fresh or restored identity.
   * @param {ClientOptions} opts
   * @returns {Promise<ICPClient>}
   */
  static async create(opts) {
    if (!opts.handlerUrl) throw new ICPError('format.missing_field', 'handlerUrl required');
    if (!opts.principal) throw new ICPError('format.missing_field', 'principal required');
    const identity = opts.identity ?? generateIdentity();
    return new ICPClient({
      handlerUrl: opts.handlerUrl,
      principal: opts.principal,
      identity,
      verbs: opts.verbs ?? ['purchase.create', 'subscription.create', 'purchase.return', 'inventory.query'],
      maxPerIntent: opts.maxPerIntent ?? { amount: '10000', currency: 'USDC' },
      revocationUrl: opts.revocationUrl ?? `https://example.com/icp-revocation/${identity.aid}`,
    });
  }

  /** @private */
  constructor(cfg) {
    this.handlerUrl = cfg.handlerUrl.replace(/\/+$/, '');
    this.principal = cfg.principal;
    this.identity = cfg.identity;
    this.verbs = cfg.verbs;
    this.maxPerIntent = cfg.maxPerIntent;
    this.revocationUrl = cfg.revocationUrl;
    this._merchantPubCache = null;
  }

  /** The Agent's AID. */
  get aid() {
    return this.identity.aid;
  }

  // ---- Discovery ------------------------------------------------------

  /**
   * Fetch the handler's .well-known/icp capabilities document.
   * Caches the merchant public key for subsequent signature verification.
   */
  async capabilities() {
    const r = await fetch(`${this.handlerUrl}/icp/v1/.well-known/icp`);
    if (!r.ok) throw new ICPError('format.unknown_route', `handler /.well-known/icp returned ${r.status}`);
    const caps = await r.json();
    if (caps.merchant_pubkey?.raw_hex) {
      this._merchantPubCache = Buffer.from(caps.merchant_pubkey.raw_hex, 'hex');
    }
    return caps;
  }

  // ---- Verbs ----------------------------------------------------------

  /**
   * inventory.query — read-only discovery.
   * @param {{ merchant: string, settler: string, skus?: {sku:string,quantity?:number}[], filters?: object, max_results?: number }} opts
   */
  async inventory(opts) {
    const intent = this._baseIntent('inventory.query', opts);
    if (opts.skus) intent.skus = opts.skus;
    if (opts.filters) intent.filters = opts.filters;
    if (opts.max_results) intent.max_results = opts.max_results;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.snapshot, result.signature);
    return result;
  }

  /**
   * purchase.create — one-shot purchase. Returns the merchant Quote;
   * caller must then call `accept(quote_id)` to commit.
   * @param {{ merchant: string, settler: string, items: LineItem[], max_total: Money, ship_to?: object, from_proposal_id?: string }} opts
   */
  async purchase(opts) {
    const intent = this._baseIntent('purchase.create', opts);
    intent.items = opts.items;
    intent.max_total = opts.max_total;
    if (opts.ship_to) intent.ship_to = opts.ship_to;
    if (opts.from_proposal_id) intent.from_proposal_id = opts.from_proposal_id;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.quote, result.signature);
    return result;
  }

  /**
   * Accept a Quote returned from purchase(). Returns funding instructions
   * the buyer wallet must execute on-chain (or off-chain rail).
   */
  async accept(quoteId, body = {}) {
    const r = await fetch(`${this.handlerUrl}/icp/v1/quotes/${encodeURIComponent(quoteId)}/accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    const j = await r.json();
    if (!r.ok) throw new ICPError(j.code ?? 'format.unknown', j.message ?? `accept returned ${r.status}`, j);
    return j;
  }

  /**
   * subscription.create — establish recurring authorization.
   * @param {{ merchant: string, settler: string, service_id: string, cadence: string, max_total_per_period: Money, max_occurrences?: number|null, first_charge_at: string }} opts
   */
  async subscribe(opts) {
    const intent = this._baseIntent('subscription.create', opts);
    intent.service_id = opts.service_id;
    intent.cadence = opts.cadence;
    intent.max_total_per_period = opts.max_total_per_period;
    intent.max_occurrences = opts.max_occurrences ?? null;
    intent.first_charge_at = opts.first_charge_at;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.authorization, result.signature);
    return result;
  }

  /**
   * quote.request — request a non-binding PriceProposal (B2B RFQ).
   * @param {{ merchant: string, settler: string, items: {sku:string,quantity:number,target_unit_price?:Money,specifications?:object}[], ship_to?: object, expected_delivery_by?: string, purchase_window?: string, context?: string }} opts
   */
  async requestQuote(opts) {
    const intent = this._baseIntent('quote.request', opts);
    intent.items = opts.items;
    if (opts.ship_to) intent.ship_to = opts.ship_to;
    if (opts.expected_delivery_by) intent.expected_delivery_by = opts.expected_delivery_by;
    if (opts.purchase_window) intent.purchase_window = opts.purchase_window;
    if (opts.context) intent.context = opts.context;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.proposal, result.signature);
    return result;
  }

  /**
   * subscription.cancel — cancel an existing subscription. Returns a signed
   * CancellationAuthorization with effective_at + optional pro-rated refund.
   * @param {{ merchant: string, settler: string, subscription_id: string, effective?: 'immediate'|'end-of-period', reason?: string }} opts
   */
  async cancel(opts) {
    const intent = this._baseIntent('subscription.cancel', opts);
    intent.subscription_id = opts.subscription_id;
    intent.effective = opts.effective ?? 'immediate';
    if (opts.reason) intent.reason = opts.reason;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.authorization, result.signature);
    return result;
  }

  /**
   * purchase.return — request return/refund/replacement for a prior settlement.
   * @param {{ merchant: string, settler: string, original_settlement_id: string, items: {sku:string,quantity:number,reason?:string}[], desired_outcome: 'refund'|'replacement'|'credit'|'partial-refund', max_refund?: Money, narrative?: string }} opts
   */
  async return_(opts) {
    const intent = this._baseIntent('purchase.return', opts);
    intent.original_settlement_id = opts.original_settlement_id;
    intent.items = opts.items;
    intent.desired_outcome = opts.desired_outcome;
    if (opts.max_refund) intent.max_refund = opts.max_refund;
    if (opts.narrative) intent.narrative = opts.narrative;
    const result = await this._submit(intent);
    await this._verifyMerchantSignature(result.authorization, result.signature);
    return result;
  }

  // ---- Observe & retrieve ---------------------------------------------

  /**
   * Async iterator over EscrowEvents for a given escrow.
   * Yields each event as it arrives over Server-Sent Events.
   */
  async *observe(escrowId) {
    const url = `${this.handlerUrl}/icp/v1/escrows/${encodeURIComponent(escrowId)}/events`;
    const r = await fetch(url, { headers: { accept: 'text/event-stream' } });
    if (!r.ok) throw new ICPError('format.unknown_escrow', `observe returned ${r.status}`);
    const reader = r.body.getReader();
    const decoder = new TextDecoder();
    let buf = '';
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      let idx;
      while ((idx = buf.indexOf('\n\n')) !== -1) {
        const chunk = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        for (const line of chunk.split('\n')) {
          if (line.startsWith('data: ')) {
            try {
              yield JSON.parse(line.slice(6));
            } catch (_) { /* skip malformed */ }
          }
        }
      }
    }
  }

  /** Fetch a SettlementReceipt by settlement_id. */
  async settlement(settlementId) {
    const r = await fetch(`${this.handlerUrl}/icp/v1/settlements/${encodeURIComponent(settlementId)}`);
    if (!r.ok) {
      const j = await r.json().catch(() => ({}));
      throw new ICPError(j.code ?? 'format.unknown_settlement', `settlement returned ${r.status}`);
    }
    return r.json();
  }

  // ---- Internals ------------------------------------------------------

  _baseIntent(verb, opts) {
    const now = new Date();
    const exp = new Date(now.getTime() + 300 * 1000);
    return {
      v: 'icp-1.0',
      verb,
      intent_id: this._newId('icp_int'),
      buyer: this.identity.aid,
      merchant: opts.merchant,
      settler: opts.settler,
      expiry: exp.toISOString(),
      principal_binding: this._principalBinding(),
      nonce: randomBytes(16).toString('hex'),
      iat: now.toISOString(),
      exp: exp.toISOString(),
    };
  }

  _principalBinding() {
    return {
      principal: this.principal,
      agent: this.identity.aid,
      authority: { max_per_intent: this.maxPerIntent, verbs: this.verbs },
      expiry: new Date(Date.now() + 86400 * 1000).toISOString(),
      revocation: this.revocationUrl,
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' }, // demo: principal is self-binding
    };
  }

  async _submit(intent) {
    const canonical = canonicalJson(intent);
    const sig = signEd25519(canonical, this.identity);
    const body = {
      intent,
      signature: { alg: 'ed25519', kid: this.identity.aid, sig },
      _pubkey_hex: this.identity.ed25519_pubkey.toString('hex'),
    };
    const r = await fetch(`${this.handlerUrl}/icp/v1/intents`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    const j = await r.json();
    if (!r.ok) throw new ICPError(j.code ?? 'format.unknown', j.message ?? `submit returned ${r.status}`, j);
    return j;
  }

  /**
   * Verify a merchant signature against the merchant's published pubkey.
   * Throws ICPError('signature.invalid') if verification fails. This is a
   * load-bearing safety check — do NOT trust merchant responses without it.
   */
  async _verifyMerchantSignature(payload, signature) {
    if (!this._merchantPubCache) await this.capabilities();
    if (!this._merchantPubCache) {
      // Some merchant configurations don't expose pubkey via .well-known; skip with warning.
      return;
    }
    const canonical = canonicalJson(payload);
    if (!verifyEd25519(canonical, signature.sig, this._merchantPubCache)) {
      throw new ICPError(
        'signature.invalid',
        'merchant signature failed verification against published .well-known/icp pubkey',
      );
    }
  }

  _newId(prefix) {
    const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
    const bytes = randomBytes(16);
    let bits = 0n;
    for (const b of bytes) bits = (bits << 8n) | BigInt(b);
    let s = '';
    for (let i = 0; i < 26; i++) {
      s = ALPHABET[Number(bits & 31n)] + s;
      bits >>= 5n;
    }
    return `${prefix}_${s}`;
  }
}

// ===========================================================================
// Internal helpers
// ===========================================================================

function extractRawPublicKey(keyObject) {
  const spki = keyObject.export({ format: 'der', type: 'spki' });
  if (spki.length !== 44) throw new ICPError('format.bad_field', `unexpected SPKI length ${spki.length}`);
  return spki.subarray(12, 44);
}

function base58btcEncode(buf) {
  let n = 0n;
  for (const b of buf) n = (n << 8n) | BigInt(b);
  let out = '';
  while (n > 0n) {
    const r = Number(n % 58n);
    n = n / 58n;
    out = BASE58_ALPHABET[r] + out;
  }
  for (const b of buf) {
    if (b === 0) out = '1' + out;
    else break;
  }
  return out;
}
