/**
 * Markdown Memory Store for StateSet CLI
 *
 * Provides transparent, human-readable memory storage using markdown files.
 * Inspired by Clawdbot's simple memory architecture where agents write to
 * `memory/*.md` using standard file tools.
 *
 * Key benefits:
 * - Human-readable and inspectable
 * - Easy to debug and modify
 * - Git-friendly (can track memory changes)
 * - Simple to understand
 *
 * Directory structure:
 *   ~/.stateset/memory/
 *   ├── MEMORY.md           # Main memory file (auto-summarized)
 *   ├── sessions/
 *   │   └── {sessionId}.md  # Per-session detailed transcripts
 *   ├── entities/
 *   │   └── {type}_{id}.md  # Entity-specific memories (customer, order, etc.)
 *   └── topics/
 *       └── {topic}.md      # Topic-specific knowledge
 *
 * Usage:
 *   const store = new MarkdownMemoryStore();
 *   await store.save({ summary: 'User created order #123', facts: [...] });
 *   const memories = await store.search('order #123');
 */

import { promises as fs } from 'node:fs';
import { join, dirname } from 'node:path';
import { homedir } from 'node:os';
import { existsSync, mkdirSync } from 'node:fs';

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MEMORY_DIR = join(homedir(), '.stateset', 'memory');
const MAIN_MEMORY_FILE = 'MEMORY.md';
const MAX_MAIN_MEMORY_ENTRIES = 100;
const MAX_SESSION_ENTRIES = 50;

// ============================================================================
// Helpers
// ============================================================================

/**
 * Ensure directory exists
 */
function ensureDir(dir) {
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
}

/**
 * Format date for markdown
 */
function formatDate(date = new Date()) {
  return date.toISOString().replace('T', ' ').slice(0, 19);
}

/**
 * Parse markdown memory file into entries
 */
function parseMemoryFile(content) {
  if (!content || !content.trim()) return [];

  const entries = [];
  const sections = content.split(/^---$/m);

  for (const section of sections) {
    const trimmed = section.trim();
    if (!trimmed) continue;

    const entry = { raw: trimmed };

    // Extract timestamp
    const timestampMatch = trimmed.match(/^\*\*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\*\*/);
    if (timestampMatch) {
      entry.timestamp = timestampMatch[1];
    }

    // Extract summary (first line after timestamp)
    const lines = trimmed.split('\n');
    for (const line of lines) {
      if (line.startsWith('**Summary:**')) {
        entry.summary = line.replace('**Summary:**', '').trim();
        break;
      }
      // Also try to get summary from first non-timestamp line
      if (!entry.summary && !line.startsWith('**') && line.trim()) {
        entry.summary = line.trim();
      }
    }

    // Extract facts
    const factsMatch = trimmed.match(/\*\*Facts:\*\*\n((?:- .+\n?)+)/);
    if (factsMatch) {
      entry.facts = factsMatch[1]
        .split('\n')
        .filter(l => l.startsWith('- '))
        .map(l => l.slice(2).trim());
    }

    // Extract agent
    const agentMatch = trimmed.match(/\*\*Agent:\*\* (.+)/);
    if (agentMatch) {
      entry.agent = agentMatch[1].trim();
    }

    // Extract session
    const sessionMatch = trimmed.match(/\*\*Session:\*\* (.+)/);
    if (sessionMatch) {
      entry.sessionId = sessionMatch[1].trim();
    }

    entries.push(entry);
  }

  return entries;
}

/**
 * Format entry as markdown
 */
function formatEntry(entry) {
  const lines = [];

  // Timestamp
  lines.push(`**${formatDate(entry.createdAt ? new Date(entry.createdAt) : new Date())}**`);
  lines.push('');

  // Summary
  if (entry.summary) {
    lines.push(`**Summary:** ${entry.summary}`);
    lines.push('');
  }

  // Facts
  if (entry.facts && entry.facts.length > 0) {
    lines.push('**Facts:**');
    for (const fact of entry.facts) {
      lines.push(`- ${fact}`);
    }
    lines.push('');
  }

  // Metadata
  if (entry.agent) {
    lines.push(`**Agent:** ${entry.agent}`);
  }
  if (entry.sessionId) {
    lines.push(`**Session:** ${entry.sessionId}`);
  }

  return lines.join('\n');
}

// ============================================================================
// MarkdownMemoryStore
// ============================================================================

export class MarkdownMemoryStore {
  /**
   * @param {object} [options]
   * @param {string} [options.memoryDir] - Base directory for memory files
   * @param {number} [options.maxMainEntries] - Max entries in main memory file
   * @param {number} [options.maxSessionEntries] - Max entries per session file
   */
  constructor(options = {}) {
    this.memoryDir = options.memoryDir || DEFAULT_MEMORY_DIR;
    this.maxMainEntries = options.maxMainEntries || MAX_MAIN_MEMORY_ENTRIES;
    this.maxSessionEntries = options.maxSessionEntries || MAX_SESSION_ENTRIES;

    // Ensure directories exist
    ensureDir(this.memoryDir);
    ensureDir(join(this.memoryDir, 'sessions'));
    ensureDir(join(this.memoryDir, 'entities'));
    ensureDir(join(this.memoryDir, 'topics'));
  }

