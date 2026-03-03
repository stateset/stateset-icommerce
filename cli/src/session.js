/**
 * Session Persistence Module for StateSet CLI
 *
 * Manages session storage, history, and resume functionality.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import * as crypto from 'node:crypto';

const DIRECTORY_MODE = 0o700;
const FILE_MODE = 0o600;
const MAX_REDACTION_DEPTH = 6;
const SESSION_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;
const SENSITIVE_KEY_PATTERN = /(token|secret|password|api[-_]?key|authorization|cookie)/i;
const SENSITIVE_VALUE_PATTERNS = [
  {
    pattern: /(\bBearer)\s+[A-Za-z0-9._~+/=-]+/gi,
    replacement: '$1 [REDACTED]',
  },
  {
    pattern: /(\b(?:token|api[_-]?key|secret|password)\b\s*=\s*)([^\s,&]+)/gi,
    replacement: '$1[REDACTED]',
  },
  {
    pattern: /([?&](?:token|api[_-]?key|secret|password)=)[^&\s]+/gi,
    replacement: '$1[REDACTED]',
  },
  {
    pattern: /(--(?:token|api[-_]?key|secret|password)(?:=|\s+))([^\s]+)/gi,
    replacement: '$1[REDACTED]',
  },
  {
    pattern: /("(?:token|secret|password|api[_-]?key|authorization|cookie)"\s*:\s*")[^"]*(")/gi,
    replacement: '$1[REDACTED]$2',
  },
];

function setPermissionIfSupported(targetPath, mode) {
  try {
    fs.chmodSync(targetPath, mode);
  } catch (chmodErr) {
    console.debug('chmod not supported on this platform:', chmodErr.message);
  }
}

function ensureSecureDirectory(directoryPath) {
  fs.mkdirSync(directoryPath, { recursive: true, mode: DIRECTORY_MODE });
  setPermissionIfSupported(directoryPath, DIRECTORY_MODE);
}

function writeSecureFile(filePath, content) {
  ensureSecureDirectory(path.dirname(filePath));
  fs.writeFileSync(filePath, content, { mode: FILE_MODE });
  setPermissionIfSupported(filePath, FILE_MODE);
}

function normalizeSessionId(sessionId) {
  if (typeof sessionId !== 'string' || !SESSION_ID_PATTERN.test(sessionId)) {
    throw new Error(`Invalid session ID: ${sessionId}`);
  }
  return sessionId;
}

function redactString(value) {
  let redacted = value;
  for (const { pattern, replacement } of SENSITIVE_VALUE_PATTERNS) {
    redacted = redacted.replace(pattern, replacement);
  }
  return redacted;
}

function redactSensitive(value, depth = 0) {
  if (depth > MAX_REDACTION_DEPTH || value === null || value === undefined) return value;
  if (typeof value === 'string') return redactString(value);
  if (Array.isArray(value)) return value.map((item) => redactSensitive(item, depth + 1));
  if (typeof value !== 'object') return value;

  const redacted = {};
  for (const [key, entryValue] of Object.entries(value)) {
    redacted[key] = SENSITIVE_KEY_PATTERN.test(key)
      ? '[REDACTED]'
      : redactSensitive(entryValue, depth + 1);
  }
  return redacted;
}

/**
 * Default session directory
 */
export const DEFAULT_SESSION_DIR = path.join(os.homedir(), '.stateset', 'sessions');

/**
 * Session metadata schema
 */
const SESSION_VERSION = 1;

/**
 * SessionManager - Manages session persistence and history
 */
export class SessionManager {
  constructor(options = {}) {
    this.sessionDir = options.sessionDir || DEFAULT_SESSION_DIR;
    this.maxSessions = options.maxSessions || 100;
    this.maxAge = options.maxAge || 7 * 24 * 60 * 60 * 1000; // 7 days
    this._lastTimestampMs = 0;

    // Ensure session directory exists
    this.ensureDirectory();
  }

