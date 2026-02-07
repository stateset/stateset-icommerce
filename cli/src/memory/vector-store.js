/**
 * Vector Memory Store for StateSet iCommerce
 *
 * Extends the base MemoryStore with vector embeddings for semantic search.
 * Uses a TF-IDF hashing trick projection as a zero-dependency fallback
 * when no external embedding API is available.
 *
 * Provides:
 *   - embed(text)           → Float64Array dense vector
 *   - vectorSearch(query)   → semantic similarity results
 *   - hybridSearch(query)   → RRF fusion of text LIKE + vector similarity
 *   - batchEmbed(texts)     → embed multiple texts at once
 */

import Database from 'better-sqlite3';
import { getMemoryStore } from './store.js';

// ============================================================================
// Constants
// ============================================================================

const VECTOR_DIM = 256;
const RRF_K = 60; // Reciprocal Rank Fusion constant

// ============================================================================
// Schema extension
// ============================================================================

const VECTOR_SCHEMA = `
CREATE TABLE IF NOT EXISTS memory_vectors (
  memory_id    INTEGER PRIMARY KEY REFERENCES conversation_memory(id) ON DELETE CASCADE,
  embedding    BLOB    NOT NULL,
  norm         REAL    NOT NULL DEFAULT 1.0,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch() * 1000)
);

CREATE INDEX IF NOT EXISTS idx_vectors_memory
  ON memory_vectors(memory_id);
`;

// ============================================================================
// TF-IDF Hashing Trick Embedder (zero-dependency fallback)
// ============================================================================

/**
 * FNV-1a 32-bit hash.
 * @param {string} str
 * @returns {number}
 */
function fnv1a(str) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    hash ^= str.charCodeAt(i);
    hash = (hash * 0x01000193) >>> 0;
  }
  return hash;
}

/**
 * Tokenize text into lowercased word tokens.
 * @param {string} text
 * @returns {string[]}
 */
function tokenize(text) {
  return text
    .toLowerCase()
    .replace(/[^\w\s]/g, ' ')
    .split(/\s+/)
    .filter((t) => t.length > 1 && t.length < 40);
}

/**
 * Build a TF-IDF sparse vector and project via hashing trick to a dense vector.
 * @param {string} text
 * @param {number} [dim=VECTOR_DIM]
 * @returns {Float64Array}
 */
function hashEmbed(text, dim = VECTOR_DIM) {
  const tokens = tokenize(text);
  if (tokens.length === 0) return new Float64Array(dim);

  // Term frequency
  const tf = new Map();
  for (const t of tokens) {
    tf.set(t, (tf.get(t) || 0) + 1);
  }

  // Log-scaled TF, project with hashing trick
  const vec = new Float64Array(dim);
  for (const [term, count] of tf) {
    const logTf = 1 + Math.log(count);
    const h = fnv1a(term);
    const bucket = h % dim;
    // Use second hash for sign to reduce collisions
    const sign = fnv1a(term + '\x00') & 1 ? 1 : -1;
    vec[bucket] += logTf * sign;
  }

  // Also add bigrams for better semantic capture
  for (let i = 0; i < tokens.length - 1; i++) {
    const bigram = tokens[i] + '_' + tokens[i + 1];
    const h = fnv1a(bigram);
    const bucket = h % dim;
    const sign = fnv1a(bigram + '\x00') & 1 ? 1 : -1;
    vec[bucket] += 0.5 * sign;
  }

  return vec;
}

/**
 * Compute L2 norm of a vector.
 * @param {Float64Array} vec
 * @returns {number}
 */
function l2Norm(vec) {
  let sum = 0;
  for (let i = 0; i < vec.length; i++) {
    sum += vec[i] * vec[i];
  }
  return Math.sqrt(sum);
}

/**
 * Cosine similarity between two vectors.
 * @param {Float64Array} a
 * @param {Float64Array} b
 * @param {number} [normA]
 * @param {number} [normB]
 * @returns {number} Similarity in [-1, 1]
 */
function cosineSimilarity(a, b, normA, normB) {
  let dot = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
  }
  const na = normA || l2Norm(a);
  const nb = normB || l2Norm(b);
  if (na === 0 || nb === 0) return 0;
  return dot / (na * nb);
}

// ============================================================================
// VectorMemoryStore
// ============================================================================

