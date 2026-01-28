/**
 * Plugin Configuration State Management for StateSet iCommerce
 *
 * Manages per-plugin enable/disable state with:
 * - Global allow/deny lists
 * - Per-plugin enable overrides
 * - Disable reason tracking
 * - Persistent state (JSON file)
 *
 * Inspired by moltbot's NormalizedPluginsConfig and enable resolution logic.
 */

import fs from 'fs';
import path from 'path';

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} PluginConfigEntry
 * @property {boolean} [enabled] - Override enable state
 * @property {Object} [config] - Plugin-specific configuration
 */

/**
 * @typedef {Object} PluginConfigStateOptions
 * @property {boolean} [globalEnabled=true] - Master switch for all plugins
 * @property {string[]} [allow] - Allow list (if set, only listed plugins load)
 * @property {string[]} [deny] - Deny list (excluded plugins)
 * @property {Object<string, PluginConfigEntry>} [entries] - Per-plugin config
 * @property {string} [statePath] - Path to persist state
 */

/**
 * @typedef {Object} EnableResolution
 * @property {boolean} enabled
 * @property {string} reason - Human-readable reason for the state
 */

// ============================================================================
// PluginConfigState
// ============================================================================

export class PluginConfigState {
  /**
   * @param {PluginConfigStateOptions} [opts]
   */
  constructor(opts = {}) {
    this._globalEnabled = opts.globalEnabled !== false;
    this._allow = new Set(opts.allow || []);
    this._deny = new Set(opts.deny || []);
    this._entries = new Map();
    this._statePath = opts.statePath || null;

    // Import entries
    if (opts.entries) {
      for (const [id, entry] of Object.entries(opts.entries)) {
        this._entries.set(id, { ...entry });
      }
    }

    // Load persisted state
    if (this._statePath) {
      this._loadFromDisk();
    }
  }

  // ============================================================================
  // Enable Resolution
  // ============================================================================

  /**
   * Determine if a plugin should be enabled.
   *
   * Resolution order:
   * 1. Global master switch
   * 2. Deny list (always wins)
   * 3. Per-plugin override
   * 4. Allow list (if set, plugin must be listed)
   * 5. Plugin manifest enabledByDefault
   *
   * @param {string} pluginId
   * @param {Object} [manifest] - Plugin manifest for defaults
   * @returns {EnableResolution}
   */
  resolve(pluginId, manifest) {
    // 1. Global switch
    if (!this._globalEnabled) {
      return { enabled: false, reason: 'plugins globally disabled' };
    }

    // 2. Deny list
    if (this._deny.has(pluginId)) {
      return { enabled: false, reason: 'in deny list' };
    }

    // 3. Per-plugin override
    const entry = this._entries.get(pluginId);
    if (entry && entry.enabled !== undefined) {
      return {
        enabled: entry.enabled,
        reason: entry.enabled ? 'explicitly enabled' : 'explicitly disabled',
      };
    }

    // 4. Allow list
    if (this._allow.size > 0) {
      if (this._allow.has(pluginId)) {
        return { enabled: true, reason: 'in allow list' };
      }
      return { enabled: false, reason: 'not in allow list' };
    }

    // 5. Manifest default
    if (manifest?.enabledByDefault) {
      return { enabled: true, reason: 'enabled by default (manifest)' };
    }

    // 6. Default: enabled (loaded plugins are active unless explicitly denied)
    return { enabled: true, reason: 'enabled (default)' };
  }

  /**
   * Check if a plugin is enabled.
   *
   * @param {string} pluginId
   * @param {Object} [manifest]
   * @returns {boolean}
   */
  isEnabled(pluginId, manifest) {
    return this.resolve(pluginId, manifest).enabled;
  }

  /**
   * Get the reason a plugin is disabled.
   *
   * @param {string} pluginId
   * @param {Object} [manifest]
   * @returns {string}
   */
  getDisableReason(pluginId, manifest) {
    const resolution = this.resolve(pluginId, manifest);
    return resolution.enabled ? '' : resolution.reason;
  }

  // ============================================================================
  // Mutations
  // ============================================================================

