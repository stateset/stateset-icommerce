/**
 * Agent Session Store for StateSet Harness
 *
 * Persists model/provider/think-level metadata per Claude session ID.
 * Also stores recent summaries to keep context stable across runs.
 */

import Database from 'better-sqlite3';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'agent-sessions.db');

export class AgentSessionStore {
  constructor({ dbPath = DEFAULT_DB_PATH, maxSummaries = 5 } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');
    this.maxSummaries = maxSummaries;

    this.db.exec(`
      CREATE TABLE IF NOT EXISTS agent_sessions (
        session_id   TEXT PRIMARY KEY,
        provider     TEXT,
        model        TEXT,
        think_level  TEXT,
        agent        TEXT,
        summaries    TEXT,
        last_request TEXT,
        last_response TEXT,
        created_at   INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL
      );

      CREATE INDEX IF NOT EXISTS idx_agent_sessions_updated
        ON agent_sessions(updated_at DESC);
    `);

    this._get = this.db.prepare(
      `SELECT * FROM agent_sessions WHERE session_id = ?`
    );
    this._upsert = this.db.prepare(
      `INSERT INTO agent_sessions
        (session_id, provider, model, think_level, agent, summaries, last_request, last_response, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(session_id) DO UPDATE SET
        provider = excluded.provider,
        model = excluded.model,
        think_level = excluded.think_level,
        agent = excluded.agent,
        summaries = excluded.summaries,
        last_request = excluded.last_request,
        last_response = excluded.last_response,
        updated_at = excluded.updated_at`
    );
    this._delete = this.db.prepare(
      `DELETE FROM agent_sessions WHERE session_id = ?`
    );
  }

  get(sessionId) {
    if (!sessionId) return null;
    const row = this._get.get(sessionId);
    if (!row) return null;
    return {
      sessionId: row.session_id,
      provider: row.provider,
      model: row.model,
      thinkLevel: row.think_level,
      agent: row.agent,
      summaries: row.summaries ? safeJsonParse(row.summaries, []) : [],
      lastRequest: row.last_request,
      lastResponse: row.last_response,
      createdAt: row.created_at,
      updatedAt: row.updated_at
    };
  }

  upsert(sessionId, data = {}) {
    if (!sessionId) return null;
    const existing = this.get(sessionId);
    const createdAt = existing?.createdAt || Date.now();
    const summaries = data.summaries ?? existing?.summaries ?? [];

    this._upsert.run(
      sessionId,
      data.provider ?? existing?.provider ?? null,
      data.model ?? existing?.model ?? null,
      data.thinkLevel ?? existing?.thinkLevel ?? null,
      data.agent ?? existing?.agent ?? null,
      JSON.stringify(summaries),
      data.lastRequest ?? existing?.lastRequest ?? null,
      data.lastResponse ?? existing?.lastResponse ?? null,
      createdAt,
      Date.now()
    );

    return this.get(sessionId);
  }

  appendSummary(sessionId, summary, maxSummaries = this.maxSummaries) {
    if (!sessionId || !summary) return null;
    const existing = this.get(sessionId);
    const summaries = Array.isArray(existing?.summaries) ? existing.summaries.slice() : [];
    summaries.unshift(summary);
    if (summaries.length > maxSummaries) {
      summaries.length = maxSummaries;
    }
    return this.upsert(sessionId, { summaries });
  }

  delete(sessionId) {
    return this._delete.run(sessionId).changes > 0;
  }

  close() {
    this.db.close();
  }
}

function safeJsonParse(value, fallback) {
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

let _store = null;

export function getAgentSessionStore(options = {}) {
  if (!_store) {
    _store = new AgentSessionStore(options);
  }
  return _store;
}

export function resetAgentSessionStore() {
  if (_store) {
    try { _store.close(); } catch { /* ignore */ }
  }
  _store = null;
}
