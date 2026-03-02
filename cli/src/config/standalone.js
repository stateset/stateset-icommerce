/**
 * Standalone Configuration for StateSet iCommerce
 *
 * Local-only config layer that works without the Sequencer or SET Chain.
 * Stored in .stateset/config.json (NOT sync.json).
 *
 * This is the default configuration for Tier 1 (standalone) deployments.
 */

import fs from 'fs';
import path from 'path';

/**
 * @typedef {Object} WebhookConfig
 * @property {number} port - HTTP port for webhook server
 * @property {string[]} sources - Active webhook sources (e.g., 'stripe', 'shopify')
 */

/**
 * @typedef {Object} AdapterConfig
 * @property {string[]} active - Active adapter names
 */

/**
 * @typedef {Object} PolicyConfig
 * @property {string} dir - Directory containing policy YAML/JSON files
 * @property {boolean} autoLoad - Auto-load policies on startup
 * @property {string} unknownDomainMode - Behavior for unknown domains: 'allow' or 'deny'
 */

/**
 * @typedef {Object} StandaloneConfig
 * @property {string} dbPath - SQLite database path
 * @property {WebhookConfig} webhooks - Webhook server config
 * @property {AdapterConfig} adapters - Platform adapter config
 * @property {PolicyConfig} policies - Policy engine config
 * @property {{ enabled: boolean }} sync - Sync config (always disabled in standalone)
 */

/** @type {StandaloneConfig} */
export const DEFAULT_STANDALONE_CONFIG = {
  dbPath: './store.db',
  webhooks: {
    port: 3000,
    sources: [],
  },
  adapters: {
    active: [],
  },
  policies: {
    dir: './policies',
    autoLoad: true,
    unknownDomainMode: 'deny',
  },
  sync: {
    enabled: false,
  },
};

const CONFIG_DIR = '.stateset';
const CONFIG_FILE = 'config.json';

/**
 * Get the path to the standalone config file.
 * @param {string} cwd
 * @returns {string}
 */
function getConfigPath(cwd) {
  return path.join(cwd, CONFIG_DIR, CONFIG_FILE);
}

/**
 * Load standalone configuration from .stateset/config.json.
 * Returns defaults if no config file exists.
 * @param {string} [cwd=process.cwd()]
 * @returns {StandaloneConfig}
 */
export function loadStandaloneConfig(cwd = process.cwd()) {
  const configPath = getConfigPath(cwd);

  if (!fs.existsSync(configPath)) {
    return { ...DEFAULT_STANDALONE_CONFIG };
  }

  try {
    const content = fs.readFileSync(configPath, 'utf-8');
    const config = JSON.parse(content);

    return {
      dbPath: config.dbPath ?? DEFAULT_STANDALONE_CONFIG.dbPath,
      webhooks: { ...DEFAULT_STANDALONE_CONFIG.webhooks, ...config.webhooks },
      adapters: { ...DEFAULT_STANDALONE_CONFIG.adapters, ...config.adapters },
      policies: { ...DEFAULT_STANDALONE_CONFIG.policies, ...config.policies },
      sync: { ...DEFAULT_STANDALONE_CONFIG.sync, ...config.sync },
    };
  } catch (err) {
    console.warn(`Failed to parse ${configPath}: ${err.message}`);
    return { ...DEFAULT_STANDALONE_CONFIG };
  }
}

/**
 * Save standalone configuration to .stateset/config.json.
 * Creates .stateset/ directory if it doesn't exist.
 * @param {StandaloneConfig} config
 * @param {string} [cwd=process.cwd()]
 */
export function saveStandaloneConfig(config, cwd = process.cwd()) {
  const dir = path.join(cwd, CONFIG_DIR);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const configPath = getConfigPath(cwd);
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n', 'utf-8');
}

/**
 * Check whether the current working directory is in standalone mode.
 * Standalone = no .stateset/sync.json present OR sync.enabled is false in config.json.
 * @param {string} [cwd=process.cwd()]
 * @returns {boolean}
 */
export function isStandaloneMode(cwd = process.cwd()) {
  const syncPath = path.join(cwd, CONFIG_DIR, 'sync.json');
  if (fs.existsSync(syncPath)) {
    return false;
  }

  const config = loadStandaloneConfig(cwd);
  return !config.sync.enabled;
}
