/**
 * Sync Configuration Management
 *
 * Manages the .stateset/sync.json configuration file for VES sync.
 */

import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import {
  SECURITY_PROFILE_HYBRID,
  SECURITY_PROFILE_LEGACY,
  assertSecureTransportForProfile,
  isSecureSequencerProtocol,
  resolveSecurityProfile,
} from './pqc.js';
import { hasNativeHybridPqcSupport } from './crypto.js';
import { auditProfileChanged, auditProfileDowngradeBlocked } from './pqc-audit.js';

/**
 * @typedef {Object} SyncConfig
 * @property {SequencerConfig} sequencer - Sequencer connection settings
 * @property {IdentityConfig} identity - Agent identity
 * @property {AuthConfig} auth - Authentication credentials
 * @property {SyncSettings} sync - Sync behavior settings
 * @property {LocalConfig} local - Local database settings
 * @property {KeysConfig} keys - Agent key configuration (VES v1.0)
 */

/**
 * @typedef {Object} SequencerConfig
 * @property {string} url - Sequencer URL (grpc:// or https://)
 * @property {boolean} tls - Enable TLS
 * @property {string} [certPath] - Custom CA certificate path
 * @property {boolean} [insecure] - Allow insecure transport explicitly (legacy dev only)
 */

/**
 * @typedef {Object} IdentityConfig
 * @property {string} tenantId - Tenant UUID
 * @property {string} storeId - Store UUID
 * @property {string} agentId - Agent UUID (auto-generated if not set)
 */

/**
 * @typedef {Object} AuthConfig
 * @property {string} [apiKey] - API key authentication
 * @property {string} [jwt] - JWT token authentication
 */

/**
 * @typedef {Object} SyncSettings
 * @property {boolean} autoSync - Enable automatic background sync
 * @property {number} syncIntervalMs - Sync interval in milliseconds
 * @property {number} batchSize - Max events per push batch
 * @property {'legacy' | 'hybrid' | 'pqc-strict'} securityProfile - PQ migration profile
 * @property {RetryPolicy} retryPolicy - Retry configuration
 */

/**
 * @typedef {Object} RetryPolicy
 * @property {number} maxRetries - Maximum retry attempts
 * @property {number} baseDelay - Base delay in ms
 * @property {number} maxDelay - Maximum delay in ms
 */

/**
 * @typedef {Object} LocalConfig
 * @property {string} dbPath - Path to SQLite database
 * @property {number} outboxRetentionDays - Days to retain synced events
 */

/**
 * @typedef {Object} KeysConfig
 * @property {string} keysDir - Directory for key storage (relative to .stateset)
 * @property {boolean} autoGenerate - Auto-generate keys if none exist
 * @property {boolean} encryptPayloads - Enable payload encryption by default
 */

const DEFAULT_CONFIG = {
  sequencer: {
    url: 'grpcs://localhost:50051',
    tls: true,
    insecure: false,
  },
  identity: {
    tenantId: null,
    storeId: null,
    agentId: null,
  },
  auth: {
    apiKey: null,
    jwt: null,
  },
  sync: {
    autoSync: false,
    syncIntervalMs: 30000,
    batchSize: 100,
    securityProfile: SECURITY_PROFILE_HYBRID,
    retryPolicy: {
      maxRetries: 3,
      baseDelay: 1000,
      maxDelay: 30000,
    },
  },
  local: {
    dbPath: './store.db',
    outboxRetentionDays: 30,
  },
  keys: {
    keysDir: 'keys',
    autoGenerate: true,
    encryptPayloads: false,
  },
};

const ALLOWED_SEQUENCER_PROTOCOLS = new Set(['grpc:', 'grpcs:', 'http:', 'https:']);

function parseSequencerUrl(url) {
  if (typeof url !== 'string' || !url.trim()) {
    throw new Error('Sequencer URL must be a non-empty string');
  }

  const parsed = new URL(url.trim());
  if (!ALLOWED_SEQUENCER_PROTOCOLS.has(parsed.protocol)) {
    throw new Error(`Unsupported sequencer protocol: ${parsed.protocol}`);
  }
  if (!parsed.hostname) {
    throw new Error('Sequencer URL must include a host');
  }
  return parsed;
}

/**
 * Get the config directory path
 * @param {string} [cwd] - Current working directory
 * @returns {string}
 */
export function getConfigDir(cwd = process.cwd()) {
  return path.join(cwd, '.stateset');
}

/**
 * Get the config file path
 * @param {string} [cwd] - Current working directory
 * @returns {string}
 */
