/**
 * Memory Module for StateSet CLI
 *
 * Provides multiple memory storage backends:
 *
 * 1. MemoryStore (SQLite) - Structured conversation memory with SQL queries
 * 2. MarkdownMemoryStore - Human-readable markdown files for transparency
 * 3. VectorStore - Semantic search using embeddings
 *
 * Usage:
 *   import { getMemoryStore, getMarkdownMemoryStore, getVectorStore } from './memory/index.js';
 *
 *   // SQLite-backed memory (fast, queryable)
 *   const sqlStore = getMemoryStore();
 *
 *   // Markdown files (transparent, debuggable)
 *   const mdStore = getMarkdownMemoryStore();
 *
 *   // Vector search (semantic)
 *   const vecStore = getVectorStore();
 */

// SQLite-backed structured memory
export { MemoryStore, getMemoryStore, resetMemoryStore } from './store.js';

// Human-readable markdown memory
export {
  MarkdownMemoryStore,
  getMarkdownMemoryStore,
  resetMarkdownMemoryStore,
  parseMemoryFile,
  formatEntry,
} from './markdown-store.js';

// Vector/semantic memory
export { VectorStore, getVectorStore, resetVectorStore } from './vector-store.js';

// Memory injection into prompts
export { MemoryInjector, createMemoryInjector } from './injector.js';

// Conversation summarization
export { ConversationSummarizer, createSummarizer } from './summarizer.js';

// Unified memory manager that uses multiple stores
export class UnifiedMemory {
  /**
   * @param {object} [options]
   * @param {boolean} [options.useSqlite=true] - Use SQLite store
   * @param {boolean} [options.useMarkdown=true] - Use markdown store
   * @param {boolean} [options.useVector=false] - Use vector store
   */
  constructor(options = {}) {
    this.useSqlite = options.useSqlite ?? true;
    this.useMarkdown = options.useMarkdown ?? true;
    this.useVector = options.useVector ?? false;

    this._sqlStore = null;
    this._mdStore = null;
    this._vecStore = null;
  }

  get sqliteStore() {
    if (!this._sqlStore && this.useSqlite) {
      const { getMemoryStore } = require('./store.js');
      this._sqlStore = getMemoryStore();
    }
    return this._sqlStore;
  }

  get markdownStore() {
    if (!this._mdStore && this.useMarkdown) {
      const { getMarkdownMemoryStore } = require('./markdown-store.js');
      this._mdStore = getMarkdownMemoryStore();
    }
    return this._mdStore;
  }

  get vectorStore() {
    if (!this._vecStore && this.useVector) {
      const { getVectorStore } = require('./vector-store.js');
      this._vecStore = getVectorStore();
    }
    return this._vecStore;
  }

  /**
   * Save memory to all enabled stores
   *
   * @param {object} entry
   * @param {string} entry.summary
   * @param {string[]} [entry.facts]
   * @param {string} [entry.agent]
   * @param {string} [entry.sessionId]
   * @param {string} [entry.channel]
   * @param {string} [entry.senderId]
   */
  async save(entry) {
    const promises = [];

    if (this.useSqlite && this.sqliteStore) {
      promises.push(
        Promise.resolve(
          this.sqliteStore.save({
            channel: entry.channel || 'cli',
            senderId: entry.senderId || 'local',
            sessionId: entry.sessionId,
            summary: entry.summary,
            facts: entry.facts || [],
            agent: entry.agent,
          }),
        ),
      );
    }

    if (this.useMarkdown && this.markdownStore) {
      promises.push(this.markdownStore.save(entry));
    }

    if (this.useVector && this.vectorStore) {
      // Vector store requires embedding - skip for now
    }

    await Promise.all(promises);
  }

  /**
   * Search across all enabled stores
   *
   * @param {string} query
   * @param {number} [limit=10]
   * @returns {Promise<object[]>}
   */
  async search(query, limit = 10) {
    const results = [];

    if (this.useSqlite && this.sqliteStore) {
      const sqlResults = this.sqliteStore.search('cli', 'local', query, limit);
      results.push(...sqlResults.map((r) => ({ ...r, source: 'sqlite' })));
    }

    if (this.useMarkdown && this.markdownStore) {
      const mdResults = await this.markdownStore.search(query, limit);
      results.push(...mdResults.map((r) => ({ ...r, source: 'markdown' })));
    }

    // Deduplicate by summary
    const seen = new Set();
    return results
      .filter((r) => {
        const key = r.summary?.slice(0, 100);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .slice(0, limit);
  }

  /**
   * Get recent memories
   *
   * @param {number} [limit=10]
   * @returns {Promise<object[]>}
   */
  async getRecent(limit = 10) {
    const results = [];

    if (this.useSqlite && this.sqliteStore) {
      const sqlResults = this.sqliteStore.getRecent('cli', 'local', limit);
      results.push(...sqlResults.map((r) => ({ ...r, source: 'sqlite' })));
    }

    if (this.useMarkdown && this.markdownStore) {
      const mdResults = await this.markdownStore.getRecent(limit);
      results.push(...mdResults.map((r) => ({ ...r, source: 'markdown' })));
    }

    // Sort by timestamp and deduplicate
    return results
      .sort((a, b) => {
        const aTime = a.created_at || a.timestamp || 0;
        const bTime = b.created_at || b.timestamp || 0;
        return bTime - aTime;
      })
      .slice(0, limit);
  }

  /**
   * Get statistics
   */
  async getStats() {
    const stats = {};

    if (this.useSqlite && this.sqliteStore) {
      stats.sqlite = { count: this.sqliteStore.count() };
    }

    if (this.useMarkdown && this.markdownStore) {
      stats.markdown = await this.markdownStore.getStats();
    }

    return stats;
  }
}

// Singleton unified memory
let _unifiedInstance = null;

export function getUnifiedMemory(options) {
  if (!_unifiedInstance) {
    _unifiedInstance = new UnifiedMemory(options);
  }
  return _unifiedInstance;
}

export function resetUnifiedMemory() {
  _unifiedInstance = null;
}

export default {
  // SQLite
  MemoryStore: require('./store.js').MemoryStore,
  getMemoryStore: require('./store.js').getMemoryStore,

  // Markdown
  MarkdownMemoryStore: require('./markdown-store.js').MarkdownMemoryStore,
  getMarkdownMemoryStore: require('./markdown-store.js').getMarkdownMemoryStore,

  // Vector
  VectorStore: require('./vector-store.js').VectorStore,
  getVectorStore: require('./vector-store.js').getVectorStore,

  // Unified
  UnifiedMemory,
  getUnifiedMemory,
};
