/**
 * Plugin Slot System for StateSet iCommerce
 *
 * Manages exclusive plugin slots where only one plugin of a given
 * "kind" can be active at a time. For example, only one "memory"
 * plugin or one "search" plugin can be active.
 *
 * Inspired by moltbot's memory slot system.
 *
 * Usage:
 *   const slots = getPluginSlots();
 *   slots.defineSlot('memory', { required: false });
 *   slots.assign('memory', 'redis-memory');
 *   slots.getAssigned('memory'); // => 'redis-memory'
 */

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} SlotDefinition
 * @property {string} name - Slot name (e.g., 'memory', 'search', 'analytics')
 * @property {string} [description] - Human-readable description
 * @property {boolean} [required=false] - Whether a plugin must fill this slot
 * @property {string} [defaultPlugin] - Default plugin ID for this slot
 */

/**
 * @typedef {Object} SlotState
 * @property {string} name - Slot name
 * @property {string|null} assigned - Currently assigned plugin ID
 * @property {string[]} candidates - Plugin IDs that can fill this slot
 * @property {boolean} required
 * @property {string} [defaultPlugin]
 */

// ============================================================================
// PluginSlots
// ============================================================================

export class PluginSlots {
  constructor() {
    /** @type {Map<string, SlotDefinition>} */
    this._definitions = new Map();

    /** @type {Map<string, string|null>} - slot name -> assigned plugin ID */
    this._assignments = new Map();

    /** @type {Map<string, Set<string>>} - slot name -> candidate plugin IDs */
    this._candidates = new Map();

    /** @type {Map<string, string>} - plugin ID -> slot name (reverse lookup) */
    this._pluginSlots = new Map();
  }

  // ============================================================================
  // Slot Definitions
  // ============================================================================

  /**
   * Define a new plugin slot.
   *
   * @param {string} name - Slot name
   * @param {Object} [opts]
   * @param {string} [opts.description]
   * @param {boolean} [opts.required=false]
   * @param {string} [opts.defaultPlugin]
   */
  defineSlot(name, opts = {}) {
    if (this._definitions.has(name)) {
      throw new Error(`Slot "${name}" is already defined`);
    }

    this._definitions.set(name, {
      name,
      description: opts.description || '',
      required: opts.required === true,
      defaultPlugin: opts.defaultPlugin || null,
    });

    this._assignments.set(name, null);
    this._candidates.set(name, new Set());
  }

  /**
   * Check if a slot is defined.
   *
   * @param {string} name
   * @returns {boolean}
   */
  hasSlot(name) {
    return this._definitions.has(name);
  }

  // ============================================================================
  // Candidate Registration
  // ============================================================================

  /**
   * Register a plugin as a candidate for a slot.
   *
   * @param {string} slotName
   * @param {string} pluginId
   */
  registerCandidate(slotName, pluginId) {
    if (!this._definitions.has(slotName)) {
      throw new Error(`Slot "${slotName}" is not defined`);
    }

    this._candidates.get(slotName).add(pluginId);
  }

  /**
   * Unregister a plugin as a candidate.
   *
   * @param {string} slotName
   * @param {string} pluginId
   */
  unregisterCandidate(slotName, pluginId) {
    const candidates = this._candidates.get(slotName);
    if (candidates) {
      candidates.delete(pluginId);
    }

    // Clear assignment if this was the assigned plugin
    if (this._assignments.get(slotName) === pluginId) {
      this._assignments.set(slotName, null);
      this._pluginSlots.delete(pluginId);
    }
  }

  /**
   * Unregister a plugin from all slots.
   *
   * @param {string} pluginId
   */
  unregisterPlugin(pluginId) {
    for (const [slotName, candidates] of this._candidates) {
      candidates.delete(pluginId);

      if (this._assignments.get(slotName) === pluginId) {
        this._assignments.set(slotName, null);
      }
    }

    this._pluginSlots.delete(pluginId);
  }

  // ============================================================================
  // Assignment
  // ============================================================================

  /**
   * Assign a plugin to a slot.
   * Only one plugin can hold a slot at a time.
   *
   * @param {string} slotName
   * @param {string} pluginId
   * @throws {Error} If plugin is not a registered candidate
   */
  assign(slotName, pluginId) {
    if (!this._definitions.has(slotName)) {
      throw new Error(`Slot "${slotName}" is not defined`);
    }

    const candidates = this._candidates.get(slotName);
    if (!candidates.has(pluginId)) {
      throw new Error(`Plugin "${pluginId}" is not a registered candidate for slot "${slotName}"`);
    }

    // Clear previous assignment
    const previousPlugin = this._assignments.get(slotName);
    if (previousPlugin) {
      this._pluginSlots.delete(previousPlugin);
    }

    // Check if plugin already holds a different slot
    const existingSlot = this._pluginSlots.get(pluginId);
    if (existingSlot && existingSlot !== slotName) {
      this._assignments.set(existingSlot, null);
    }

    this._assignments.set(slotName, pluginId);
    this._pluginSlots.set(pluginId, slotName);
  }

