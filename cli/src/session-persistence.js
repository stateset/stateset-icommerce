/**
 * Session Persistence for StateSet MCP Server
 * Saves and restores conversation context for interrupted sessions
 */

import fs from 'fs/promises';
import path from 'path';

const DIRECTORY_MODE = 0o700;
const FILE_MODE = 0o600;
const SESSION_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;

function validateSessionId(sessionId) {
  if (typeof sessionId !== 'string' || !SESSION_ID_PATTERN.test(sessionId)) {
    throw new Error(`Invalid session id: ${sessionId}`);
  }
  return sessionId;
}

async function chmodIfSupported(targetPath, mode) {
  try {
    await fs.chmod(targetPath, mode);
  } catch (chmodErr) {
    console.debug('chmod not supported on this platform:', chmodErr.message);
  }
}

async function ensureSecureDirectory(directoryPath) {
  await fs.mkdir(directoryPath, { recursive: true, mode: DIRECTORY_MODE });
  await chmodIfSupported(directoryPath, DIRECTORY_MODE);
}

async function writeSecureFile(filePath, content) {
  await ensureSecureDirectory(path.dirname(filePath));
  await fs.writeFile(filePath, content, { mode: FILE_MODE });
  await chmodIfSupported(filePath, FILE_MODE);
}

export class SessionPersistence {
  constructor(options = {}) {
    this.sessionDir = options.sessionDir || '.stateset/sessions';
    this.maxSessions = options.maxSessions || 100;
    this.sessionTtl = options.sessionTtl || 24 * 60 * 60 * 1000; // 24 hours
    this.sessions = new Map();
    this.initialized = false;
  }

  async initialize() {
    if (this.initialized) return;

    try {
      await ensureSecureDirectory(this.sessionDir);

      const files = await fs.readdir(this.sessionDir);
      const now = Date.now();

      for (const file of files) {
        if (file.endsWith('.json')) {
          try {
            const filePath = path.join(this.sessionDir, file);
            const content = await fs.readFile(filePath, 'utf-8');
            const session = JSON.parse(content);

            if (now - session.lastAccessedAt > this.sessionTtl) {
              await fs.unlink(filePath);
            } else if (session?.id && SESSION_ID_PATTERN.test(session.id)) {
              this.sessions.set(session.id, session);
            } else {
              await fs.unlink(filePath);
            }
          } catch (error) {
            console.warn(`Failed to load session from ${file}:`, error.message);
          }
        }
      }

      this.initialized = true;
    } catch (error) {
      console.error('Failed to initialize session persistence:', error);
      throw error;
    }
  }

  async saveSession(session) {
    await this.initialize();
    const sessionId = validateSessionId(session?.id);

    const sessionData = {
      ...session,
      id: sessionId,
      lastAccessedAt: Date.now(),
      persistedAt: Date.now(),
    };

    this.sessions.set(sessionId, sessionData);

    const filePath = path.join(this.sessionDir, `${sessionId}.json`);
    await writeSecureFile(filePath, JSON.stringify(sessionData, null, 2));

    await this.cleanupOldSessions();

    return sessionData;
  }

  async getSession(sessionId) {
    await this.initialize();
    let safeSessionId;
    try {
      safeSessionId = validateSessionId(sessionId);
    } catch {
      return null;
    }

    const session = this.sessions.get(safeSessionId);
    if (!session) {
      return null;
    }

    const now = Date.now();
    if (now - session.lastAccessedAt > this.sessionTtl) {
      await this.deleteSession(safeSessionId);
      return null;
    }

    session.lastAccessedAt = now;
    this.sessions.set(safeSessionId, session);

    return session;
  }

  async deleteSession(sessionId) {
    await this.initialize();
    let safeSessionId;
    try {
      safeSessionId = validateSessionId(sessionId);
    } catch {
      return;
    }

    this.sessions.delete(safeSessionId);

    const filePath = path.join(this.sessionDir, `${safeSessionId}.json`);
    try {
      await fs.unlink(filePath);
    } catch (error) {
      if (error.code !== 'ENOENT') {
        console.warn(`Failed to delete session ${sessionId}:`, error.message);
      }
    }
  }

