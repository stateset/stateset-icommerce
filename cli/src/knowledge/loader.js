/**
 * Knowledge Base Loader for StateSet iCommerce
 *
 * Reads markdown knowledge files from the knowledge/ directory, chunks them
 * into manageable pieces, and indexes each chunk into the provided vector
 * store so they can be retrieved via semantic similarity search.
 */

import fs from 'node:fs/promises';
import path from 'node:path';

export class KnowledgeLoader {
  /**
   * @param {Object} vectorStore - A vector store with an `upsert(doc)` method
   */
  constructor(vectorStore) {
    this._store = vectorStore;
    this._loaded = false;
  }

  /**
   * Load all markdown files from the knowledge directory and index them.
   * Idempotent — subsequent calls after the first are no-ops.
   *
   * @param {string} [knowledgeDir] - Directory to scan; defaults to this file's directory
   * @returns {Promise<void>}
   */
  async loadAll(knowledgeDir) {
    if (this._loaded) return;
    const dir = knowledgeDir || path.join(import.meta.dirname, '.');
    let files;
    try {
      files = await fs.readdir(dir);
    } catch (err) {
      console.warn(`[KnowledgeLoader] Cannot read knowledge directory ${dir}: ${err.message}`);
      return;
    }
    const mdFiles = files.filter((f) => f.endsWith('.md'));

    for (const file of mdFiles) {
      let content;
      try {
        content = await fs.readFile(path.join(dir, file), 'utf-8');
      } catch (err) {
        console.warn(`[KnowledgeLoader] Failed to read ${file}: ${err.message}`);
        continue;
      }
      const topic = file.replace('.md', '');
      const chunks = this.chunk(content);
      await this.indexChunks(topic, chunks);
    }
    this._loaded = true;
  }

  /**
   * Split a markdown document into chunks suitable for embedding.
   * Splits first by heading boundaries, then by paragraph if a section is too long.
   *
   * @param {string} text - Raw markdown text
   * @param {number} [maxChunkSize=512] - Maximum characters per chunk
   * @returns {string[]}
   */
  chunk(text, maxChunkSize = 512) {
    // Split on lines that start with one to three # characters (headings)
    const sections = text.split(/\n#{1,3}\s+/);
    const chunks = [];

    for (const section of sections) {
      const trimmed = section.trim();
      if (!trimmed) continue;

      if (trimmed.length <= maxChunkSize) {
        chunks.push(trimmed);
      } else {
        // Split oversized sections by paragraph
        const paragraphs = trimmed.split(/\n\n+/);
        let current = '';
        for (const para of paragraphs) {
          if (current.length + para.length + 2 > maxChunkSize && current) {
            chunks.push(current.trim());
            current = para;
          } else {
            current = current ? current + '\n\n' + para : para;
          }
        }
        if (current.trim()) chunks.push(current.trim());
      }
    }

    return chunks;
  }

  /**
   * Upsert chunks into the vector store.
   * Skips silently if the store is unavailable or does not expose `upsert`.
   *
   * @param {string} source - Topic name derived from filename (without .md)
   * @param {string[]} chunks - Text chunks to index
   * @returns {Promise<void>}
   */
  async indexChunks(source, chunks) {
    if (!this._store || typeof this._store.upsert !== 'function') return;
    for (let i = 0; i < chunks.length; i++) {
      try {
        await this._store.upsert({
          id: `knowledge:${source}:${i}`,
          content: chunks[i],
          metadata: { source: 'knowledge', topic: source, chunkIndex: i },
        });
      } catch (err) {
        console.debug(`[KnowledgeLoader] Failed to index chunk ${source}:${i}: ${err.message}`);
      }
    }
  }

  /**
   * Whether the knowledge base has been loaded at least once.
   * @returns {boolean}
   */
  get isLoaded() {
    return this._loaded;
  }

  /**
   * Reset the loaded flag (useful for testing).
   */
  reset() {
    this._loaded = false;
  }
}