export class VectorMemoryStore {
  /**
   * @param {Object} [opts]
   * @param {import('./store.js').MemoryStore} [opts.memoryStore] - Base memory store (uses singleton if not provided)
   * @param {string} [opts.dbPath] - Database path for the vector table
   * @param {number} [opts.dim] - Embedding dimension (default: 256)
   * @param {Function} [opts.embedFn] - Custom embedding function (default: hashEmbed)
   */
  constructor(opts = {}) {
    this._base = opts.memoryStore || getMemoryStore(opts);
    this._dim = opts.dim || VECTOR_DIM;
    this._embedFn = opts.embedFn || ((text) => hashEmbed(text, this._dim));

    // Open or reuse the base store's DB path for the vector table
    const dbPath = opts.dbPath || this._base._dbPath;
    this._db = new Database(dbPath);
    this._db.pragma('journal_mode = WAL');
    this._db.exec(VECTOR_SCHEMA);

    // Prepared statements
    this._insertVecStmt = this._db.prepare(`
      INSERT OR REPLACE INTO memory_vectors (memory_id, embedding, norm, created_at)
      VALUES (?, ?, ?, ?)
    `);

    this._getVecStmt = this._db.prepare(`
      SELECT embedding, norm FROM memory_vectors WHERE memory_id = ?
    `);

    this._allVecsStmt = this._db.prepare(`
      SELECT mv.memory_id, mv.embedding, mv.norm,
             cm.channel, cm.sender_id, cm.summary, cm.facts, cm.agent, cm.created_at
      FROM memory_vectors mv
      JOIN conversation_memory cm ON cm.id = mv.memory_id
      WHERE cm.channel = ? AND cm.sender_id = ?
      ORDER BY cm.created_at DESC
    `);

    this._allVecsGlobalStmt = this._db.prepare(`
      SELECT mv.memory_id, mv.embedding, mv.norm,
             cm.channel, cm.sender_id, cm.summary, cm.facts, cm.agent, cm.created_at
      FROM memory_vectors mv
      JOIN conversation_memory cm ON cm.id = mv.memory_id
      ORDER BY cm.created_at DESC
      LIMIT ?
    `);

    this._deleteVecStmt = this._db.prepare(`
      DELETE FROM memory_vectors WHERE memory_id = ?
    `);

    this._countVecsStmt = this._db.prepare(`
      SELECT COUNT(*) as cnt FROM memory_vectors
    `);
  }

  /**
   * Get the underlying base MemoryStore.
   * @returns {MemoryStore}
   */
  get base() {
    return this._base;
  }

  // --------------------------------------------------------------------------
  // Embedding
  // --------------------------------------------------------------------------

  /**
   * Embed a text string into a dense vector.
   * @param {string} text
   * @returns {Float64Array}
   */
  embed(text) {
    return this._embedFn(text);
  }

  /**
   * Embed multiple texts.
   * @param {string[]} texts
   * @returns {Float64Array[]}
   */
  batchEmbed(texts) {
    return texts.map((t) => this.embed(t));
  }

  // --------------------------------------------------------------------------
  // Save with embedding
  // --------------------------------------------------------------------------

  /**
   * Save a conversation memory with its vector embedding.
   * @param {Object} entry - Same as MemoryStore.save() entry
   * @returns {{ id: number }}
   */
  save(entry) {
    const { id } = this._base.save(entry);
    const embedding = this.embed(entry.summary);
    const norm = l2Norm(embedding);
    const blob = Buffer.from(embedding.buffer);
    this._insertVecStmt.run(id, blob, norm, Date.now());
    return { id };
  }

  // --------------------------------------------------------------------------
  // Vector search
  // --------------------------------------------------------------------------

  /**
   * Search memories by vector similarity (cosine).
   * @param {string} query
   * @param {Object} [opts]
   * @param {string} [opts.channel]
   * @param {string} [opts.senderId]
   * @param {number} [opts.limit=5]
   * @param {number} [opts.minSimilarity=0.1]
   * @returns {Array<{ id: number, summary: string, similarity: number, facts: string[], agent: string, createdAt: number }>}
   */
  vectorSearch(query, opts = {}) {
    const { channel, senderId, limit = 5, minSimilarity = 0.1 } = opts;
    const queryVec = this.embed(query);
    const queryNorm = l2Norm(queryVec);

    if (queryNorm === 0) return [];

    // Get candidate vectors
    let rows;
    if (channel && senderId) {
      rows = this._allVecsStmt.all(channel, senderId);
    } else {
      rows = this._allVecsGlobalStmt.all(Math.max(limit * 10, 200));
    }

    // Score each candidate
    const scored = [];
    for (const row of rows) {
      const embedding = new Float64Array(
        row.embedding.buffer.slice(
          row.embedding.byteOffset,
          row.embedding.byteOffset + row.embedding.byteLength,
        ),
      );
      const sim = cosineSimilarity(queryVec, embedding, queryNorm, row.norm);
      if (sim >= minSimilarity) {
        scored.push({
          id: row.memory_id,
          summary: row.summary,
          facts: row.facts ? JSON.parse(row.facts) : [],
          agent: row.agent,
          channel: row.channel,
          senderId: row.sender_id,
          createdAt: row.created_at,
          similarity: sim,
        });
      }
    }

    // Sort by similarity descending
    scored.sort((a, b) => b.similarity - a.similarity);
    return scored.slice(0, limit);
  }

  // --------------------------------------------------------------------------
  // Hybrid search (RRF fusion)
  // --------------------------------------------------------------------------