  /**
   * Ensure session directory exists
   */
  ensureDirectory() {
    ensureSecureDirectory(this.sessionDir);
  }

  /**
   * Generate a new session ID
   */
  generateId() {
    const timestamp = Date.now().toString(36);
    const random = crypto.randomBytes(4).toString('hex');
    return `${timestamp}-${random}`;
  }

  _nextIsoTimestamp() {
    let now = Date.now();
    if (now <= this._lastTimestampMs) now = this._lastTimestampMs + 1;
    this._lastTimestampMs = now;
    return new Date(now).toISOString();
  }

  /**
   * Create a new session
   */
  create(options = {}) {
    const id = this.generateId();
    const now = this._nextIsoTimestamp();
    const session = {
      id,
      version: SESSION_VERSION,
      createdAt: now,
      updatedAt: now,
      database: options.database || null,
      agent: options.agent || null,
      model: options.model || null,
      operations: [],
      context: options.context || {},
      metadata: {
        operationCount: 0,
        lastOperation: null,
        totalDuration: 0,
      },
    };

    this.save(session);
    return session;
  }

  /**
   * Get session file path
   */
  getPath(sessionId) {
    const safeSessionId = normalizeSessionId(sessionId);
    const basePath = path.resolve(this.sessionDir);
    const sessionPath = path.resolve(basePath, `${safeSessionId}.json`);
    const basePrefix = basePath.endsWith(path.sep) ? basePath : `${basePath}${path.sep}`;
    if (!sessionPath.startsWith(basePrefix)) {
      throw new Error(`Invalid session path: ${sessionId}`);
    }
    return sessionPath;
  }

  /**
   * Save session to disk
   */
  save(session) {
    if (!session?.id) {
      throw new Error('Session must include an id');
    }
    normalizeSessionId(session.id);
    session.updatedAt = this._nextIsoTimestamp();
    const filePath = this.getPath(session.id);
    writeSecureFile(filePath, JSON.stringify(session, null, 2));
    return session;
  }

  /**
   * Load session from disk
   */
  load(sessionId) {
    let filePath;
    try {
      filePath = this.getPath(sessionId);
    } catch {
      return null;
    }

    if (!fs.existsSync(filePath)) {
      return null;
    }

    try {
      const data = fs.readFileSync(filePath, 'utf-8');
      return JSON.parse(data);
    } catch (error) {
      console.error(`Failed to load session ${sessionId}:`, error.message);
      return null;
    }
  }

  /**
   * Check if session exists
   */
  exists(sessionId) {
    try {
      return fs.existsSync(this.getPath(sessionId));
    } catch {
      return false;
    }
  }