export function getConfigPath(cwd = process.cwd()) {
  return path.join(getConfigDir(cwd), 'sync.json');
}

/**
 * Get the keys directory path
 * @param {string} [cwd] - Current working directory
 * @param {string} [keysDir] - Keys subdirectory name
 * @returns {string}
 */
export function getKeysDir(cwd = process.cwd(), keysDir = 'keys') {
  return path.join(getConfigDir(cwd), keysDir);
}

/**
 * Check if sync is configured
 * @param {string} [cwd] - Current working directory
 * @returns {boolean}
 */
export function isSyncConfigured(cwd = process.cwd()) {
  return fs.existsSync(getConfigPath(cwd));
}

/**
 * Load sync configuration
 * @param {string} [cwd] - Current working directory
 * @returns {SyncConfig|null} Configuration or null if not configured
 */
export function loadSyncConfig(cwd = process.cwd()) {
  const configPath = getConfigPath(cwd);

  if (!fs.existsSync(configPath)) {
    return null;
  }

  try {
    const content = fs.readFileSync(configPath, 'utf-8');
    const config = JSON.parse(content);

    // Merge with defaults
    return {
      sequencer: { ...DEFAULT_CONFIG.sequencer, ...config.sequencer },
      identity: { ...DEFAULT_CONFIG.identity, ...config.identity },
      auth: { ...DEFAULT_CONFIG.auth, ...config.auth },
      sync: {
        ...DEFAULT_CONFIG.sync,
        ...config.sync,
        retryPolicy: {
          ...DEFAULT_CONFIG.sync.retryPolicy,
          ...(config.sync?.retryPolicy || {}),
        },
      },
      local: { ...DEFAULT_CONFIG.local, ...config.local },
      keys: { ...DEFAULT_CONFIG.keys, ...config.keys },
    };
  } catch (error) {
    throw new Error(`Failed to load sync config: ${error.message}`);
  }
}

/**
 * Save sync configuration
 * @param {SyncConfig} config - Configuration to save
 * @param {string} [cwd] - Current working directory
 */
export function saveSyncConfig(config, cwd = process.cwd()) {
  const configDir = getConfigDir(cwd);
  const configPath = getConfigPath(cwd);

  // Ensure .stateset directory exists
  if (!fs.existsSync(configDir)) {
    fs.mkdirSync(configDir, { recursive: true });
  }

  // Add .stateset to .gitignore if not already there
  const gitignorePath = path.join(cwd, '.gitignore');
  if (fs.existsSync(gitignorePath)) {
    const gitignore = fs.readFileSync(gitignorePath, 'utf-8');
    if (!gitignore.includes('.stateset')) {
      fs.appendFileSync(gitignorePath, '\n# StateSet sync config\n.stateset/\n');
    }
  }

  // Write config
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
}

/**
 * Create initial sync configuration
 * @param {Object} options - Configuration options
 * @param {string} options.sequencerUrl - Sequencer URL
 * @param {string} options.tenantId - Tenant UUID
 * @param {string} options.storeId - Store UUID
 * @param {string} [options.apiKey] - API key
 * @param {string} [options.dbPath] - Database path
 * @param {boolean} [options.autoGenerateKeys=true] - Auto-generate keys
 * @param {boolean} [options.encryptPayloads=false] - Enable payload encryption
 * @param {'legacy' | 'hybrid' | 'pqc-strict'} [options.securityProfile='hybrid'] - PQ migration profile
 * @param {boolean} [options.allowInsecureTransport=false] - Allow insecure legacy transport explicitly
 * @param {string} [cwd] - Current working directory
 * @returns {SyncConfig}
 */