  async listSessions(options = {}) {
    await this.initialize();

    const sessions = Array.from(this.sessions.values()).sort(
      (a, b) => b.lastAccessedAt - a.lastAccessedAt,
    );

    if (options.limit) {
      sessions.length = Math.min(sessions.length, options.limit);
    }

    return sessions.map((s) => ({
      id: s.id,
      createdAt: s.createdAt,
      lastAccessedAt: s.lastAccessedAt,
      operationCount: s.operations?.length || 0,
      status: s.status || 'active',
      lastOperation: s.operations?.[s.operations.length - 1],
    }));
  }

  async cleanupOldSessions() {
    const now = Date.now();
    const expired = [];

    for (const [sessionId, session] of this.sessions.entries()) {
      if (now - session.lastAccessedAt > this.sessionTtl) {
        expired.push(sessionId);
      }
    }

    for (const sessionId of expired) {
      await this.deleteSession(sessionId);
    }

    const overflow = this.sessions.size - this.maxSessions;
    if (overflow <= 0) {
      return;
    }

    const oldestActiveSessions = Array.from(this.sessions.values())
      .sort((a, b) => a.lastAccessedAt - b.lastAccessedAt)
      .slice(0, overflow);

    for (const session of oldestActiveSessions) {
      await this.deleteSession(session.id);
    }
  }

  async restoreSession(sessionId) {
    const session = await this.getSession(sessionId);
    if (!session) {
      throw new Error(`Session ${sessionId} not found or expired`);
    }

    return {
      session,
      canResume: true,
      context: {
        operations: session.operations || [],
        state: session.state,
        metadata: session.metadata,
      },
      nextSteps: this.suggestNextSteps(session),
    };
  }

  suggestNextSteps(session) {
    const operations = session.operations || [];
    if (operations.length === 0) {
      return [{ action: 'start_fresh', description: 'Start a new operation' }];
    }

    const lastOp = operations[operations.length - 1];
    const suggestions = [];

    if (lastOp.status === 'failed') {
      suggestions.push({
        action: 'retry_last_operation',
        description: `Retry the failed operation: ${lastOp.tool}`,
        context: lastOp,
      });
    }

    if (lastOp.status === 'success' && lastOp.tool === 'create_order') {
      suggestions.push({
        action: 'reserve_inventory',
        description: 'Reserve inventory for the created order',
        context: lastOp.result,
      });
    }

    if (lastOp.status === 'success' && lastOp.tool === 'reserve_inventory') {
      suggestions.push({
        action: 'confirm_reservation',
        description: 'Confirm the inventory reservation',
        context: lastOp.result,
      });
    }

    if (session.state?.pendingRollback) {
      suggestions.push({
        action: 'execute_rollback',
        description: 'Rollback the failed transaction',
        context: session.state.pendingRollback,
      });
    }

    suggestions.push({
      action: 'continue_new_operation',
      description: 'Start a new operation with existing context',
    });

    return suggestions;
  }

  async exportSession(sessionId) {
    const session = await this.getSession(sessionId);
    if (!session) {
      throw new Error(`Session ${sessionId} not found`);
    }

    return {
      id: session.id,
      createdAt: session.createdAt,
      lastAccessedAt: session.lastAccessedAt,
      operations: session.operations || [],
      state: session.state,
      metadata: session.metadata,
      exportTimestamp: Date.now(),
    };
  }

  async importSession(sessionData) {
    if (!sessionData?.id || !sessionData.operations) {
      throw new Error('Invalid session data: missing id or operations');
    }
    validateSessionId(sessionData.id);

    const session = {
      id: sessionData.id,
      createdAt: sessionData.createdAt || Date.now(),
      lastAccessedAt: Date.now(),
      operations: sessionData.operations,
      state: sessionData.state || {},
      metadata: sessionData.metadata || {},
      status: 'imported',
    };

    return this.saveSession(session);
  }

  async createAuditTrail(sessionId) {
    const session = await this.getSession(sessionId);
    if (!session) {
      throw new Error(`Session ${sessionId} not found`);
    }

    const auditTrail = {
      sessionId: session.id,
      sessionDuration: session.lastAccessedAt - session.createdAt,
      totalOperations: session.operations?.length || 0,
      successfulOperations: session.operations?.filter((op) => op.status === 'success').length || 0,
      failedOperations: session.operations?.filter((op) => op.status === 'failed').length || 0,
      operations:
        session.operations?.map((op) => ({
          timestamp: op.timestamp,
          tool: op.tool,
          status: op.status,
          params: op.params,
          error: op.error,
        })) || [],
      generatedAt: Date.now(),
    };

    return auditTrail;
  }
}

export default SessionPersistence;
