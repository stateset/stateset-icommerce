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
// Entity extraction
// ============================================================================

/**
 * Ordered list of entity patterns. Each entry describes one entity type,
 * the regex to match it in user text, and which capture group holds the ID.
 *
 * Patterns are intentionally case-insensitive and support the most common
 * formats used in StateSet entity IDs.
 */
const ENTITY_PATTERNS = [
  // Order IDs: ORD-XXXXX, "order #12345", UUID preceded by "order"
  {
    type: 'order',
    re: /\b(ORD-[A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'order',
    re: /\border\s+#\s*([A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'order',
    re: /\border\s+([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/gi,
    group: 1,
  },
  // Customer IDs: CUST-XXXXX, "customer #NNN", email addresses
  {
    type: 'customer',
    re: /\b(CUST-[A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'customer',
    re: /\bcustomer\s+#\s*([A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'customer',
    re: /\b([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})\b/g,
    group: 1,
  },
  // Product IDs: PROD-XXXXX, SKU-XXXXX
  {
    type: 'product',
    re: /\b(PROD-[A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'product',
    re: /\b(SKU-[A-Z0-9-]+)/gi,
    group: 1,
  },
  // Return IDs: RET-XXXXX, "return #NNN"
  {
    type: 'return',
    re: /\b(RET-[A-Z0-9-]+)/gi,
    group: 1,
  },
  {
    type: 'return',
    re: /\breturn\s+#\s*([A-Z0-9-]+)/gi,
    group: 1,
  },
];

/**
 * Extract entity references from a text string.
 *
 * @param {string} text - User message text
 * @returns {Array<{ type: 'order'|'customer'|'product'|'return', id: string }>}
 *   Deduplicated list of entity references found in the text, in order of
 *   first appearance.
 */
export function extractEntityIds(text) {
  if (!text || typeof text !== 'string') return [];

  const seen = new Set(); // "type:id" keys for deduplication
  const results = [];

  for (const { type, re, group } of ENTITY_PATTERNS) {
    // Reset lastIndex to support re-use of stateful regexes across calls
    re.lastIndex = 0;
    let match;
    while ((match = re.exec(text)) !== null) {
      const id = match[group];
      if (!id) continue;
      const key = `${type}:${id.toLowerCase()}`;
      if (!seen.has(key)) {
        seen.add(key);
        results.push({ type, id });
      }
    }
  }

  return results;
}

// ============================================================================
// MemoryInjector
// ============================================================================

export class MemoryInjector {
  /**
   * @param {Object} [opts]
   * @param {number} [opts.maxMemories=5] - Maximum memories to inject
   * @param {number} [opts.maxBodyLength=2000] - Maximum total characters for memory context
   * @param {number} [opts.maxEntityMemories=3] - Maximum entity memories per entity
   * @param {Object|null} [opts.knowledgeStore=null] - Optional vector store for knowledge RAG
   * @param {number} [opts.maxKnowledgeResults=3] - Maximum knowledge chunks to inject
   */
  constructor(opts = {}) {
    this._maxMemories = opts.maxMemories || 5;
    this._maxBodyLength = opts.maxBodyLength || 2000;
    this._maxEntityMemories = opts.maxEntityMemories || 3;
    this._knowledgeStore = opts.knowledgeStore || null;
    this._maxKnowledgeResults = opts.maxKnowledgeResults || 3;
  }

  /**
   * Hook handler: prepend memory context (and entity context) to the agent prompt.
   *
   * When entity IDs are detected in the user message, a supplementary
   * `<entity-context>` block is included alongside the standard
   * `<memory-context>` block so the agent has immediate access to prior
   * interactions with those specific entities.
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

      // ---- Standard recent-memory context ----
      const memories = store.getRecent(channel, senderId, this._maxMemories);
      const formatted = memories.length > 0 ? this.formatMemories(memories) : null;

      // ---- Entity-aware context ----
      const entities = extractEntityIds(data.text);
      const entityFormatted =
        entities.length > 0 && typeof store.searchByEntity === 'function'
          ? this.formatEntityContext(store, channel, senderId, entities)
          : null;

      // ---- Knowledge base RAG context ----
      const knowledgeFormatted = await this.formatKnowledgeContext(data.text);

      if (!formatted && !entityFormatted && !knowledgeFormatted) return data;

      const parts = [];
      if (formatted) parts.push(formatted);
      if (entityFormatted) parts.push(entityFormatted);
      if (knowledgeFormatted) parts.push(knowledgeFormatted);

      return {
        ...data,
        text: parts.join('\n\n') + '\n\n' + data.text,
      };
    } catch (err) {
      // Memory is optional — don't fail the request
      console.warn(`[MemoryInjector] Failed to load memories: ${err.message}`);
      return data;
    }
  }

  /**
   * Query the store for each detected entity and format a combined
   * `<entity-context>` block.  Returns `null` when no entity memories exist.
   *
   * @param {import('./store.js').MemoryStore} store
   * @param {string} channel
   * @param {string} senderId
   * @param {Array<{ type: string, id: string }>} entities
   * @returns {string|null}
   */
  formatEntityContext(store, channel, senderId, entities) {
    const lines = ['<entity-context>', 'Entity-specific memory:'];
    let totalLen = 0;
    let found = 0;

    for (const { type, id } of entities) {
      let entityMemories;
      try {
        entityMemories = store.searchByEntity(channel, senderId, type, id, this._maxEntityMemories);
      } catch (err) {
        console.debug(`[MemoryInjector] Entity search failed for ${type}:${id}: ${err.message}`);
        continue;
      }

      if (!entityMemories || entityMemories.length === 0) continue;

      const header = `[${type.toUpperCase()}: ${id}]`;
      if (totalLen + header.length > this._maxBodyLength) break;
      lines.push(header);
      totalLen += header.length;

      for (const mem of entityMemories) {
        const date = new Date(mem.created_at).toISOString().split('T')[0];
        const agent = mem.agent ? ` (${mem.agent})` : '';
        const entry = `  - [${date}${agent}] ${mem.summary}`;
        if (totalLen + entry.length > this._maxBodyLength) break;
        lines.push(entry);
        totalLen += entry.length;

        if (mem.facts && mem.facts.length > 0) {
          const factsLine = `    Facts: ${mem.facts.join(', ')}`;
          if (totalLen + factsLine.length <= this._maxBodyLength) {
            lines.push(factsLine);
            totalLen += factsLine.length;
          }
        }
        found++;
      }
    }

    if (found === 0) return null;

    lines.push('</entity-context>');
    return lines.join('\n');
  }

  /**
   * Query the knowledge vector store with the user message and return a
   * `<knowledge-context>` block containing the most relevant chunks.
   * Returns `null` when no knowledge store is configured or no results are found.
   *
   * @param {string} userMessage - The user's input text used as the query
   * @returns {Promise<string|null>}
   */
  async formatKnowledgeContext(userMessage) {
    if (!this._knowledgeStore || typeof this._knowledgeStore.search !== 'function') {
      return null;
    }
    if (!userMessage || typeof userMessage !== 'string') return null;

    let results;
    try {
      results = await this._knowledgeStore.search(userMessage, this._maxKnowledgeResults);
    } catch (err) {
      console.debug(`[MemoryInjector] Knowledge store search failed: ${err.message}`);
      return null;
    }

    if (!results || results.length === 0) return null;

    const lines = ['<knowledge-context>', 'Relevant commerce knowledge:'];
    let totalLen = 0;

    for (const result of results) {
      const topic = result.metadata?.topic || result.id || 'general';
      const header = `[${topic}]`;
      const body = result.content || result.text || '';
      if (!body) continue;

      const entry = `${header}\n${body}`;
      if (totalLen + entry.length > this._maxBodyLength) break;

      lines.push(entry);
      totalLen += entry.length;
    }

    if (lines.length <= 2) return null; // only header lines, no actual content

    lines.push('</knowledge-context>');
    return lines.join('\n');
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

  /**
   * Set max entity memories per entity reference.
   * @param {number} n
   */
  setMaxEntityMemories(n) {
    this._maxEntityMemories = n;
  }

  /**
   * Attach or replace the knowledge vector store used for RAG context injection.
   * Pass `null` to disable knowledge context.
   *
   * @param {Object|null} store - Vector store with a `search(query, limit)` method
   */
  setKnowledgeStore(store) {
    this._knowledgeStore = store;
  }

  /**
   * Set maximum number of knowledge chunks to inject per request.
   * @param {number} n
   */
  setMaxKnowledgeResults(n) {
    this._maxKnowledgeResults = n;
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
