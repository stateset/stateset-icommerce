// Type declarations for @stateset/icp-client.
//
// Hand-authored to match `src/index.mjs` exactly. The .mjs source is
// the canonical implementation; this file describes its shape for
// TypeScript consumers. Keep this in sync — there's a drift-guard test
// in `test/types-sync.test.mjs` that asserts every JS export has a
// corresponding declaration here.

/**
 * Typed ICP-1.0 error. The `code` is a dotted-namespace string from
 * `icp-spec/schemas/error-codes.md` (e.g. `signature.invalid`,
 * `channel.replay`, `policy.settler.not_allowed`, ...).
 */
export class ICPError extends Error {
  constructor(code: string, message: string, details?: Record<string, unknown>);
  readonly code: string;
  readonly details: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Wire primitives
// ---------------------------------------------------------------------------

/** ISO 4217 fiat code or registered stablecoin code (e.g. "USDC"). */
export type Currency = string;

/** Decimal amount serialized as a string. */
export interface Money {
  amount: string;
  currency: Currency;
}

/** Agent Identifier per ICP-1.0 §4.2. `aid:v1:z` + Base58btc(SHA-256(...)). */
export type AID = string;

/** Detached signature envelope. */
export interface Signature {
  alg: "ed25519";
  /** Signer's AID, or "self" for principal-binding self-signatures. */
  kid: string;
  /** 64-byte Ed25519 signature, lowercase hex. */
  sig: string;
}

/** ICP-1.0 Agent identity: Ed25519 + X25519 keypairs + derived AID. */
export interface Identity {
  /** 32-byte Ed25519 seed (raw private key material). */
  ed25519_seed: Buffer;
  /** 32-byte X25519 seed. */
  x25519_seed: Buffer;
  /** 32-byte Ed25519 public key. */
  ed25519_pubkey: Buffer;
  /** 32-byte X25519 public key. */
  x25519_pubkey: Buffer;
  /** `aid:v1:z<base58btc>` */
  aid: AID;
}

// ---------------------------------------------------------------------------
// Identity helpers
// ---------------------------------------------------------------------------

/** Generate a fresh random Agent identity. Persist + reuse the seeds in production. */
export function generateIdentity(): Identity;

/** Reconstruct an Agent identity from 32-byte seeds. */
export function identityFromSeeds(edSeed: Buffer, xSeed: Buffer): Identity;

// ---------------------------------------------------------------------------
// Canonical JSON + Ed25519
// ---------------------------------------------------------------------------

/** RFC 8785 (JCS) canonical JSON. Used as the signing-input encoding. */
export function canonicalJson(value: unknown): string;

/** Sign canonical bytes with the identity's Ed25519 key. Returns lowercase hex. */
export function signEd25519(canonical: string, identity: Identity): string;

/**
 * Verify an Ed25519 signature against a raw 32-byte public key.
 * Returns `true` if the signature is valid, `false` on any failure.
 * Never throws.
 */
export function verifyEd25519(
  canonical: string,
  signatureHex: string,
  edPubRaw: Buffer,
): boolean;

// ---------------------------------------------------------------------------
// SettlementReceipt verification
// ---------------------------------------------------------------------------

/**
 * Co-signed final settlement receipt. The canonical bytes used as the
 * signing input are `canonicalJson(receipt)` with both `merchant_signature`
 * and `settler_signature` fields stripped — so both signatures cover
 * the same bytes.
 */
export interface SettlementReceipt {
  type: "icp.settlement.receipt";
  v: "icp-1.0";
  settlement_id: string;
  escrow_id: string;
  intent_id?: string;
  final_state: "released" | "refunded";
  amount: Money;
  rail?: string;
  rail_txid?: string;
  settled_at: string;  // RFC 3339
  released_to?: string;
  merchant_signature: Signature;
  settler_signature: Signature;
}

export interface VerifySettlementReceiptOptions {
  receipt: SettlementReceipt | Record<string, unknown>;
  merchantPubkeyRaw: Buffer;
  settlerPubkeyRaw: Buffer;
  /** Skip the settler-signature check. Default true. */
  requireSettler?: boolean;
}

/**
 * Verify a co-signed SettlementReceipt against both the merchant and
 * the Settler's published Ed25519 public keys. Returns the receipt
 * unchanged on success. Throws `ICPError`:
 *   - `format.missing_field` — receipt missing a signature field.
 *   - `signature.invalid` — merchant signature failed.
 *   - `settlement.settler_signature_invalid` — settler signature failed.
 */
export function verifySettlementReceipt(
  opts: VerifySettlementReceiptOptions,
): SettlementReceipt;

// ---------------------------------------------------------------------------
// ICPIP-0005 receiver helpers
// ---------------------------------------------------------------------------

/**
 * Inbound event types emitted by the handler per ICPIP-0005 §3.
 * Receivers MAY observe additional types defined by future ICPIPs.
 */
export type EventType =
  | "settlement.released"
  | "settlement.refunded"
  | "escrow.opened"
  | "escrow.refunded"
  | "dispute.opened"
  | "dispute.resolved"
  | "subscription.charge_pending"
  | "subscription.canceled"
  | "inventory.price_changed"
  | "inventory.stock_depleted"
  | "payout.released"
  | "compliance.kyb_due"
  | "risk.flag"
  | (string & {});  // open for future event types

/** Per ICPIP-0005 §2 — signed envelope returned by `verifyWebhook`. */
export interface EventEnvelope {
  v: "icp-1.0";
  event_id: string;
  event_type: EventType;
  channel_id: string;
  sequence: number;
  originated_at: string;  // RFC 3339
  source: AID;
  target: AID;
  payload: Record<string, unknown>;
  previous_event_id: string | null;
  delivery_attempt: number;
}

export interface VerifyWebhookOptions {
  /** Raw HTTP body string. MUST NOT be pre-parsed. */
  body: string;
  /** HTTP request headers (case-insensitive lookup). */
  headers: Record<string, string> | Headers | { get(name: string): string | null };
  /** HTTP method (typically "POST"). */
  method: string;
  /** HTTP path (include query string if the original request had one). */
  path: string;
  /** Raw 32-byte Ed25519 pubkey from the merchant's `.well-known/icp`. */
  merchantPubkeyRaw: Buffer;
  /** Default 300 (±5min replay window per spec). */
  toleranceSeconds?: number;
  /** Override "now" — for testing only. */
  nowSeconds?: number;
}

/**
 * Verify an inbound ICPIP-0005 webhook and return its parsed envelope.
 *
 * Throws `ICPError` with one of:
 *   - `channel.signature_invalid` — header missing, signature mismatch,
 *     malformed body, or envelope-signature failure.
 *   - `channel.replay` — HTTP timestamp outside the tolerance window.
 *
 * Stripe-style: hand the raw body + headers in, get back the envelope
 * (or a typed error).
 */
export function verifyWebhook(opts: VerifyWebhookOptions): EventEnvelope;

// ---------------------------------------------------------------------------
// ICPClient
// ---------------------------------------------------------------------------

export interface LineItem {
  sku: string;
  quantity: number;
  unit_price: Money;
}

export interface ICPClientCreateOptions {
  handlerUrl: string;
  principal: string;
  identity?: Identity;
  /** Default $500 USDC; per-Intent spend ceiling. */
  maxPerIntent?: Money;
  /** Verbs the Agent is authorized for. Defaults to all 7 commerce verbs. */
  verbs?: string[];
  /** Revocation URL the merchant uses to validate the binding. */
  revocationUrl?: string;
}

export interface SignedResponse<T = unknown> {
  signature: Signature;
  /** Verb-specific payload (`quote`, `snapshot`, `authorization`, etc.). */
  [payloadKey: string]: T | Signature | unknown;
}

export interface PurchaseOpts {
  merchant: AID;
  settler: string;
  items: LineItem[];
  max_total: Money;
  ship_to?: Record<string, unknown>;
  from_proposal_id?: string;
}

export interface InventoryOpts {
  merchant: AID;
  settler: string;
  skus?: Array<{ sku: string; quantity?: number }>;
  filters?: Record<string, unknown>;
  max_results?: number;
}

export interface SubscribeOpts {
  merchant: AID;
  settler: string;
  service_id: string;
  cadence: string;
  max_total_per_period: Money;
  max_occurrences?: number | null;
  first_charge_at: string;
}

export interface CancelOpts {
  merchant: AID;
  settler: string;
  subscription_id: string;
  effective?: "immediate" | "end-of-period";
  reason?: string;
}

export interface ReturnOpts {
  merchant: AID;
  settler: string;
  original_settlement_id: string;
  items: Array<{ sku: string; quantity: number; reason?: string }>;
  desired_outcome: "refund" | "replacement" | "credit" | "partial-refund";
  max_refund?: Money;
  narrative?: string;
}

export interface QuoteRequestOpts {
  merchant: AID;
  settler: string;
  items: Array<{
    sku: string;
    quantity: number;
    target_unit_price?: Money;
    specifications?: Record<string, unknown>;
  }>;
  ship_to?: Record<string, unknown>;
  expected_delivery_by?: string;
  purchase_window?: string;
  context?: string;
}

export interface RegisterWebhookOpts {
  merchant: AID;
  settler: string;
  type?: "webhook" | "sse";
  /** Required when type is "webhook"; omitted for "sse". */
  url?: string;
  event_filters?: EventType[];
  delivery?: {
    max_attempts?: number;
    backoff?: "exponential" | "constant";
    initial_delay_seconds?: number;
  };
  auth?: {
    scheme?: "ed25519" | "hmac-sha256";
    verifying_key_hex?: string;
  };
}

export interface FetchChannelEventsOpts {
  /** When true (default), verifies each envelope signature before returning. */
  verify?: boolean;
}

/**
 * High-level client for the ICP-1.0 merchant handler.
 *
 * Construct via `ICPClient.create(...)` so the merchant pubkey gets
 * cached from `.well-known/icp` before the first signed call.
 */
export class ICPClient {
  static create(opts: ICPClientCreateOptions): Promise<ICPClient>;