export function createSyncConfig(options, cwd = process.cwd()) {
  const sequencerUrl =
    typeof options.sequencerUrl === 'string' ? options.sequencerUrl.trim() : options.sequencerUrl;
  const url = parseSequencerUrl(sequencerUrl);
  const isSecure = url.protocol === 'grpcs:' || url.protocol === 'https:';
  const securityProfile = resolveSecurityProfile(
    options.securityProfile ?? DEFAULT_CONFIG.sync.securityProfile,
  );
  assertSecureTransportForProfile(
    securityProfile,
    isSecure,
    'Sequencer URL',
    options.allowInsecureTransport === true || options.insecure === true,
  );

  if (securityProfile !== SECURITY_PROFILE_LEGACY && !hasNativeHybridPqcSupport()) {
    throw new Error(
      `Security profile "${securityProfile}" requires native @stateset/embedded module with PQC support. ` +
      'Install @stateset/embedded or use the "legacy" profile.',
    );
  }

  const config = {
    sequencer: {
      url: sequencerUrl,
      tls: isSecure,
      insecure: !isSecure,
    },
    identity: {
      tenantId: options.tenantId,
      storeId: options.storeId,
      agentId: crypto.randomUUID(),
    },
    auth: {
      apiKey: options.apiKey || null,
      jwt: null,
    },
    sync: {
      autoSync: false,
      syncIntervalMs: 30000,
      batchSize: 100,
      securityProfile,
      retryPolicy: {
        maxRetries: 3,
        baseDelay: 1000,
        maxDelay: 30000,
      },
    },
    local: {
      dbPath: options.dbPath || './store.db',
      outboxRetentionDays: 30,
    },
    keys: {
      keysDir: 'keys',
      autoGenerate: options.autoGenerateKeys !== false,
      encryptPayloads: options.encryptPayloads || false,
    },
  };

  saveSyncConfig(config, cwd);
  return config;
}

/**
 * Update sync configuration
 * @param {Partial<SyncConfig>} updates - Configuration updates
 * @param {string} [cwd] - Current working directory
 * @returns {SyncConfig} Updated configuration
 */
export function updateSyncConfig(updates, cwd = process.cwd()) {
  if (updates?.sequencer?.url) {
    parseSequencerUrl(updates.sequencer.url);
  }

  const current = loadSyncConfig(cwd) || DEFAULT_CONFIG;

  const updated = {
    sequencer: { ...current.sequencer, ...updates.sequencer },
    identity: { ...current.identity, ...updates.identity },
    auth: { ...current.auth, ...updates.auth },
    sync: {
      ...current.sync,
      ...updates.sync,
      retryPolicy: {
        ...current.sync.retryPolicy,
        ...(updates.sync?.retryPolicy || {}),
      },
    },
    local: { ...current.local, ...updates.local },
    keys: { ...current.keys, ...updates.keys },
  };

  const securityProfile = resolveSecurityProfile(
    updated.sync?.securityProfile ?? DEFAULT_CONFIG.sync.securityProfile,
  );

  // Prevent profile downgrades (pqc-strict→hybrid→legacy) unless force flag is set
  const currentProfile = resolveSecurityProfile(
    current.sync?.securityProfile ?? DEFAULT_CONFIG.sync.securityProfile,
  );
  const PROFILE_STRENGTH = { legacy: 0, hybrid: 1, 'pqc-strict': 2 };
  if (
    (PROFILE_STRENGTH[securityProfile] ?? 0) < (PROFILE_STRENGTH[currentProfile] ?? 0) &&
    !updates._forceProfileDowngrade
  ) {
    auditProfileDowngradeBlocked(currentProfile, securityProfile);
    throw new Error(
      `Security profile downgrade from "${currentProfile}" to "${securityProfile}" is not allowed. ` +
      'Downgrading removes post-quantum protection from future events. ' +
      'Pass _forceProfileDowngrade: true to override.',
    );
  }

  if (securityProfile !== currentProfile) {
    auditProfileChanged(currentProfile, securityProfile, !!updates._forceProfileDowngrade);
  }

  updated.sync.securityProfile = securityProfile;

  if (updated.sequencer?.url) {
    try {
      const url = parseSequencerUrl(updated.sequencer.url);
      assertSecureTransportForProfile(
        securityProfile,
        isSecureSequencerProtocol(url.protocol),
        'Sequencer URL',
        updated.sequencer?.insecure === true,
      );
    } catch (error) {
      throw new Error(`Invalid sync configuration update: ${error.message}`);
    }
  }

  saveSyncConfig(updated, cwd);
  return updated;
}

/**
 * Get API key from config or environment
 * @param {SyncConfig} config
 * @returns {string|null}
 */
export function getApiKey(config) {
  return process.env.STATESET_API_KEY || config.auth?.apiKey || null;
}

/**
 * Get JWT token from config or environment
 * @param {SyncConfig} config
 * @returns {string|null}
 */
export function getJwtToken(config) {
  return process.env.STATESET_JWT || config.auth?.jwt || null;
}

/**
 * Validate sync configuration
 * @param {SyncConfig} config
 * @returns {{valid: boolean, errors: string[]}}
 */
