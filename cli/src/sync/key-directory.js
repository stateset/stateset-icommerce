/**
 * Peer Key Directory
 *
 * Resolves another agent's signing key so the receive path can verify what
 * that agent wrote. Keys come from the sequencer's signed directory, are
 * cached with a TTL, and are pinned on first use: a changed public key for an
 * already-pinned key_id is refused, because a sequencer that can silently swap
 * key material can forge any agent's events.
 *
 * The TTL fallback here (3600s) matches `DEFAULT_CONFIG.peerKeyTtlSeconds` in
 * `sync/config.js`, which is the source of truth callers normally go through.
 */

const DEFAULT_TTL_SECONDS = 3600;
const DEFAULT_MAX_STALE_SECONDS = 86400;

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
   * @param {string} agentId
   * @param {number} keyId
   * @param {string} atTime - the event's createdAt, ISO 8601
   * @returns {Promise<{publicKey: string, publicKeyBundle: Object|null}|{error: string}>}
   */
  async resolve(agentId, keyId, atTime) {
    let keys = this.outbox.getPeerKeys(agentId);
    const fetchedAt = keys.length ? Date.parse(keys[0].fetchedAt) : 0;
    const ageSeconds = (Date.now() - fetchedAt) / 1000;

    if (!keys.length || ageSeconds >= this.ttlSeconds) {
      try {
        keys = await this.refresh(agentId);
      } catch {
        // A directory outage must not stop verification of keys we already
        // hold — but we will not trust them forever.
        if (!keys.length || ageSeconds >= this.maxStaleSeconds) {
          return { error: 'key_unresolved' };
        }
      }
    }

    const key = keys.find((k) => k.keyId === keyId);
    if (!key) return { error: 'key_unresolved' };

    const pin = this.outbox.getPeerKeyPin(agentId, keyId);
    if (pin && pin.publicKey !== key.publicKey) {
      return { error: 'peer_key_conflict' };
    }

    const at = Date.parse(atTime);
    if (key.revokedAt && at >= Date.parse(key.revokedAt)) {
      return { error: 'key_revoked' };
    }
    if (key.validFrom && at < Date.parse(key.validFrom)) {
      return { error: 'key_outside_validity_window' };
    }
    if (key.validTo && at > Date.parse(key.validTo)) {
      return { error: 'key_outside_validity_window' };
    }

    if (!pin) this.outbox.pinPeerKey(agentId, keyId, key.publicKey);

    return { publicKey: key.publicKey, publicKeyBundle: key.publicKeyBundle };
  }

  /**
   * Fetch and cache the directory for one agent.
   * @param {string} agentId
   * @returns {Promise<Array<Object>>}
   */
  async refresh(agentId) {
    const directory = await this.client.getAgentSigningKeys(agentId);
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