  readonly aid: AID;
  readonly identity: Identity;
  readonly handlerUrl: string;
  readonly principal: string;

  /** Fetch `.well-known/icp`. Caches the merchant pubkey internally. */
  capabilities(): Promise<Record<string, unknown>>;

  /** `inventory.query` — read-only SKU/price discovery. */
  inventory(opts: InventoryOpts): Promise<SignedResponse>;

  /** `purchase.create` — submit a signed purchase Intent, get a Quote. */
  purchase(opts: PurchaseOpts): Promise<SignedResponse>;

  /** Accept a Quote returned from `purchase()`. */
  accept(
    quoteId: string,
    body?: Record<string, unknown>,
  ): Promise<Record<string, unknown>>;

  /** `subscription.create` — open a recurring authorization. */
  subscribe(opts: SubscribeOpts): Promise<SignedResponse>;

  /** `subscription.cancel` — cancel an existing subscription. */
  cancel(opts: CancelOpts): Promise<SignedResponse>;

  /** `purchase.return` — request a refund / replacement / credit. */
  return_(opts: ReturnOpts): Promise<SignedResponse>;

  /** `quote.request` — B2B RFQ for non-binding pricing. */
  requestQuote(opts: QuoteRequestOpts): Promise<SignedResponse>;

  /** `channel.register` — register a webhook or SSE push channel (ICPIP-0005). */
  registerWebhook(opts: RegisterWebhookOpts): Promise<SignedResponse>;

  /**
   * ICPIP-0005 §5 recovery — fetch retained events for a channel with
   * `sequence > since`. Verifies each envelope by default.
   */
  fetchChannelEvents(
    channelId: string,
    since?: number,
    opts?: FetchChannelEventsOpts,
  ): Promise<EventEnvelope[]>;

  /** Async iterator over EscrowEvents for a given escrow (SSE). */
  observe(escrowId: string): AsyncIterableIterator<Record<string, unknown>>;

  /** Fetch a SettlementReceipt by id. */
  settlement(settlementId: string): Promise<Record<string, unknown>>;
}
