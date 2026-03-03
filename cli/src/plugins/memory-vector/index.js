/**
 * Vector Memory Plugin for StateSet iCommerce
 *
 * Provides in-memory vector storage for conversation memory using
 * TF-IDF bag-of-words embeddings with cosine similarity search.
 *
 * No external dependencies required.
 */

// ============================================================================
// Vector Math
// ============================================================================

/**
 * Tokenize text into lowercase terms.
 * @param {string} text
 * @returns {string[]}
 */
function tokenize(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, ' ')
    .split(/\s+/)
    .filter((t) => t.length > 1);
}

/**
 * Build a term-frequency vector from tokens.
 * @param {string[]} tokens
 * @returns {Map<string, number>}
 */
function termFrequency(tokens) {
  const tf = new Map();
  for (const token of tokens) {
    tf.set(token, (tf.get(token) || 0) + 1);
  }
  // Normalize by total token count
  const total = tokens.length || 1;
  for (const [term, count] of tf) {
    tf.set(term, count / total);
  }
  return tf;
}

/**
 * Compute cosine similarity between two term-frequency maps.
 * @param {Map<string, number>} a
 * @param {Map<string, number>} b
 * @returns {number} - Similarity in [0, 1]
 */
function cosineSimilarity(a, b) {
  let dot = 0;
  let magA = 0;
  let magB = 0;

  for (const [term, val] of a) {
    magA += val * val;
    if (b.has(term)) {
      dot += val * b.get(term);
    }
  }

  for (const val of b.values()) {
    magB += val * val;
  }

  if (magA === 0 || magB === 0) return 0;
  return dot / (Math.sqrt(magA) * Math.sqrt(magB));
}

// ============================================================================
// VectorStore
// ============================================================================

class VectorStore {
  /**
   * @param {Object} opts
   * @param {number} opts.maxMemories
   * @param {number} opts.maxAgeMs
   * @param {number} opts.topK
   */
  constructor(opts) {
    this._maxMemories = opts.maxMemories || 1000;
    this._maxAgeMs = opts.maxAgeMs || 7 * 24 * 60 * 60 * 1000;
    this._topK = opts.topK || 5;

    /** @type {Array<{ id: string, text: string, vector: Map<string, number>, timestamp: number, senderId?: string }>} */
    this._memories = [];
    this._nextId = 1;
  }

  /**
   * Store a memory.
   * @param {string} text
   * @param {string} [senderId]
   * @returns {{ id: string }}
   */
  add(text, senderId) {
    const tokens = tokenize(text);
    if (tokens.length === 0) return { id: null };

    const vector = termFrequency(tokens);
    const id = `mem-${this._nextId++}`;

    this._memories.push({
      id,
      text,
      vector,
      timestamp: Date.now(),
      senderId,
    });

    // Evict oldest if over limit
    while (this._memories.length > this._maxMemories) {
      this._memories.shift();
    }

    return { id };
  }

  /**
   * Search for similar memories.
   * @param {string} query
   * @param {number} [topK]
   * @returns {Array<{ id: string, text: string, score: number, timestamp: number }>}
   */
  search(query, topK) {
    const k = topK || this._topK;
    const queryTokens = tokenize(query);
    if (queryTokens.length === 0) return [];

    const queryVector = termFrequency(queryTokens);

    const scored = this._memories.map((mem) => ({
      id: mem.id,
      text: mem.text,
      score: cosineSimilarity(queryVector, mem.vector),
      timestamp: mem.timestamp,
    }));

    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, k).filter((s) => s.score > 0);
  }

  /**
   * Prune memories older than maxAgeMs.
   * @returns {number} - Number of pruned memories
   */
  prune() {
    const cutoff = Date.now() - this._maxAgeMs;
    const before = this._memories.length;
    this._memories = this._memories.filter((m) => m.timestamp >= cutoff);
    return before - this._memories.length;
  }

  /**
   * Clear all memories.
   * @returns {number} - Number of cleared memories
   */
  clear() {
    const count = this._memories.length;
    this._memories = [];
    return count;
  }

  /**
   * Get memory count.
   * @returns {number}
   */
  get size() {
    return this._memories.length;
  }

  /**
   * Get stats.
   */
  getStats() {
    return {
      count: this._memories.length,
      maxMemories: this._maxMemories,
      maxAgeMs: this._maxAgeMs,
      topK: this._topK,
      oldestMs: this._memories.length > 0 ? Date.now() - this._memories[0].timestamp : 0,
    };
  }
}