  /**
   * Enable a plugin.
   *
   * @param {string} pluginId
   */
  enable(pluginId) {
    this._ensureEntry(pluginId);
    this._entries.get(pluginId).enabled = true;
    this._deny.delete(pluginId);
    this._persist();
  }

  /**
   * Disable a plugin.
   *
   * @param {string} pluginId
   */
  disable(pluginId) {
    this._ensureEntry(pluginId);
    this._entries.get(pluginId).enabled = false;
    this._persist();
  }

  /**
   * Reset a plugin to default enable state.
   *
   * @param {string} pluginId
   */
  resetToDefault(pluginId) {
    const entry = this._entries.get(pluginId);
    if (entry) {
      delete entry.enabled;
      this._persist();
    }
  }

  /**
   * Set plugin-specific config.
   *
   * @param {string} pluginId
   * @param {Object} config
   */
  setConfig(pluginId, config) {
    this._ensureEntry(pluginId);
    this._entries.get(pluginId).config = { ...config };
    this._persist();
  }

  /**
   * Get plugin-specific config.
   *
   * @param {string} pluginId
   * @returns {Object}
   */
  getConfig(pluginId) {
    const entry = this._entries.get(pluginId);
    return entry?.config || {};
  }

  /**
   * Update allow list.
   *
   * @param {string[]} allow
   */
  setAllowList(allow) {
    this._allow = new Set(allow);
    this._persist();
  }

  /**
   * Update deny list.
   *
   * @param {string[]} deny
   */
  setDenyList(deny) {
    this._deny = new Set(deny);
    this._persist();
  }

  /**
   * Set global enabled state.
   *
   * @param {boolean} enabled
   */
  setGlobalEnabled(enabled) {
    this._globalEnabled = enabled;
    this._persist();
  }

  // ============================================================================
  // Queries
  // ============================================================================

  /**
   * Get all entries with their enable states.
   *
   * @returns {Array<{ id: string, enabled: boolean, reason: string, hasConfig: boolean }>}
   */
  listEntries() {
    const result = [];
    for (const [id, entry] of this._entries) {
      const resolution = this.resolve(id);
      result.push({
        id,
        enabled: resolution.enabled,
        reason: resolution.reason,
        hasConfig: !!entry.config && Object.keys(entry.config).length > 0,
      });
    }
    return result;
  }

  /**
   * Export full state for serialization.
   *
   * @returns {Object}
   */
  toJSON() {
    const entries = {};
    for (const [id, entry] of this._entries) {
      entries[id] = { ...entry };
    }

    return {
      globalEnabled: this._globalEnabled,
      allow: [...this._allow],
      deny: [...this._deny],
      entries,
    };
  }

  // ============================================================================
  // Persistence
  // ============================================================================

  /** @private */
  _ensureEntry(pluginId) {
    if (!this._entries.has(pluginId)) {
      this._entries.set(pluginId, {});
    }
  }

  /** @private */
  _persist() {
    if (!this._statePath) return;

    try {
      const dir = path.dirname(this._statePath);
      fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(this._statePath, JSON.stringify(this.toJSON(), null, 2));
    } catch (err) {
      console.error('[PluginConfig] Failed to persist state:', err.message);
    }
  }

  /** @private */
  _loadFromDisk() {
    if (!this._statePath) return;

    try {
      if (!fs.existsSync(this._statePath)) return;

      const raw = JSON.parse(fs.readFileSync(this._statePath, 'utf-8'));

      if (raw.globalEnabled !== undefined) this._globalEnabled = raw.globalEnabled;
      if (Array.isArray(raw.allow)) this._allow = new Set(raw.allow);
      if (Array.isArray(raw.deny)) this._deny = new Set(raw.deny);

      if (raw.entries && typeof raw.entries === 'object') {
        for (const [id, entry] of Object.entries(raw.entries)) {
          this._entries.set(id, { ...entry });
        }
      }
    } catch (err) {
      console.error('[PluginConfig] Failed to load state:', err.message);
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global PluginConfigState instance.
 *
 * @param {PluginConfigStateOptions} [opts] - Options (used only on first call)
 * @returns {PluginConfigState}
 */
export function getPluginConfigState(opts) {
  if (!_instance) {
    _instance = new PluginConfigState(opts);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetPluginConfigState() {
  _instance = null;
}
