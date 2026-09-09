// Bounded nonce replay guard — ICP-1.0-DRAFT §5.3 ("MUST reject any message
// with an already-seen nonce within the `exp` window").
//
// The spec requires verifiers to maintain a nonce cache for at least
// `max(iat + 86400s)`. This is a self-contained, zero-dependency
// LRU+TTL set keyed on `(aid, nonce)`:
//
//   - TTL: entries older than `ttlMs` are treated as expired and evicted
//     lazily on access (so the guard never resurrects a stale nonce after
//     its replay window has closed).
//   - Hard bound, fail closed, charged PER SIGNER: each signer AID may hold
//     at most `maxEntriesPerSigner` live nonces; when a signer is full of
//     LIVE entries, its new nonces are REJECTED rather than admitted by
//     evicting an old one. Evicting a live nonce would let an attacker flood
//     fresh nonces to flush a victim's nonce out of the window and replay it
//     — §5.3's MUST holds for the entire window, so the guard sacrifices
//     availability rather than integrity. Charging the cap to the signer
//     keeps that sacrifice local: one flooding agent can only lock itself
//     out, never every other agent on the handler. A separate `maxSigners`
//     bound stops freshly minted AIDs from growing the guard without limit.
//     Expired entries are evicted first and always free space.
//
// We intentionally do NOT use `cli/src/offline.js`'s OfflineCache: the
// handler is a zero-external-dependency package, and a replay guard wants
// FIFO-by-insertion eviction keyed on a composite (aid, nonce) string with
// a hard TTL, which is a different contract than a generic value cache.

const DEFAULT_TTL_MS = 86_400_000; // 24h — the §5.3 floor for long-running transitions
const DEFAULT_MAX_ENTRIES_PER_SIGNER = 1_000;
const DEFAULT_MAX_SIGNERS = 100_000;
// Last-resort memory backstop. The per-signer cap times the signer bound is a
// very large product, so a separate absolute ceiling keeps a single process
// from growing without limit under abuse. Reaching it is already pathological.
const DEFAULT_MAX_TOTAL_ENTRIES = 1_000_000;

export class ReplayGuard {
  /**
   * @param {object} [opts]
   * @param {number} [opts.ttlMs]               Entry lifetime; defaults to 24h (§5.3 floor).
   * @param {number} [opts.maxEntriesPerSigner] Live nonces retained per signer AID.
   * @param {number} [opts.maxEntries]          Deprecated spelling of `maxEntriesPerSigner`.
   * @param {number} [opts.maxSigners]          Distinct signer AIDs holding live nonces.
   * @param {number} [opts.maxTotalEntries]     Absolute ceiling on retained entries.
   * @param {() => number} [opts.now]           Clock injection for tests.
   */
  constructor(opts = {}) {
    this.ttlMs = opts.ttlMs ?? DEFAULT_TTL_MS;
    this.maxEntriesPerSigner =
      opts.maxEntriesPerSigner ?? opts.maxEntries ?? DEFAULT_MAX_ENTRIES_PER_SIGNER;
    this.maxSigners = opts.maxSigners ?? DEFAULT_MAX_SIGNERS;
    this.maxTotalEntries = opts.maxTotalEntries ?? DEFAULT_MAX_TOTAL_ENTRIES;
    if (!Number.isFinite(this.ttlMs) || this.ttlMs <= 0) {
      throw new Error('ReplayGuard ttlMs must be a positive finite number');
    }
    if (!Number.isInteger(this.maxEntriesPerSigner) || this.maxEntriesPerSigner <= 0) {
      throw new Error('ReplayGuard maxEntriesPerSigner must be a positive integer');
    }
    if (!Number.isInteger(this.maxSigners) || this.maxSigners <= 0) {
      throw new Error('ReplayGuard maxSigners must be a positive integer');
    }
    if (!Number.isInteger(this.maxTotalEntries) || this.maxTotalEntries <= 0) {
      throw new Error('ReplayGuard maxTotalEntries must be a positive integer');
    }
    this._now = opts.now ?? Date.now;
    // Map<string, { aid: string, ts: number }> — composite key → insertion record.
    // Insertion order is monotonic in `ts`, which the expiry sweep relies on.
    this._seen = new Map();
    // Map<string, number> — signer AID → live entry count, so the cap can be
    // charged to the signer that created the entries.
    this._perSigner = new Map();
  }

  /** Composite key. `nonce` is treated as an opaque string. The separator is
   * NUL, which cannot appear in either half, so no (aid, nonce) split can
   * alias another pair. */
  static key(aid, nonce) {
    return `${aid}\0${nonce}`;
  }

  /**
   * Record a `(aid, nonce)` pair. Returns `true` if this is the FIRST time
   * the pair has been seen within the live window (caller MAY proceed), or
   * `false` if the message must be rejected — a replay (`replay.nonce_seen`),
   * a signer at its own capacity, or a new signer beyond the signer bound.
   *
   * Side effects: evicts expired entries it encounters. Calling this exactly
   * once per verified message is the intended contract — it is NOT
   * idempotent (a second call with the same pair within the TTL returns
   * `false`).
   */
  checkAndRecord(aid, nonce) {
    const now = this._now();
    const k = ReplayGuard.key(aid, nonce);

    const entry = this._seen.get(k);
    if (entry !== undefined) {
      if (now - entry.ts < this.ttlMs) {
        // Live duplicate → replay.
        return false;
      }
      // Stale entry past its window — drop it and treat as first-seen.
      this._forget(k, entry.aid);
    }

    this._evictExpired(now);
    if (this._seen.size >= this.maxTotalEntries) {
      // Absolute memory backstop; still fails closed rather than evicting a
      // live nonce, which would re-open the replay window §5.3 keeps shut.
      return false;
    }
    const live = this._perSigner.get(aid) ?? 0;
    if (live >= this.maxEntriesPerSigner) {
      // This signer is full of LIVE nonces → fail closed for this signer only.
      return false;
    }
    if (live === 0 && this._perSigner.size >= this.maxSigners) {
      // Admitting an unbounded number of freshly minted AIDs would let a
      // caller grow the guard without limit, so new signers fail closed too.
      return false;
    }

    this._seen.set(k, { aid, ts: now });
    this._perSigner.set(aid, live + 1);
    return true;
  }

  /** Number of live entries currently retained (after lazy eviction). */
  size() {
    this._evictExpired(this._now());
    return this._seen.size;
  }

  /** Live entries retained for one signer AID (after lazy eviction). */
  sizeFor(aid) {
    this._evictExpired(this._now());
    return this._perSigner.get(aid) ?? 0;
  }

  /** Drop everything — used by tests. */
  clear() {
    this._seen.clear();
    this._perSigner.clear();
  }

  // -------------------------------------------------------------------------
  // Internal eviction
  // -------------------------------------------------------------------------

  _forget(key, aid) {
    this._seen.delete(key);
    const live = (this._perSigner.get(aid) ?? 1) - 1;
    if (live > 0) this._perSigner.set(aid, live);
    else this._perSigner.delete(aid);
  }

  /**
   * Evict entries older than `ttlMs`. Because the Map preserves insertion
   * order and every entry's timestamp is monotonic with insertion, we can
   * stop at the first non-expired entry.
   */
  _evictExpired(now) {
    for (const [k, entry] of this._seen) {
      if (now - entry.ts < this.ttlMs) break; // first live entry → rest are live
      this._forget(k, entry.aid);
    }
  }
}
