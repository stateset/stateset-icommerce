/**
 * A2A Agent Protocol Handshake
 *
 * Enables agents to negotiate capabilities before transacting. Prevents
 * failures from protocol mismatches by exchanging and comparing capability
 * manifests before any payment or data exchange occurs.
 *
 * @example
 * ```javascript
 * const hs = createHandshakeService({
 *   agentId: 'agent-seller-01',
 *   supportedNetworks: ['set_chain', 'base', 'ethereum'],
 *   supportedAssets: ['USDC', 'USDT'],
 *   features: { escrow: true, subscriptions: true },
 *   maxTransactionAmount: 50000,
 *   preferredFinality: 'final',
 *   webhookEndpoint: 'https://seller.example/hooks',
 *   publicKey: '0xABCD...',
 * });
 *
 * // Initiate handshake with a counterparty
 * const result = hs.initiateHandshake(theirCapabilities);
 * if (result.compatible) {
 *   console.log('Best network:', result.bestNetwork);
 * }
 * ```
 */

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_PROTOCOL_VERSION = '1.0';

/**
 * Network priority order — lower index = higher priority.
 * Used to pick the "best" network when multiple overlap.
 */
const NETWORK_PRIORITY = ['set_chain', 'base', 'arbitrum', 'solana', 'ethereum'];

/**
 * Asset priority order — lower index = higher priority.
 */
const ASSET_PRIORITY = ['USDC', 'USDT', 'ssUSD', 'DAI'];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Return the intersection of two arrays, preserving the order of `a`.
 *
 * @param {string[]} a
 * @param {string[]} b
 * @returns {string[]}
 */
function intersection(a, b) {
  const setB = new Set(b);
  return a.filter((item) => setB.has(item));
}

/**
 * Pick the best item from `shared` according to `priority`.
 * Items earlier in `priority` are preferred.
 * If none are in the priority list, return the first shared item.
 *
 * @param {string[]} shared
 * @param {string[]} priority
 * @returns {string|null}
 */