  /**
   * Hybrid search combining text LIKE match and vector similarity using
   * Reciprocal Rank Fusion (RRF).
   *
   * @param {string} query
   * @param {Object} [opts]
   * @param {string} [opts.channel]
   * @param {string} [opts.senderId]
   * @param {number} [opts.limit=5]
   * @param {number} [opts.textWeight=1.0] - Weight for text search ranking
   * @param {number} [opts.vectorWeight=1.0] - Weight for vector search ranking
   * @returns {Array<{ id: number, summary: string, score: number, facts: string[], textRank?: number, vectorRank?: number }>}
   */
  hybridSearch(query, opts = {}) {
    const {
      channel = 'cli',
      senderId = 'local',
      limit = 5,
      textWeight = 1.0,
      vectorWeight = 1.0,
    } = opts;

    // Text search via base store
    const textResults = this._base.search(channel, senderId, query, limit * 3);

    // Vector search
    const vectorResults = this.vectorSearch(query, {
      channel,
      senderId,
      limit: limit * 3,
      minSimilarity: 0.05,
    });

    // Build RRF score map
    const scoreMap = new Map();

    // Score text results
    for (let rank = 0; rank < textResults.length; rank++) {
      const item = textResults[rank];
      const rrf = textWeight / (RRF_K + rank + 1);
      const existing = scoreMap.get(item.id) || {
        id: item.id,
        summary: item.summary,
        facts: item.facts || [],
        agent: item.agent,
        createdAt: item.created_at,
        score: 0,
      };
      existing.score += rrf;
      existing.textRank = rank + 1;
      scoreMap.set(item.id, existing);
    }

    // Score vector results
    for (let rank = 0; rank < vectorResults.length; rank++) {
      const item = vectorResults[rank];
      const rrf = vectorWeight / (RRF_K + rank + 1);
      const existing = scoreMap.get(item.id) || {
        id: item.id,
        summary: item.summary,
        facts: item.facts || [],
        agent: item.agent,
        createdAt: item.createdAt,
        score: 0,
      };
      existing.score += rrf;
      existing.vectorRank = rank + 1;
      existing.similarity = item.similarity;
      scoreMap.set(item.id, existing);
    }

    // Sort by combined RRF score
    const merged = [...scoreMap.values()];
    merged.sort((a, b) => b.score - a.score);
    return merged.slice(0, limit);
  }

  // --------------------------------------------------------------------------
  // Utilities
  // --------------------------------------------------------------------------

  /**
   * Count vector embeddings stored.
   * @returns {number}
   */
  vectorCount() {
    return this._countVecsStmt.get().cnt;
  }

  /**
   * Rebuild vectors for all existing memories that lack embeddings.
   * @param {string} [channel]
   * @param {string} [senderId]
   * @returns {{ processed: number, errors: number }}
   */
  backfill(channel, senderId) {
    const memories =
      channel && senderId
        ? this._base.getRecent(channel, senderId, 10000)
        : this._base.getAllRecent(10000);

    let processed = 0;
    let errors = 0;

    const insertMany = this._db.transaction((items) => {
      for (const mem of items) {
        try {
          const existing = this._getVecStmt.get(mem.id);
          if (existing) continue;

          const embedding = this.embed(mem.summary);
          const norm = l2Norm(embedding);
          const blob = Buffer.from(embedding.buffer);
          this._insertVecStmt.run(mem.id, blob, norm, Date.now());
          processed++;
        } catch {
          errors++;
        }
      }
    });

    insertMany(memories);
    return { processed, errors };
  }

  /**
   * Delete a vector embedding for a memory.
   * @param {number} memoryId
   * @returns {boolean}
   */
  deleteVector(memoryId) {
    return this._deleteVecStmt.run(memoryId).changes > 0;
  }

  /**
   * Get stats about the vector store.
   * @returns {{ totalMemories: number, totalVectors: number, dim: number, embeddingType: string }}
   */
  stats() {
    return {
      totalMemories: this._base.count(),
      totalVectors: this.vectorCount(),
      dim: this._dim,
      embeddingType: this._embedFn === hashEmbed ? 'hash-tfidf' : 'custom',
    };
  }

  /**
   * Close the database connections.
   */
  close() {
    if (this._db) {
      this._db.close();
      this._db = null;
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _vectorInstance = null;

/**
 * Get the global VectorMemoryStore singleton.
 * @param {Object} [opts]
 * @returns {VectorMemoryStore}
 */
export function getVectorMemoryStore(opts) {
  if (!_vectorInstance) {
    _vectorInstance = new VectorMemoryStore(opts);
  }
  return _vectorInstance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetVectorMemoryStore() {
  if (_vectorInstance) {
    _vectorInstance.close();
    _vectorInstance = null;
  }
}

// Re-export utilities for testing
export { hashEmbed, cosineSimilarity, l2Norm, tokenize, fnv1a };