  /**
   * Delete a session
   */
  delete(sessionId) {
    let filePath;
    try {
      filePath = this.getPath(sessionId);
    } catch {
      return false;
    }
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
      return true;
    }
    return false;
  }

  /**
   * Add operation to session history
   */
  addOperation(sessionId, operation) {
    const session = this.load(sessionId);
    if (!session) return null;
    session.metadata = session.metadata || {
      operationCount: 0,
      lastOperation: null,
      totalDuration: 0,
    };

    const entry = {
      timestamp: new Date().toISOString(),
      request: redactSensitive(operation.request),
      response: redactSensitive(operation.response),
      toolCalls: redactSensitive(operation.toolCalls || []),
      duration: operation.duration || 0,
      success: operation.success !== false,
    };

    session.operations.push(entry);
    session.metadata.operationCount++;
    session.metadata.lastOperation = entry.timestamp;
    session.metadata.totalDuration += entry.duration;

    // Limit history size
    if (session.operations.length > 50) {
      session.operations = session.operations.slice(-50);
    }

    this.save(session);
    return session;
  }

  /**
   * Update session context
   */
  updateContext(sessionId, context) {
    const session = this.load(sessionId);
    if (!session) return null;

    session.context = { ...session.context, ...context };
    this.save(session);
    return session;
  }

  /**
   * List all sessions
   */
  list(options = {}) {
    const { limit = 20, sortBy = 'updatedAt', order = 'desc' } = options;

    const files = fs.readdirSync(this.sessionDir).filter((f) => f.endsWith('.json'));

    const sessions = files
      .map((f) => {
        try {
          const data = fs.readFileSync(path.join(this.sessionDir, f), 'utf-8');
          const session = JSON.parse(data);
          return {
            id: session.id,
            createdAt: session.createdAt,
            updatedAt: session.updatedAt,
            database: session.database,
            agent: session.agent,
            operationCount: session.metadata?.operationCount || 0,
            lastOperation: session.metadata?.lastOperation,
          };
        } catch (err) {
          console.debug('[session] Session file parse failed:', err.message || err);
          return null;
        }
      })
      .filter(Boolean);

    // Sort
    sessions.sort((a, b) => {
      const aVal = a[sortBy] || '';
      const bVal = b[sortBy] || '';
      return order === 'desc' ? bVal.localeCompare(aVal) : aVal.localeCompare(bVal);
    });

    return sessions.slice(0, limit);
  }

  /**
   * Find sessions by criteria
   */
  find(criteria = {}) {
    const sessions = this.list({ limit: this.maxSessions });

    return sessions.filter((s) => {
      if (criteria.database && s.database !== criteria.database) return false;
      if (criteria.agent && s.agent !== criteria.agent) return false;
      if (criteria.since && new Date(s.updatedAt) < new Date(criteria.since)) return false;
      return true;
    });
  }

  /**
   * Get most recent session
   */
  getRecent(criteria = {}) {
    const sessions = this.find(criteria);
    return sessions[0] || null;
  }

  /**
   * Clean up old sessions
   */
  cleanup(options = {}) {
    const maxAge = options.maxAge || this.maxAge;
    const maxCount = options.maxCount || this.maxSessions;
    const cutoff = Date.now() - maxAge;

    const sessions = this.list({ limit: 1000 });
    let deleted = 0;

    // Delete old sessions
    for (const session of sessions) {
      const updatedAt = new Date(session.updatedAt).getTime();
      if (updatedAt < cutoff) {
        this.delete(session.id);
        deleted++;
      }
    }

    // Delete excess sessions
    const remaining = this.list({ limit: 1000 });
    if (remaining.length > maxCount) {
      const toDelete = remaining.slice(maxCount);
      for (const session of toDelete) {
        this.delete(session.id);
        deleted++;
      }
    }

    return { deleted };
  }

  /**
   * Archive session to different location
   */
  archive(sessionId, archiveDir) {
    let safeSessionId;
    try {
      safeSessionId = normalizeSessionId(sessionId);
    } catch {
      return false;
    }

    const session = this.load(sessionId);
    if (!session) return false;

    const archivePath = path.join(archiveDir || path.join(this.sessionDir, 'archive'));
    ensureSecureDirectory(archivePath);

    const archiveFile = path.join(archivePath, `${safeSessionId}.json`);
    writeSecureFile(archiveFile, JSON.stringify(session, null, 2));
    this.delete(sessionId);

    return true;
  }

  /**
   * Export session as markdown report
   */
  exportMarkdown(sessionId) {
    const session = this.load(sessionId);
    if (!session) return null;

    let md = `# Session Report: ${session.id}\n\n`;
    md += `**Created:** ${session.createdAt}\n`;
    md += `**Updated:** ${session.updatedAt}\n`;
    md += `**Database:** ${session.database || 'default'}\n`;
    md += `**Agent:** ${session.agent || 'auto'}\n`;
    md += `**Operations:** ${session.metadata?.operationCount || 0}\n\n`;

    md += `## Operations\n\n`;

    for (const op of session.operations) {
      const requestText =
        typeof op.request === 'string' ? op.request : JSON.stringify(op.request || '');
      const responseText =
        typeof op.response === 'string' ? op.response : JSON.stringify(op.response || '');

      md += `### ${op.timestamp}\n\n`;
      md += `**Request:** ${requestText}\n\n`;

      if (op.toolCalls?.length > 0) {
        md += `**Tools:**\n`;
        for (const tool of op.toolCalls) {
          md += `- ${tool.name}\n`;
        }
        md += '\n';
      }

      md += `**Response:** ${responseText.slice(0, 200)}${responseText.length > 200 ? '...' : ''}\n\n`;
      md += `---\n\n`;
    }

    return md;
  }

  /**
   * Get session statistics
   */
  getStats() {
    const sessions = this.list({ limit: 1000 });

    const stats = {
      totalSessions: sessions.length,
      totalOperations: sessions.reduce((sum, s) => sum + (s.operationCount || 0), 0),
      byAgent: {},
      byDatabase: {},
      recentActivity: sessions.filter((s) => {
        const age = Date.now() - new Date(s.updatedAt).getTime();
        return age < 24 * 60 * 60 * 1000; // Last 24 hours
      }).length,
    };

    for (const session of sessions) {
      const agent = session.agent || 'auto';
      stats.byAgent[agent] = (stats.byAgent[agent] || 0) + 1;

      const db = session.database || 'default';
      stats.byDatabase[db] = (stats.byDatabase[db] || 0) + 1;
    }

    return stats;
  }
}

