/**
 * Tier Detection for StateSet iCommerce
 *
 * Detects which product tier the user is running based on available configuration:
 *
 * - Tier 1 (Standalone): Local SQLite commerce engine, adapters, policies, webhooks.
 * - Tier 2 (Sequencer):  Everything in Tier 1 + VES sync, multi-agent coordination, audit trail.
 * - Tier 3 (Full):       Everything in Tier 2 + on-chain settlement, ssUSD, x402, SetPaymaster.
 */

import fs from 'fs';
import path from 'path';

export const TIERS = Object.freeze({
  STANDALONE: 'standalone',
  SEQUENCER: 'sequencer',
  FULL: 'full',
});

const CONFIG_DIR = '.stateset';

/**
 * Detect which tier the user is running.
 *
 * Detection logic:
 * 1. If .stateset/sync.json exists AND contains a chain RPC URL → FULL
 * 2. If .stateset/sync.json exists → SEQUENCER
 * 3. Otherwise → STANDALONE
 *
 * @param {string} [cwd=process.cwd()]
 * @returns {string} One of TIERS.STANDALONE, TIERS.SEQUENCER, TIERS.FULL
 */
export function detectTier(cwd = process.cwd()) {
  const syncPath = path.join(cwd, CONFIG_DIR, 'sync.json');

  if (!fs.existsSync(syncPath)) {
    return TIERS.STANDALONE;
  }

  try {
    const content = fs.readFileSync(syncPath, 'utf-8');
    const syncConfig = JSON.parse(content);

    // Check for chain/settlement configuration (Tier 3 indicators)
    const hasChainRpc =
      syncConfig.chain?.rpcUrl || syncConfig.settlement?.rpcUrl || syncConfig.anchor?.l2RpcUrl;

    if (hasChainRpc) {
      return TIERS.FULL;
    }

    return TIERS.SEQUENCER;
  } catch {
    // sync.json exists but can't be parsed — treat as Sequencer tier
    return TIERS.SEQUENCER;
  }
}

/**
 * Get the capabilities available at a given tier.
 * @param {string} tier - One of TIERS values
 * @returns {string[]}
 */
export function getTierCapabilities(tier) {
  const base = [
    'commerce',
    'policies',
    'adapters',
    'webhooks',
    'analytics',
    'manufacturing',
    'tax',
    'promotions',
    'subscriptions',
  ];

  if (tier === TIERS.SEQUENCER) {
    return [...base, 'sync', 'crypto', 'multi-agent', 'receipts', 'audit-trail'];
  }

  if (tier === TIERS.FULL) {
    return [
      ...base,
      'sync',
      'crypto',
      'multi-agent',
      'receipts',
      'audit-trail',
      'chain',
      'x402',
      'stablecoin',
      'anchoring',
      'stark-proofs',
    ];
  }

  return base;
}

/**
 * Get a human-readable label for the tier.
 * @param {string} tier
 * @returns {string}
 */
export function getTierLabel(tier) {
  switch (tier) {
    case TIERS.STANDALONE:
      return 'iCommerce Standalone';
    case TIERS.SEQUENCER:
      return 'iCommerce + Sequencer';
    case TIERS.FULL:
      return 'Full Trilogy (iCommerce + Sequencer + SET Chain)';
    default:
      return 'Unknown';
  }
}

/**
 * Check whether a capability is available at the current tier.
 * @param {string} capability
 * @param {string} [cwd=process.cwd()]
 * @returns {boolean}
 */
export function hasCapability(capability, cwd = process.cwd()) {
  const tier = detectTier(cwd);
  return getTierCapabilities(tier).includes(capability);
}
