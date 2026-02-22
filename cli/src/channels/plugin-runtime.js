/**
 * Plugin Runtime Utilities for StateSet iCommerce
 *
 * Dependency injection container that exposes utilities to plugins.
 * When a plugin initializes, it receives a runtime context with
 * access to loggers, config, commerce, sessions, rich messages,
 * capabilities, and more.
 *
 * Inspired by moltbot's 150+ utility plugin runtime.
 */

import { getMetrics } from './metrics.js';
import { getNotifier } from './notifier.js';
import { getHandoffQueue } from './handoff.js';
import { getCommandRegistry } from './command-registry.js';
import { getPluginRegistry } from './plugin-api.js';
import { getCapabilities, getAllCapabilities, hasCapability } from './capabilities.js';
import { getPluginSlots } from './plugin-slots.js';
import { getPluginConfigState } from './plugin-config.js';
import {
  createOrderSummary,
  createOrderList,
  createInventoryCard,
  createCartSummary,
  createAnalyticsSummary,
  richMessageToPlainText,
} from './rich-messages.js';

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} PluginRuntimeContext
 * @property {PluginLogger} logger - Namespaced logger
 * @property {Object} config - Plugin-specific configuration
 * @property {Object|null} commerce - Commerce instance
 * @property {Object|null} autonomousEngine - Autonomous engine
 * @property {RuntimeServices} services - Access to core services
 * @property {RichMessageBuilders} richMessages - Rich message builders
 * @property {CapabilityUtils} capabilities - Channel capability queries
 * @property {RuntimeStorage} storage - Plugin-scoped storage
 * @property {EnvironmentInfo} env - Environment information
 */

// ============================================================================
// Logger
// ============================================================================

/**
 * @typedef {Object} PluginLogger
 * @property {Function} info - Log info message
 * @property {Function} warn - Log warning
 * @property {Function} error - Log error
 * @property {Function} debug - Log debug (only in verbose mode)
 */

/**
 * Create a namespaced logger for a plugin.
 *
 * @param {string} pluginId
 * @param {boolean} [verbose=false]
 * @returns {PluginLogger}
 */
function createPluginLogger(pluginId, verbose = false) {
  const prefix = `[plugin:${pluginId}]`;

  return {
    info: (...args) => console.info(prefix, ...args),
    warn: (...args) => console.warn(prefix, ...args),
    error: (...args) => console.error(prefix, ...args),
    debug: verbose ? (...args) => console.debug(`${prefix} [debug]`, ...args) : () => {},
  };
}

// ============================================================================
// Storage
// ============================================================================

/**
 * Simple plugin-scoped key-value storage.
 * Backed by an in-memory Map with optional file persistence.
 */
class PluginStorage {
  /**
   * @param {string} pluginId
   * @param {string} [stateDir]
   */
  constructor(pluginId, stateDir) {
    this._pluginId = pluginId;
    this._stateDir = stateDir;
    this._data = new Map();
    this._loaded = false;
  }

  /**
   * Get a value.
   *
   * @param {string} key
   * @param {*} [defaultValue]
   * @returns {*}
   */
  get(key, defaultValue) {
    this._ensureLoaded();
    const value = this._data.get(key);
    return value !== undefined ? value : defaultValue;
  }

  /**
   * Set a value.
   *
   * @param {string} key
   * @param {*} value
   */
  set(key, value) {
    this._ensureLoaded();
    this._data.set(key, value);
    this._save();
  }

  /**
   * Delete a value.
   *
   * @param {string} key
   * @returns {boolean}
   */
  delete(key) {
    this._ensureLoaded();
    const deleted = this._data.delete(key);
    if (deleted) this._save();
    return deleted;
  }

  /**
   * Check if a key exists.
   *
   * @param {string} key
   * @returns {boolean}
   */
  has(key) {
    this._ensureLoaded();
    return this._data.has(key);
  }

  /**
   * Get all keys.
   *
   * @returns {string[]}
   */
  keys() {
    this._ensureLoaded();
    return [...this._data.keys()];
  }

  /**
   * Clear all data.
   */
  clear() {
    this._data.clear();
    this._save();
  }

  /** @private */
  _ensureLoaded() {
    if (this._loaded) return;
    this._loaded = true;

    if (!this._stateDir) return;

    try {
      const { readFileSync, existsSync } = require('fs');
      const { join } = require('path');
      const filePath = join(this._stateDir, `${this._pluginId}.json`);

      if (existsSync(filePath)) {
        const raw = JSON.parse(readFileSync(filePath, 'utf-8'));
        if (raw && typeof raw === 'object') {
          for (const [key, value] of Object.entries(raw)) {
            this._data.set(key, value);
          }
        }
      }
    } catch (err) {
      console.debug('[plugin-runtime] State load failed:', err.message || err);
    }
  }