// ============================================================================
// Plugin Init
// ============================================================================

/**
 * Initialize the Vector Memory plugin.
 *
 * @param {import('../../channels/plugin-api.js').PluginAPI} api
 * @param {Object} ctx
 * @param {Object} ctx.config - Plugin config
 */
export default function init(api, { config }) {
  const store = new VectorStore({
    maxMemories: config.maxMemories || 1000,
    maxAgeMs: config.maxAgeMs || 604800000,
    topK: config.topK || 5,
  });

  let pruneTimer = null;

  // --- Hooks ---

  // Store incoming messages as memories
  api.on(
    'message_received',
    async ({ text, senderId }) => {
      if (!text || text.startsWith('/')) return;
      store.add(text, senderId);
    },
    { priority: 200 },
  );

  // Inject relevant memories before agent processing
  api.on(
    'before_agent_start',
    async ({ text }) => {
      if (!text || text.startsWith('/')) return {};

      const results = store.search(text);
      if (results.length === 0) return {};

      const memoryContext = results
        .map((r) => `[${new Date(r.timestamp).toISOString().slice(0, 16)}] ${r.text}`)
        .join('\n');

      const augmented = `${text}\n\n[Relevant memories]\n${memoryContext}`;
      return { text: augmented };
    },
    { priority: 50 },
  );

  // --- Commands ---

  api.registerCommand({
    name: 'remember',
    description: 'Store a memory',
    acceptsArgs: true,
    handler: async (argText) => {
      if (!argText || !argText.trim()) {
        return { response: 'Usage: /remember <text to remember>' };
      }
      const { id } = store.add(argText.trim());
      if (!id) {
        return { response: 'Could not store memory (text too short).' };
      }
      return { response: `Stored memory ${id}. Total: ${store.size}` };
    },
  });

  api.registerCommand({
    name: 'recall',
    description: 'Search memories by similarity',
    acceptsArgs: true,
    handler: async (argText) => {
      if (!argText || !argText.trim()) {
        return { response: 'Usage: /recall <search query>' };
      }
      const results = store.search(argText.trim());
      if (results.length === 0) {
        return { response: 'No relevant memories found.' };
      }
      const lines = results.map(
        (r, i) => `${i + 1}. [${(r.score * 100).toFixed(0)}%] ${r.text.slice(0, 120)}`,
      );
      return { response: `Found ${results.length} memories:\n${lines.join('\n')}` };
    },
  });

  api.registerCommand({
    name: 'forget',
    description: 'Clear all stored memories',
    acceptsArgs: false,
    handler: async () => {
      const count = store.clear();
      return { response: `Cleared ${count} memories.` };
    },
  });

  api.registerCommand({
    name: 'memories',
    description: 'Show memory store statistics',
    acceptsArgs: false,
    handler: async () => {
      const stats = store.getStats();
      const lines = [
        `Memories: ${stats.count} / ${stats.maxMemories}`,
        `Top K: ${stats.topK}`,
        `Max Age: ${(stats.maxAgeMs / 86400000).toFixed(1)} days`,
        stats.oldestMs > 0 ? `Oldest: ${(stats.oldestMs / 60000).toFixed(0)} min ago` : '',
      ].filter(Boolean);
      return { response: lines.join('\n') };
    },
  });

  // --- Background Service ---

  api.registerService({
    name: 'memory-pruner',
    start: async () => {
      const interval = config.pruneIntervalMs || 3600000;
      pruneTimer = setInterval(() => {
        const pruned = store.prune();
        if (pruned > 0) {
          console.debug(
            `[memory-vector] Pruned ${pruned} expired memories. Remaining: ${store.size}`,
          );
        }
      }, interval);
      if (pruneTimer.unref) pruneTimer.unref();
      console.debug(`[memory-vector] Pruner started (interval: ${interval}ms)`);
    },
    stop: async () => {
      if (pruneTimer) {
        clearInterval(pruneTimer);
        pruneTimer = null;
      }
      console.debug('[memory-vector] Pruner stopped.');
    },
  });
}
