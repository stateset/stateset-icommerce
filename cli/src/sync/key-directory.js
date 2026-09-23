/**
 * Peer Key Directory
 *
 * Resolves another agent's signing key so the receive path can verify what
 * that agent wrote. Keys come from the sequencer's signed directory, are
 * cached with a TTL, and are pinned on first use: a changed public key for an
 * already-pinned key_id is refused, because a sequencer that can silently swap
 * key material can forge any agent's events.
 *
 * The TTL fallback here (300s) matches `DEFAULT_CONFIG.peerKeyTtlSeconds` in
 * `sync/config.js`, which is the source of truth callers normally go through.
 */

const DEFAULT_TTL_SECONDS = 300;
const DEFAULT_MAX_STALE_SECONDS = 86400;

/**
 * Refusals that are NOT a directory outage, and so must not be served from the
 * stale cache. `sequencer_key_not_configured` is a local misconfiguration that
 * silently serving cached keys would hide; `directory_untrusted` is a forged,
 * misdirected, replayed or unattested response, which the spec makes a hard
 * refusal of the whole response rather than a reason to fall back.
 */
const HARD_REFUSAL_CODES = new Set(['sequencer_key_not_configured', 'directory_untrusted']);

/**
 * Map a directory-fetch failure to the quarantine reason an operator should
 * act on. Anything without a recognised code is a genuine failure to obtain
 * the key — an unreachable sequencer, a socket reset — and stays
 * `key_unresolved`.
 *
 * @param {unknown} error
 * @returns {'sequencer_key_not_configured'|'directory_untrusted'|'key_unresolved'}
 */
function refreshFailureReason(error) {
  const code = /** @type {{code?: string}} */ (error)?.code;
  return HARD_REFUSAL_CODES.has(code) ? /** @type {any} */ (code) : 'key_unresolved';
}

export class PeerKeyDirectory {
  /**
   * @param {import('./outbox.js').Outbox} outbox
   * @param {{getAgentSigningKeys: (agentId: string) => Promise<Object>}} client
   * @param {Object} [config]
   */
  constructor(outbox, client, config = {}) {
    this.outbox = outbox;
    this.client = client;
    this.ttlSeconds = config.peerKeyTtlSeconds ?? DEFAULT_TTL_SECONDS;
    this.maxStaleSeconds = config.peerKeyMaxStaleSeconds ?? DEFAULT_MAX_STALE_SECONDS;
  }

  /**
   * Resolve the key an event claims to be signed by.
   *
   * On failure the returned `error` is the quarantine reason and `detail` is
   * the underlying message, so the caller can surface the real cause (a
   * missing config line reads very differently from a forged directory) even
   * though only the reason is durable.
   *
   * @param {string} agentId
   * @param {number} keyId
   * @param {string} atTime - the event's createdAt, ISO 8601
   * @returns {Promise<{publicKey: string, publicKeyBundle: Object|null}|{error: string, detail?: string}>}
   */
  async resolve(agentId, keyId, atTime) {
    let keys = this.outbox.getPeerKeys(agentId);
    const fetchedAt = keys.length ? Date.parse(keys[0].fetchedAt) : 0;
    const ageSeconds = (Date.now() - fetchedAt) / 1000;

    if (!keys.length || ageSeconds >= this.ttlSeconds) {
      try {
        keys = await this.refresh(agentId);
      } catch (error) {
        const reason = refreshFailureReason(error);
        // A directory outage must not stop verification of keys we already
        // hold — but we will not trust them forever, and a hard refusal
        // (misconfigured, forged, misdirected, stale, unattested) is not an
        // outage and gets no cache fallback at all.
        if (reason !== 'key_unresolved' || !keys.length || ageSeconds >= this.maxStaleSeconds) {
          return { error: reason, detail: error?.message };
        }
      }
    }

    const key = keys.find((k) => k.keyId === keyId);
    if (!key) {
      return {
        error: 'key_unresolved',
        detail: `agent ${agentId} has no key_id ${keyId} in its directory`,
      };
    }

    const pin = this.outbox.getPeerKeyPin(agentId, keyId);
    if (pin && pin.publicKey !== key.publicKey) {
      return {
        error: 'peer_key_conflict',
        detail: `the directory now presents a different public key for agent ${agentId} key_id ${keyId} than the one pinned on first use`,
      };
    }

    const at = Date.parse(atTime);
    if (key.revokedAt && at >= Date.parse(key.revokedAt)) {
      return { error: 'key_revoked', detail: `key_id ${keyId} was revoked at ${key.revokedAt}` };
    }
    if (key.validFrom && at < Date.parse(key.validFrom)) {
      return {
        error: 'key_outside_validity_window',
        detail: `event predates key_id ${keyId} validFrom ${key.validFrom}`,
      };
    }
    if (key.validTo && at > Date.parse(key.validTo)) {
      return {
        error: 'key_outside_validity_window',
        detail: `event postdates key_id ${keyId} validTo ${key.validTo}`,
      };
    }

    if (!pin) this.outbox.pinPeerKey(agentId, keyId, key.publicKey);

    return { publicKey: key.publicKey, publicKeyBundle: key.publicKeyBundle };
  }

  /**
   * Fetch and cache the directory for one agent.
   *
   * A directory that was not cryptographically attested is refused before it
   * can be cached. `UnifiedSequencerClient#getAgentSigningKeys` returns
   * `verified: false` over gRPC, where there is no directory-signature check
   * at all and the keys are trusted only because the channel is — caching
   * those would make an unattested response the anchor for every subsequent
   * event verification, which is exactly the trust the REST path refuses to
   * grant without a signature.
   *
   * @param {string} agentId
   * @returns {Promise<Array<Object>>}
   */
  async refresh(agentId) {
    const directory = await this.client.getAgentSigningKeys(agentId);
    if (directory?.verified === false) {
      const error = new Error(
        'Key directory is not cryptographically attested (the transport did not verify a ' +
          'directorySignature); refusing to cache peer keys. The gRPC receive path is ' +
          'unsupported — use an https:// sequencer URL.',
      );
      error.code = 'directory_untrusted';
      throw error;
    }
    const keys = directory.keys || [];
    this.outbox.upsertPeerKeys(agentId, keys, new Date().toISOString());
    return this.outbox.getPeerKeys(agentId);
  }
}

/**
 * @param {import('./outbox.js').Outbox} outbox
 * @param {Object} client
 * @param {Object} [config]
 * @returns {PeerKeyDirectory}
 */
export function createPeerKeyDirectory(outbox, client, config = {}) {
  return new PeerKeyDirectory(outbox, client, config);
}