  /** @private */
  _save() {
    if (!this._stateDir) return;

    try {
      const { writeFileSync, mkdirSync } = require('fs');
      const { join } = require('path');

      mkdirSync(this._stateDir, { recursive: true });

      const obj = {};
      for (const [key, value] of this._data) {
        obj[key] = value;
      }

      writeFileSync(join(this._stateDir, `${this._pluginId}.json`), JSON.stringify(obj, null, 2));
    } catch (err) {
      console.debug('[plugin-runtime] State save failed:', err.message || err);
    }
  }
}

// ============================================================================
// Runtime Context Factory
// ============================================================================

/**
 * Create a runtime context for a plugin.
 *
 * @param {Object} opts
 * @param {string} opts.pluginId
 * @param {Object} [opts.pluginConfig] - Plugin-specific config
 * @param {Object} [opts.commerce] - Commerce instance
 * @param {Object} [opts.autonomousEngine] - Autonomous engine
 * @param {string} [opts.stateDir] - Directory for plugin state storage
 * @param {boolean} [opts.verbose=false]
 * @param {string} [opts.dbPath] - Database path
 * @returns {PluginRuntimeContext}
 */
export function createPluginRuntime(opts) {
  const {
    pluginId,
    pluginConfig = {},
    commerce = null,
    autonomousEngine = null,
    stateDir,
    verbose = false,
    dbPath,
  } = opts;

  const logger = createPluginLogger(pluginId, verbose);
  const storage = new PluginStorage(pluginId, stateDir);

  return {
    logger,

    config: Object.freeze({ ...pluginConfig }),

    commerce,

    autonomousEngine,

    services: {
      /** Access the metrics collector */
      getMetrics,

      /** Access the channel notifier */
      getNotifier,

      /** Access the handoff queue */
      getHandoffQueue,

      /** Access the command registry */
      getCommandRegistry,

      /** Access the plugin registry */
      getPluginRegistry,

      /** Access the plugin slots */
      getPluginSlots,

      /** Access the plugin config state */
      getPluginConfigState,

      /** Send a notification through all configured channels */
      sendNotification: async (notification) => {
        const notifier = getNotifier();
        return notifier.sendNotification(notification);
      },
    },

    richMessages: {
      createOrderSummary,
      createOrderList,
      createInventoryCard,
      createCartSummary,
      createAnalyticsSummary,
      richMessageToPlainText,
    },

    capabilities: {
      /** Get capabilities for a channel */
      getCapabilities,

      /** Get all channel capabilities */
      getAllCapabilities,

      /** Check if a channel has a specific capability */
      hasCapability,

      /** Get channels that support a capability */
      getChannelsWithCapability: (cap) => {
        const all = getAllCapabilities();
        return Object.entries(all)
          .filter(([, caps]) => caps[cap] === true)
          .map(([channel]) => channel);
      },
    },

    storage,

    env: {
      verbose,
      dbPath: dbPath || './store.db',
      stateDir: stateDir || null,
      pluginId,
    },
  };
}

// ============================================================================
// Shared Runtime (global context)
// ============================================================================

let _sharedRuntime = null;

/**
 * Initialize the shared runtime context.
 * Called once during orchestrator startup.
 *
 * @param {Object} opts
 * @param {Object} [opts.commerce]
 * @param {Object} [opts.autonomousEngine]
 * @param {string} [opts.stateDir]
 * @param {boolean} [opts.verbose]
 * @param {string} [opts.dbPath]
 */
export function initializeSharedRuntime(opts = {}) {
  _sharedRuntime = {
    commerce: opts.commerce || null,
    autonomousEngine: opts.autonomousEngine || null,
    stateDir: opts.stateDir || null,
    verbose: opts.verbose || false,
    dbPath: opts.dbPath || './store.db',
    vectorAutoIndex: opts.vectorAutoIndex || null,
  };
}

/**
 * Create a plugin runtime using the shared context.
 *
 * @param {string} pluginId
 * @param {Object} [pluginConfig]
 * @returns {PluginRuntimeContext}
 */
export function createPluginRuntimeFromShared(pluginId, pluginConfig = {}) {
  if (!_sharedRuntime) {
    throw new Error('Shared runtime not initialized. Call initializeSharedRuntime() first.');
  }

  const stateDir = _sharedRuntime.stateDir
    ? require('path').join(_sharedRuntime.stateDir, 'plugins')
    : null;

  return createPluginRuntime({
    pluginId,
    pluginConfig,
    commerce: _sharedRuntime.commerce,
    autonomousEngine: _sharedRuntime.autonomousEngine,
    stateDir,
    verbose: _sharedRuntime.verbose,
    dbPath: _sharedRuntime.dbPath,
  });
}

/**
 * Get the shared runtime configuration (or null if not initialized).
 *
 * @returns {Object|null}
 */
export function getSharedRuntime() {
  return _sharedRuntime;
}

/**
 * Reset shared runtime (for testing).
 */
export function resetSharedRuntime() {
  _sharedRuntime = null;
}