export function validateSyncConfig(config) {
  const errors = [];
  let parsedUrl = null;
  let securityProfile = SECURITY_PROFILE_HYBRID;

  if (!config.sequencer?.url) {
    errors.push('Sequencer URL is required');
  } else {
    try {
      parsedUrl = parseSequencerUrl(config.sequencer.url);
    } catch (error) {
      errors.push(`Invalid sequencer URL: ${error.message}`);
    }
  }

  try {
    securityProfile = resolveSecurityProfile(
      config.sync?.securityProfile ?? DEFAULT_CONFIG.sync.securityProfile,
    );
  } catch (error) {
    errors.push(error.message);
  }

  if (parsedUrl) {
    try {
      assertSecureTransportForProfile(
        securityProfile,
        isSecureSequencerProtocol(parsedUrl.protocol),
        'Sequencer URL',
        config.sequencer?.insecure === true,
      );
    } catch (error) {
      errors.push(error.message);
    }
  }

  if (!config.identity?.tenantId) {
    errors.push('Tenant ID is required');
  }

  if (!config.identity?.storeId) {
    errors.push('Store ID is required');
  }

  // Validate UUIDs
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

  if (config.identity?.tenantId && !uuidRegex.test(config.identity.tenantId)) {
    errors.push('Tenant ID must be a valid UUID');
  }

  if (config.identity?.storeId && !uuidRegex.test(config.identity.storeId)) {
    errors.push('Store ID must be a valid UUID');
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}

/**
 * SyncConfig class for programmatic access (VES v1.0)
 */
export class SyncConfig {
  constructor(config) {
    this.sequencer = config.sequencer;
    this.identity = config.identity;
    this.auth = config.auth;
    this.sync = {
      ...DEFAULT_CONFIG.sync,
      ...(config.sync || {}),
      securityProfile: resolveSecurityProfile(
        config.sync?.securityProfile ?? DEFAULT_CONFIG.sync.securityProfile,
      ),
      retryPolicy: {
        ...DEFAULT_CONFIG.sync.retryPolicy,
        ...(config.sync?.retryPolicy || {}),
      },
    };
    this.local = config.local;
    this.keys = config.keys || DEFAULT_CONFIG.keys;
  }

  /**
   * Get sequencer URL
   * @returns {string}
   */
  get sequencerUrl() {
    return this.sequencer.url;
  }

  /**
   * Get tenant ID
   * @returns {string}
   */
  get tenantId() {
    return this.identity.tenantId;
  }

  /**
   * Get store ID
   * @returns {string}
   */
  get storeId() {
    return this.identity.storeId;
  }

  /**
   * Get agent ID
   * @returns {string}
   */
  get agentId() {
    return this.identity.agentId;
  }

  /**
   * Get authentication credentials
   * @returns {{apiKey: string|null, jwt: string|null}}
   */
  getCredentials() {
    return {
      apiKey: getApiKey(this),
      jwt: getJwtToken(this),
    };
  }

  /**
   * Check if TLS is enabled
   * @returns {boolean}
   */
  get tlsEnabled() {
    return this.sequencer.tls;
  }

  /**
   * Get batch size
   * @returns {number}
   */
  get batchSize() {
    return this.sync.batchSize;
  }

  /**
   * Get retry policy
   * @returns {RetryPolicy}
   */
  get retryPolicy() {
    return this.sync.retryPolicy;
  }

  /**
   * Get PQ migration profile
   * @returns {'legacy' | 'hybrid' | 'pqc-strict'}
   */
  get securityProfile() {
    return this.sync.securityProfile;
  }

  /**
   * Get keys directory (relative to .stateset)
   * @returns {string}
   */
  get keysDir() {
    return this.keys.keysDir;
  }

  /**
   * Whether to auto-generate keys if none exist
   * @returns {boolean}
   */
  get autoGenerateKeys() {
    return this.keys.autoGenerate;
  }

  /**
   * Whether to encrypt payloads by default
   * @returns {boolean}
   */
  get encryptPayloads() {
    return this.keys.encryptPayloads;
  }

  /**
   * Validate the configuration
   * @returns {{valid: boolean, errors: string[]}}
   */
  validate() {
    return validateSyncConfig(this);
  }

  /**
   * Load from file
   * @param {string} [cwd]
   * @returns {SyncConfig|null}
   */
  static load(cwd = process.cwd()) {
    const config = loadSyncConfig(cwd);
    return config ? new SyncConfig(config) : null;
  }

  /**
   * Save to file
   * @param {string} [cwd]
   */
  save(cwd = process.cwd()) {
    saveSyncConfig(this, cwd);
  }
}