function pickBest(shared, priority) {
  if (shared.length === 0) {
    return null;
  }
  for (const item of priority) {
    if (shared.includes(item)) {
      return item;
    }
  }
  return shared[0];
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Create a handshake service for this agent.
 *
 * @param {Object} agentConfig
 * @param {string} [agentConfig.agentId] - This agent's identifier
 * @param {string} [agentConfig.protocolVersion='1.0']
 * @param {string[]} [agentConfig.supportedNetworks=['set_chain']]
 * @param {string[]} [agentConfig.supportedAssets=['USDC']]
 * @param {Object} [agentConfig.features]
 * @param {boolean} [agentConfig.features.escrow=false]
 * @param {boolean} [agentConfig.features.subscriptions=false]
 * @param {boolean} [agentConfig.features.splits=false]
 * @param {boolean} [agentConfig.features.sagas=false]
 * @param {boolean} [agentConfig.features.sse=false]
 * @param {number} [agentConfig.maxTransactionAmount=10000]
 * @param {'confirmed'|'final'} [agentConfig.preferredFinality='confirmed']
 * @param {string|null} [agentConfig.webhookEndpoint=null]
 * @param {string|null} [agentConfig.publicKey=null]
 * @returns {Object} Handshake service API
 */
export function createHandshakeService(agentConfig = {}) {
  const myCapabilities = buildCapabilityManifest(agentConfig);

  // -----------------------------------------------------------------------
  // Core methods
  // -----------------------------------------------------------------------

  /**
   * Build a normalised capability manifest from raw config.
   *
   * @param {Object} config
   * @returns {Object}
   */
  function buildCapabilityManifest(config) {
    return {
      protocolVersion: config.protocolVersion || DEFAULT_PROTOCOL_VERSION,
      agentId: config.agentId || null,
      supportedNetworks: Array.isArray(config.supportedNetworks)
        ? [...config.supportedNetworks]
        : ['set_chain'],
      supportedAssets: Array.isArray(config.supportedAssets)
        ? [...config.supportedAssets]
        : ['USDC'],
      features: {
        escrow: false,
        subscriptions: false,
        splits: false,
        sagas: false,
        sse: false,
        ...(config.features || {}),
      },
      maxTransactionAmount:
        typeof config.maxTransactionAmount === 'number' ? config.maxTransactionAmount : 10000,
      preferredFinality: config.preferredFinality || 'confirmed',
      webhookEndpoint: config.webhookEndpoint || null,
      publicKey: config.publicKey || null,
    };
  }

  /**
   * Check compatibility between two capability manifests.
   *
   * @param {Object} mine - Our capabilities
   * @param {Object} theirs - Counterparty capabilities
   * @returns {Object} Compatibility result
   */
  function checkCompatibility(mine, theirs) {
    const mismatches = [];
    const warnings = [];

    // ----- Protocol version -----
    if (mine.protocolVersion !== theirs.protocolVersion) {
      warnings.push(
        `Protocol version mismatch: ours=${mine.protocolVersion}, theirs=${theirs.protocolVersion}`,
      );
    }

    // ----- Networks -----
    const sharedNetworks = intersection(mine.supportedNetworks, theirs.supportedNetworks);
    if (sharedNetworks.length === 0) {
      mismatches.push(
        `No overlapping networks: ours=[${mine.supportedNetworks.join(', ')}], theirs=[${theirs.supportedNetworks.join(', ')}]`,
      );
    }

    // ----- Assets -----
    const sharedAssets = intersection(mine.supportedAssets, theirs.supportedAssets);
    if (sharedAssets.length === 0) {
      mismatches.push(
        `No overlapping assets: ours=[${mine.supportedAssets.join(', ')}], theirs=[${theirs.supportedAssets.join(', ')}]`,
      );
    }

    // ----- Feature warnings -----
    const featureKeys = ['escrow', 'subscriptions', 'splits', 'sagas', 'sse'];
    for (const key of featureKeys) {
      const weSupport = mine.features[key] === true;
      const theySupport = theirs.features?.[key] === true;
      if (weSupport && !theySupport) {
        warnings.push(`Counterparty does not support ${key}`);
      }
    }

    // ----- Webhook -----
    if (!theirs.webhookEndpoint) {
      warnings.push('Counterparty has no webhook endpoint configured');
    }

    // ----- Transaction amount -----
    const effectiveMaxAmount = Math.min(
      mine.maxTransactionAmount ?? Infinity,
      theirs.maxTransactionAmount ?? Infinity,
    );

    // ----- Best picks -----
    const bestNetwork = pickBest(sharedNetworks, NETWORK_PRIORITY);
    const bestAsset = pickBest(sharedAssets, ASSET_PRIORITY);

    const compatible = mismatches.length === 0;

    return {
      compatible,
      sharedNetworks,
      sharedAssets,
      mismatches,
      warnings,
      bestNetwork,
      bestAsset,
      effectiveMaxAmount,
    };
  }

  /**
   * Initiate a handshake with a target agent by evaluating their capabilities.
   *
   * @param {Object} targetCapabilities - The target agent's capability manifest
   * @returns {Object} Compatibility result plus our own capabilities
   */
  function initiateHandshake(targetCapabilities) {
    const theirs = buildCapabilityManifest(targetCapabilities);
    const result = checkCompatibility(myCapabilities, theirs);

    return {
      ...result,
      ourCapabilities: { ...myCapabilities },
      theirCapabilities: theirs,
    };
  }

  /**
   * Respond to an incoming handshake request by evaluating the caller's
   * capabilities against our own.
   *
   * @param {Object} incomingCapabilities - The incoming agent's capability manifest
   * @returns {Object} Compatibility result plus our own capabilities
   */
  function respondToHandshake(incomingCapabilities) {
    const theirs = buildCapabilityManifest(incomingCapabilities);
    const result = checkCompatibility(myCapabilities, theirs);

    return {
      ...result,
      ourCapabilities: { ...myCapabilities },
      theirCapabilities: theirs,
    };
  }

  /**
   * Return this agent's capability manifest.
   *
   * @returns {Object} Capability manifest
   */
  function getMyCapabilities() {
    return { ...myCapabilities };
  }

  // -----------------------------------------------------------------------
  // Public surface
  // -----------------------------------------------------------------------

  return {
    initiateHandshake,
    respondToHandshake,
    checkCompatibility,
    getMyCapabilities,
  };
}