  /**
   * Clear a slot assignment.
   *
   * @param {string} slotName
   */
  clearSlot(slotName) {
    const pluginId = this._assignments.get(slotName);
    if (pluginId) {
      this._pluginSlots.delete(pluginId);
    }
    this._assignments.set(slotName, null);
  }

  /**
   * Assign "none" to a slot (explicitly no plugin).
   *
   * @param {string} slotName
   */
  assignNone(slotName) {
    this.clearSlot(slotName);
  }

  // ============================================================================
  // Queries
  // ============================================================================

  /**
   * Get the currently assigned plugin for a slot.
   *
   * @param {string} slotName
   * @returns {string|null}
   */
  getAssigned(slotName) {
    return this._assignments.get(slotName) || null;
  }

  /**
   * Get all candidates for a slot.
   *
   * @param {string} slotName
   * @returns {string[]}
   */
  getCandidates(slotName) {
    const candidates = this._candidates.get(slotName);
    return candidates ? [...candidates] : [];
  }

  /**
   * Get the slot a plugin is assigned to.
   *
   * @param {string} pluginId
   * @returns {string|null}
   */
  getPluginSlot(pluginId) {
    return this._pluginSlots.get(pluginId) || null;
  }

  /**
   * Check if a plugin should be enabled based on slot assignment.
   *
   * A plugin that is a candidate for a slot but NOT the assigned one
   * should be disabled.
   *
   * @param {string} pluginId
   * @returns {{ shouldDisable: boolean, reason: string }}
   */
  checkSlotDisable(pluginId) {
    for (const [slotName, candidates] of this._candidates) {
      if (candidates.has(pluginId)) {
        const assigned = this._assignments.get(slotName);

        if (assigned === null) {
          // Slot is explicitly set to "none"
          return { shouldDisable: true, reason: `slot "${slotName}" set to none` };
        }

        if (assigned !== pluginId) {
          return { shouldDisable: true, reason: `slot "${slotName}" assigned to "${assigned}"` };
        }

        // This plugin IS the assigned one
        return { shouldDisable: false, reason: '' };
      }
    }

    // Plugin is not a slot candidate
    return { shouldDisable: false, reason: '' };
  }

  /**
   * Auto-assign slots based on defaults and candidates.
   * Call this after all plugins have registered as candidates.
   */
  autoAssign() {
    for (const [slotName, def] of this._definitions) {
      const current = this._assignments.get(slotName);
      if (current) continue; // Already assigned

      const candidates = this._candidates.get(slotName);
      if (!candidates || candidates.size === 0) continue;

      // Try default
      if (def.defaultPlugin && candidates.has(def.defaultPlugin)) {
        this.assign(slotName, def.defaultPlugin);
        continue;
      }

      // Assign first candidate
      const first = [...candidates][0];
      this.assign(slotName, first);
    }
  }

  /**
   * Get full state of all slots.
   *
   * @returns {SlotState[]}
   */
  getSlotStates() {
    const states = [];

    for (const [name, def] of this._definitions) {
      states.push({
        name,
        assigned: this._assignments.get(name) || null,
        candidates: this.getCandidates(name),
        required: def.required,
        defaultPlugin: def.defaultPlugin,
      });
    }

    return states;
  }

  /**
   * Check for missing required slots.
   *
   * @returns {string[]} - Names of unfilled required slots
   */
  getMissingRequired() {
    const missing = [];
    for (const [name, def] of this._definitions) {
      if (def.required && !this._assignments.get(name)) {
        missing.push(name);
      }
    }
    return missing;
  }

  /**
   * Clear all definitions, assignments, and candidates.
   */
  clear() {
    this._definitions.clear();
    this._assignments.clear();
    this._candidates.clear();
    this._pluginSlots.clear();
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global PluginSlots instance.
 *
 * @returns {PluginSlots}
 */
export function getPluginSlots() {
  if (!_instance) {
    _instance = new PluginSlots();
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetPluginSlots() {
  if (_instance) {
    _instance.clear();
  }
  _instance = null;
}
