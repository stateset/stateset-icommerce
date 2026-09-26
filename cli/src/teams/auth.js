/** Verify inbound Microsoft Teams activities from the Bot Connector service. */

import { createLocalJWKSet, jwtVerify } from 'jose';

const METADATA_URL = 'https://login.botframework.com/v1/.well-known/openidconfiguration';
const ISSUER = 'https://api.botframework.com';
const KEY_HOST = 'login.botframework.com';
const CACHE_MS = 10 * 60 * 1000;
const UNKNOWN_KEY_REFRESH_MS = 30 * 1000;

async function fetchJson(fetchImpl, url) {
  const response = await fetchImpl(url, { signal: AbortSignal.timeout(5000) });
  if (!response.ok) throw new Error(`Bot Connector metadata request failed (${response.status})`);
  return response.json();
}

/**
 * Create a verifier for one gateway instance. `fetchImpl` is injectable for
 * offline tests; the production gateway always uses the global HTTPS fetch.
 */
export function createTeamsActivityVerifier({ fetchImpl = globalThis.fetch } = {}) {
  let cached = null;
  let loading = null;

  async function loadKeys(force = false) {
    if (!force && cached && Date.now() - cached.loadedAt < CACHE_MS) return cached;
    if (!loading) {
      loading = (async () => {
        const metadata = await fetchJson(fetchImpl, METADATA_URL);
        if (
          metadata?.issuer !== ISSUER ||
          !Array.isArray(metadata.id_token_signing_alg_values_supported) ||
          !metadata.id_token_signing_alg_values_supported.includes('RS256')
        ) {
          throw new Error('Bot Connector metadata has an unexpected issuer or signing algorithm');
        }

        let keysUrl;
        try {
          keysUrl = new URL(metadata.jwks_uri);
        } catch {
          throw new Error('Bot Connector metadata has an invalid signing-key URL');
        }
        if (
          keysUrl.protocol !== 'https:' ||
          keysUrl.hostname !== KEY_HOST ||
          keysUrl.username ||
          keysUrl.password ||
          keysUrl.port ||
          keysUrl.hash
        ) {
          throw new Error('Bot Connector metadata has an untrusted signing-key URL');
        }

        const jwks = await fetchJson(fetchImpl, keysUrl.href);
        if (!Array.isArray(jwks?.keys)) throw new Error('Bot Connector signing keys are invalid');
        // Teams is a published channel. Only keys endorsed for msteams may
        // authenticate its activities, even when other keys are valid for JWTs.
        const keys = jwks.keys.filter(
          (key) =>
            key?.kty === 'RSA' &&
            typeof key.kid === 'string' &&
            Array.isArray(key.endorsements) &&
            key.endorsements.includes('msteams'),
        );
        if (keys.length === 0) throw new Error('Bot Connector has no Teams-endorsed signing keys');
        const fresh = { verifier: createLocalJWKSet({ keys }), loadedAt: Date.now() };
        cached = fresh;
        return fresh;
      })().finally(() => {
        loading = null;
      });
    }
    return loading;
  }

  return async function verifyTeamsActivity(authorization, activity, appId) {
    const bearer = typeof authorization === 'string' && /^Bearer ([^\s]+)$/i.exec(authorization);
    if (
      !bearer ||
      !appId ||
      !activity ||
      activity.channelId !== 'msteams' ||
      typeof activity.serviceUrl !== 'string' ||
      !activity.serviceUrl
    ) {
      return false;
    }

    const verifyWith = async (keys) => {
      const { payload } = await jwtVerify(bearer[1], keys.verifier, {
        issuer: ISSUER,
        audience: appId,
        algorithms: ['RS256'],
        clockTolerance: 300,
      });
      return (
        Number.isInteger(payload.exp) &&
        Number.isInteger(payload.nbf) &&
        payload.serviceUrl === activity.serviceUrl
      );
    };

    let keys = await loadKeys();
    try {
      return await verifyWith(keys);
    } catch (error) {
      // A rotated key may appear between refreshes. Limit attacker-triggered
      // refreshes to one per 30 seconds; all other verification failures fail
      // closed without making a network request.
      if (
        error?.code !== 'ERR_JWKS_NO_MATCHING_KEY' ||
        Date.now() - keys.loadedAt < UNKNOWN_KEY_REFRESH_MS
      ) {
        return false;
      }
    }
    keys = await loadKeys(true);
    try {
      return await verifyWith(keys);
    } catch {
      return false;
    }
  };
}