/**
 * Create a session manager
 */
export function createSessionManager(options = {}) {
  return new SessionManager(options);
}

/**
 * CommandHistory - Track command history across sessions
 */
export class CommandHistory {
  constructor(options = {}) {
    this.historyFile = options.historyFile || path.join(os.homedir(), '.stateset', 'history');
    this.maxEntries = options.maxEntries || 1000;

    this.ensureFile();
  }

  ensureFile() {
    const dir = path.dirname(this.historyFile);
    ensureSecureDirectory(dir);
    if (!fs.existsSync(this.historyFile)) {
      writeSecureFile(this.historyFile, '');
    } else {
      setPermissionIfSupported(this.historyFile, FILE_MODE);
    }
  }

  /**
   * Add command to history
   */
  add(command) {
    const safeCommand = redactString(String(command ?? ''));
    const entry = `${new Date().toISOString()}\t${safeCommand}\n`;
    fs.appendFileSync(this.historyFile, entry);
    setPermissionIfSupported(this.historyFile, FILE_MODE);
    this.trim();
  }

  /**
   * Get recent commands
   */
  getRecent(count = 20) {
    const content = fs.readFileSync(this.historyFile, 'utf-8');
    const lines = content.trim().split('\n').filter(Boolean);

    return lines
      .slice(-count)
      .map((line) => {
        const [timestamp, ...parts] = line.split('\t');
        return {
          timestamp,
          command: parts.join('\t'),
        };
      })
      .reverse();
  }

  /**
   * Search history
   */
  search(query, limit = 20) {
    const content = fs.readFileSync(this.historyFile, 'utf-8');
    const lines = content.trim().split('\n').filter(Boolean);
    const lowerQuery = query.toLowerCase();

    const matches = lines
      .filter((line) => line.toLowerCase().includes(lowerQuery))
      .slice(-limit)
      .map((line) => {
        const [timestamp, ...parts] = line.split('\t');
        return {
          timestamp,
          command: parts.join('\t'),
        };
      });

    return matches.reverse();
  }

  /**
   * Trim history to max entries
   */
  trim() {
    const content = fs.readFileSync(this.historyFile, 'utf-8');
    const lines = content.trim().split('\n').filter(Boolean);

    if (lines.length > this.maxEntries) {
      const trimmed = lines.slice(-this.maxEntries).join('\n') + '\n';
      writeSecureFile(this.historyFile, trimmed);
    }
  }

  /**
   * Clear history
   */
  clear() {
    writeSecureFile(this.historyFile, '');
  }
}

/**
 * Create a command history tracker
 */
export function createCommandHistory(options = {}) {
  return new CommandHistory(options);
}

export default {
  SessionManager,
  createSessionManager,
  CommandHistory,
  createCommandHistory,
  DEFAULT_SESSION_DIR,
};