  // -------------------------------------------------------------------------
  // Main Memory
  // -------------------------------------------------------------------------

  /**
   * Get path to main memory file
   */
  get mainMemoryPath() {
    return join(this.memoryDir, MAIN_MEMORY_FILE);
  }

  /**
   * Save a memory entry to main memory file
   *
   * @param {object} entry
   * @param {string} entry.summary - Brief summary of the interaction
   * @param {string[]} [entry.facts] - Key facts extracted
   * @param {string} [entry.agent] - Agent that handled this
   * @param {string} [entry.sessionId] - Session ID
   * @returns {Promise<void>}
   */
  async save(entry) {
    const formatted = formatEntry({
      ...entry,
      createdAt: new Date()
    });

    // Read existing content
    let existing = '';
    try {
      existing = await fs.readFile(this.mainMemoryPath, 'utf-8');
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    // Parse existing entries
    const entries = parseMemoryFile(existing);

    // Trim if needed
    while (entries.length >= this.maxMainEntries) {
      entries.shift(); // Remove oldest
    }

    // Add new entry
    entries.push({ raw: formatted });

    // Write back
    const newContent = this._buildMemoryFile(entries);
    await fs.writeFile(this.mainMemoryPath, newContent, 'utf-8');

    // Also save to session file if sessionId provided
    if (entry.sessionId) {
      await this.saveToSession(entry.sessionId, entry);
    }
  }

  /**
   * Get recent memories from main memory file
   *
   * @param {number} [limit=10] - Max entries to return
   * @returns {Promise<object[]>}
   */
  async getRecent(limit = 10) {
    try {
      const content = await fs.readFile(this.mainMemoryPath, 'utf-8');
      const entries = parseMemoryFile(content);
      return entries.slice(-limit).reverse();
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * Search main memory file for matching entries
   *
   * @param {string} query - Search query
   * @param {number} [limit=10] - Max results
   * @returns {Promise<object[]>}
   */
  async search(query, limit = 10) {
    const entries = await this.getRecent(this.maxMainEntries);
    const lowerQuery = query.toLowerCase();

    const matches = entries.filter(entry => {
      const text = entry.raw?.toLowerCase() || '';
      return text.includes(lowerQuery);
    });

    return matches.slice(0, limit);
  }

  // -------------------------------------------------------------------------
  // Session Memory
  // -------------------------------------------------------------------------

  /**
   * Get path to session memory file
   */
  getSessionPath(sessionId) {
    return join(this.memoryDir, 'sessions', `${sessionId}.md`);
  }

  /**
   * Save entry to session-specific memory file
   *
   * @param {string} sessionId
   * @param {object} entry
   * @returns {Promise<void>}
   */
  async saveToSession(sessionId, entry) {
    const sessionPath = this.getSessionPath(sessionId);
    const formatted = formatEntry({
      ...entry,
      createdAt: new Date()
    });

    // Read existing
    let existing = '';
    try {
      existing = await fs.readFile(sessionPath, 'utf-8');
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    // Parse and trim
    const entries = parseMemoryFile(existing);
    while (entries.length >= this.maxSessionEntries) {
      entries.shift();
    }

    entries.push({ raw: formatted });

    // Write back
    const newContent = this._buildSessionFile(sessionId, entries);
    await fs.writeFile(sessionPath, newContent, 'utf-8');
  }

  /**
   * Get session memory
   *
   * @param {string} sessionId
   * @returns {Promise<object[]>}
   */
  async getSessionMemory(sessionId) {
    try {
      const content = await fs.readFile(this.getSessionPath(sessionId), 'utf-8');
      return parseMemoryFile(content);
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  // -------------------------------------------------------------------------
  // Entity Memory
  // -------------------------------------------------------------------------

  /**
   * Get path to entity memory file
   */
  getEntityPath(entityType, entityId) {
    const safeId = entityId.replace(/[^a-zA-Z0-9-_]/g, '_');
    return join(this.memoryDir, 'entities', `${entityType}_${safeId}.md`);
  }

  /**
   * Save memory about a specific entity (customer, order, etc.)
   *
   * @param {string} entityType - Type: 'customer', 'order', etc.
   * @param {string} entityId - Entity identifier
   * @param {object} entry - Memory entry
   * @returns {Promise<void>}
   */
  async saveEntityMemory(entityType, entityId, entry) {
    const entityPath = this.getEntityPath(entityType, entityId);
    const formatted = formatEntry({
      ...entry,
      createdAt: new Date()
    });

    // Read existing
    let existing = '';
    try {
      existing = await fs.readFile(entityPath, 'utf-8');
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    const entries = parseMemoryFile(existing);
    entries.push({ raw: formatted });

    // Write back
    const header = `# ${entityType.charAt(0).toUpperCase() + entityType.slice(1)}: ${entityId}\n\n`;
    const body = entries.map(e => e.raw).join('\n\n---\n\n');
    await fs.writeFile(entityPath, header + body, 'utf-8');
  }

  /**
   * Get memory for a specific entity
   *
   * @param {string} entityType
   * @param {string} entityId
   * @returns {Promise<object[]>}
   */
  async getEntityMemory(entityType, entityId) {
    try {
      const content = await fs.readFile(this.getEntityPath(entityType, entityId), 'utf-8');
      return parseMemoryFile(content);
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  // -------------------------------------------------------------------------
  // Topic Memory
  // -------------------------------------------------------------------------

  /**
   * Get path to topic memory file
   */
  getTopicPath(topic) {
    const safeTopic = topic.toLowerCase().replace(/[^a-zA-Z0-9-_]/g, '_');
    return join(this.memoryDir, 'topics', `${safeTopic}.md`);
  }

  /**
   * Save knowledge about a topic
   *
   * @param {string} topic - Topic name
   * @param {object} entry - Memory entry
   * @returns {Promise<void>}
   */
  async saveTopicMemory(topic, entry) {
    const topicPath = this.getTopicPath(topic);
    const formatted = formatEntry({
      ...entry,
      createdAt: new Date()
    });

    // Read existing
    let existing = '';
    try {
      existing = await fs.readFile(topicPath, 'utf-8');
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    const entries = parseMemoryFile(existing);
    entries.push({ raw: formatted });

    // Write back
    const header = `# Topic: ${topic}\n\n`;
    const body = entries.map(e => e.raw).join('\n\n---\n\n');
    await fs.writeFile(topicPath, header + body, 'utf-8');
  }

  /**
   * Get memory for a topic
   *
   * @param {string} topic
   * @returns {Promise<object[]>}
   */
  async getTopicMemory(topic) {
    try {
      const content = await fs.readFile(this.getTopicPath(topic), 'utf-8');
      return parseMemoryFile(content);
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  // -------------------------------------------------------------------------
  // Utilities
  // -------------------------------------------------------------------------

  /**
   * Build main memory file content
   * @private
   */
  _buildMemoryFile(entries) {
    const header = `# StateSet Memory\n\n_Auto-generated memory file. Last updated: ${formatDate()}_\n\n`;
    const body = entries.map(e => e.raw).join('\n\n---\n\n');
    return header + body;
  }

  /**
   * Build session file content
   * @private
   */
  _buildSessionFile(sessionId, entries) {
    const header = `# Session: ${sessionId}\n\n_Started: ${formatDate()}_\n\n`;
    const body = entries.map(e => e.raw).join('\n\n---\n\n');
    return header + body;
  }

  /**
   * List all session IDs
   * @returns {Promise<string[]>}
   */
  async listSessions() {
    try {
      const files = await fs.readdir(join(this.memoryDir, 'sessions'));
      return files
        .filter(f => f.endsWith('.md'))
        .map(f => f.slice(0, -3));
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * List all entities with memory
   * @returns {Promise<{ type: string, id: string }[]>}
   */
  async listEntities() {
    try {
      const files = await fs.readdir(join(this.memoryDir, 'entities'));
      return files
        .filter(f => f.endsWith('.md'))
        .map(f => {
          const name = f.slice(0, -3);
          const underscoreIdx = name.indexOf('_');
          return {
            type: name.slice(0, underscoreIdx),
            id: name.slice(underscoreIdx + 1)
          };
        });
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * List all topics with memory
   * @returns {Promise<string[]>}
   */
  async listTopics() {
    try {
      const files = await fs.readdir(join(this.memoryDir, 'topics'));
      return files
        .filter(f => f.endsWith('.md'))
        .map(f => f.slice(0, -3));
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * Get memory statistics
   * @returns {Promise<object>}
   */
  async getStats() {
    const mainEntries = await this.getRecent(1000);
    const sessions = await this.listSessions();
    const entities = await this.listEntities();
    const topics = await this.listTopics();

    return {
      mainMemoryEntries: mainEntries.length,
      sessions: sessions.length,
      entities: entities.length,
      topics: topics.length,
      memoryDir: this.memoryDir
    };
  }

  /**
   * Clear all memory (use with caution)
   * @returns {Promise<void>}
   */
  async clear() {
    const files = [
      this.mainMemoryPath,
      ...(await this.listSessions()).map(s => this.getSessionPath(s)),
      ...(await this.listEntities()).map(e => this.getEntityPath(e.type, e.id)),
      ...(await this.listTopics()).map(t => this.getTopicPath(t))
    ];

    for (const file of files) {
      try {
        await fs.unlink(file);
      } catch (e) {
        if (e.code !== 'ENOENT') throw e;
      }
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global MarkdownMemoryStore singleton
 * @param {object} [options]
 * @returns {MarkdownMemoryStore}
 */
export function getMarkdownMemoryStore(options) {
  if (!_instance) {
    _instance = new MarkdownMemoryStore(options);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing)
 */
export function resetMarkdownMemoryStore() {
  _instance = null;
}

export default {
  MarkdownMemoryStore,
  getMarkdownMemoryStore,
  resetMarkdownMemoryStore,
  parseMemoryFile,
  formatEntry
};
