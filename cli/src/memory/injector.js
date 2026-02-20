/**
 * Memory Injector for StateSet iCommerce
 *
 * Hooks into the `before_agent_start` sequential hook to prepend relevant
 * conversation memories into the agent's system prompt. Follows the same
 * pattern as the SkillInjector from v0.2.7.
 *
 * Priority: 20 (skills run at 10, so memory comes after skills).
 */

import { getMemoryStore } from './store.js';

// ============================================================================
// MemoryInjector
// ============================================================================

export class MemoryInjector {
  /**
   * @param {Object} [opts]
   * @param {number} [opts.maxMemories=5] - Maximum memories to inject
   * @param {number} [opts.maxBodyLength=2000] - Maximum total characters for memory context
   */
  constructor(opts = {}) {
    this._maxMemories = opts.maxMemories || 5;
    this._maxBodyLength = opts.maxBodyLength || 2000;
  }

  /**
   * Hook handler: prepend memory context to the agent prompt.
   *
   * @param {Object} data - Hook data from before_agent_start
   * @param {string} data.text - The original prompt text
   * @param {string} [data.agentName] - Which agent is being invoked
   * @param {string} [data.channel] - Channel name (e.g., 'cli', 'telegram')
   * @param {string} [data.senderId] - Sender identifier
   * @param {boolean} [data.memoryEnabled] - Whether memory is enabled for this session
   * @returns {Object} - Modified data with memory prepended to text
   */
  async injectMemoryContext(data) {
    if (!data || !data.text) return data;
    if (!data.memoryEnabled) return data;

    const channel = data.channel || 'cli';
    const senderId = data.senderId || 'local';

    try {
      const store = getMemoryStore();
      const memories = store.getRecent(channel, senderId, this._maxMemories);

      if (memories.length === 0) return data;

      const formatted = this.formatMemories(memories);
      if (!formatted) return data;

      return {
        ...data,
        text: formatted + '\n\n' + data.text,
      };
    } catch (err) {
      // Memory is optional — don't fail the request
      console.warn(`[MemoryInjector] Failed to load memories: ${err.message}`);
      return data;
    }
  }

  /**
   * Format memories into a context block for the agent prompt.
   * @param {Object[]} memories
   * @returns {string|null}
   */
  formatMemories(memories) {
    if (!memories || memories.length === 0) return null;

    const lines = ['<memory-context>', 'Previous conversation summaries:'];
    let totalLen = 0;

    for (const mem of memories) {
      const date = new Date(mem.created_at).toISOString().split('T')[0];
      const agent = mem.agent ? ` (${mem.agent})` : '';
      const entry = `- [${date}${agent}] ${mem.summary}`;

      if (totalLen + entry.length > this._maxBodyLength) break;

      lines.push(entry);
      totalLen += entry.length;

      // Include key facts if available
      if (mem.facts && mem.facts.length > 0) {
        const factsLine = `  Facts: ${mem.facts.join(', ')}`;
        if (totalLen + factsLine.length <= this._maxBodyLength) {
          lines.push(factsLine);
          totalLen += factsLine.length;
        }
      }
    }

    lines.push('</memory-context>');
    return lines.join('\n');
  }

  /**
   * Set max memories to inject.
   * @param {number} n
   */
  setMaxMemories(n) {
    this._maxMemories = n;
  }

  /**
   * Set max body length.
   * @param {number} n
   */
  setMaxBodyLength(n) {
    this._maxBodyLength = n;
  }
}

// ============================================================================
// Hook registration
// ============================================================================

let _injector = null;

/**
 * Register the memory injector hook with a HookRunner instance.
 * Called during orchestrator startup if memory is enabled.
 *
 * @param {Object} hookRunner - The HookRunner instance
 * @param {Object} [opts]
 * @returns {MemoryInjector}
 */
export function registerMemoryHooks(hookRunner, opts) {
  _injector = new MemoryInjector(opts);

  if (hookRunner) {
    hookRunner.on(
      'before_agent_start',
      async (data) => {
        return _injector.injectMemoryContext(data);
      },
      { priority: 20, pluginId: '__memory_injector__' },
    );
  }

  return _injector;
}

/**
 * Register memory hooks using dynamic import (ESM compatible).
 * @param {Object} [opts]
 * @returns {Promise<MemoryInjector>}
 */
export async function registerMemoryHooksAsync(opts) {
  _injector = new MemoryInjector(opts);

  try {
    const { getPluginRegistry } = await import('../channels/plugin-api.js');
    getPluginRegistry()
      .getHookRunner()
      .on(
        'before_agent_start',
        async (data) => {
          return _injector.injectMemoryContext(data);
        },
        { priority: 20, pluginId: '__memory_injector__' },
      );
  } catch (err) {
    console.debug('[memory-injector] Plugin system hook registration failed:', err.message || err);
  }

  return _injector;
}

/**
 * Get the current injector instance.
 * @returns {MemoryInjector|null}
 */
export function getMemoryInjector() {
  return _injector;
}
