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

export const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'agent-sessions.db');

const SESSION_COLUMNS = [
  { name: 'session_id', sql: 'TEXT PRIMARY KEY' },
  { name: 'provider', sql: 'TEXT' },
  { name: 'model', sql: 'TEXT' },
  { name: 'think_level', sql: 'TEXT' },
  { name: 'sla_level', sql: 'TEXT' },
  { name: 'agent', sql: 'TEXT' },
  { name: 'summaries', sql: 'TEXT' },
  { name: 'last_request', sql: 'TEXT' },
  { name: 'last_response', sql: 'TEXT' },
  { name: 'prompt_report', sql: 'TEXT' },
  { name: 'session_refresh', sql: 'TEXT' },
  { name: 'last_error', sql: 'TEXT' },
  { name: 'last_error_code', sql: 'TEXT' },
  { name: 'last_error_at', sql: 'INTEGER' },
  { name: 'aborted_last_run', sql: 'INTEGER' },
  { name: 'last_run_ms', sql: 'INTEGER' },
  { name: 'last_cost_usd', sql: 'REAL' },
  { name: 'total_cost_usd', sql: 'REAL' },
  { name: 'input_tokens', sql: 'INTEGER' },
  { name: 'output_tokens', sql: 'INTEGER' },
  { name: 'total_tokens', sql: 'INTEGER' },
  { name: 'cache_read_tokens', sql: 'INTEGER' },
  { name: 'cache_write_tokens', sql: 'INTEGER' },
  { name: 'compaction_count', sql: 'INTEGER' },
  { name: 'created_at', sql: 'INTEGER NOT NULL' },
  { name: 'updated_at', sql: 'INTEGER NOT NULL' },
];

const UPSERT_COLUMNS = SESSION_COLUMNS.map((column) => column.name);
const UPSERT_ASSIGNMENTS = UPSERT_COLUMNS.filter(
  (column) => column !== 'session_id' && column !== 'created_at',
)
  .map((column) => `${column} = excluded.${column}`)
  .join(',\n        ');

export class AgentSessionStore {
  constructor({ dbPath = DEFAULT_DB_PATH, maxSummaries = 5 } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');
    this.maxSummaries = maxSummaries;

    this.db.exec(`
      CREATE TABLE IF NOT EXISTS agent_sessions (
        ${SESSION_COLUMNS.map((column) => `${column.name} ${column.sql}`).join(',\n        ')}
      );

      CREATE INDEX IF NOT EXISTS idx_agent_sessions_updated
        ON agent_sessions(updated_at DESC);
    `);

    this._ensureSchema();

    this._get = this.db.prepare(`SELECT * FROM agent_sessions WHERE session_id = ?`);
    this._count = this.db.prepare(`SELECT COUNT(*) AS count FROM agent_sessions`);
    this._listRecent = this.db.prepare(
      `SELECT * FROM agent_sessions ORDER BY updated_at DESC LIMIT ?`,
    );
    this._listRecentFailures = this.db.prepare(
      `SELECT *
         FROM agent_sessions
        WHERE last_error IS NOT NULL OR last_error_code IS NOT NULL OR aborted_last_run = 1
        ORDER BY updated_at DESC
        LIMIT ?`,
    );
    this._upsert = this.db.prepare(
      `INSERT INTO agent_sessions (${UPSERT_COLUMNS.join(', ')})
       VALUES (${UPSERT_COLUMNS.map(() => '?').join(', ')})
       ON CONFLICT(session_id) DO UPDATE SET
        ${UPSERT_ASSIGNMENTS}`,
    );
    this._delete = this.db.prepare(`DELETE FROM agent_sessions WHERE session_id = ?`);
  }

  _ensureSchema() {
    const existing = new Set(
      this.db
        .prepare(`PRAGMA table_info(agent_sessions)`)
        .all()
        .map((column) => column.name),
    );
    for (const column of SESSION_COLUMNS) {
      if (existing.has(column.name) || column.name === 'session_id') continue;
      this.db.exec(`ALTER TABLE agent_sessions ADD COLUMN ${column.name} ${column.sql}`);
    }
  }

  _hydrateRow(row) {
    if (!row) return null;
    const inputTokens = normalizeInteger(row.input_tokens);
    const outputTokens = normalizeInteger(row.output_tokens);
    return {
      sessionId: row.session_id,
      provider: row.provider,
      model: row.model,
      thinkLevel: row.think_level,
      slaLevel: row.sla_level,
      agent: row.agent,
      summaries: row.summaries ? safeJsonParse(row.summaries, []) : [],
      lastRequest: row.last_request,
      lastResponse: row.last_response,
      promptReport: row.prompt_report ? safeJsonParse(row.prompt_report, null) : null,
      sessionRefresh: row.session_refresh ? safeJsonParse(row.session_refresh, null) : null,
      lastError: row.last_error,
      lastErrorCode: row.last_error_code,
      lastErrorAt: normalizeInteger(row.last_error_at),
      abortedLastRun:
        row.aborted_last_run === null || row.aborted_last_run === undefined
          ? null
          : Boolean(row.aborted_last_run),
      lastRunMs: normalizeInteger(row.last_run_ms),
      lastCostUsd: normalizeNumber(row.last_cost_usd),
      totalCostUsd: normalizeNumber(row.total_cost_usd),
      inputTokens,
      outputTokens,
      totalTokens:
        normalizeInteger(row.total_tokens) ?? computeTotalTokens(inputTokens, outputTokens),
      cacheReadTokens: normalizeInteger(row.cache_read_tokens),
      cacheWriteTokens: normalizeInteger(row.cache_write_tokens),
      compactionCount: normalizeInteger(row.compaction_count) ?? 0,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  get(sessionId) {
    if (!sessionId) return null;
    return this._hydrateRow(this._get.get(sessionId));
  }

  count() {
    return this._count.get().count;
  }

  listRecent(limit = 5) {
    const safeLimit = normalizePositiveLimit(limit);
    return this._listRecent.all(safeLimit).map((row) => this._hydrateRow(row));
  }

  listRecentFailures(limit = 5) {
    const safeLimit = normalizePositiveLimit(limit);
    return this._listRecentFailures.all(safeLimit).map((row) => this._hydrateRow(row));
  }

  upsert(sessionId, data = {}) {
    if (!sessionId) return null;
    const existing = this.get(sessionId);
    const createdAt = existing?.createdAt || Date.now();
    const summaries = hasOwn(data, 'summaries')
      ? normalizeSummaries(data.summaries)
      : normalizeSummaries(existing?.summaries);
    const inputTokens = hasOwn(data, 'inputTokens')
      ? normalizeInteger(data.inputTokens)
      : normalizeInteger(existing?.inputTokens);
    const outputTokens = hasOwn(data, 'outputTokens')
      ? normalizeInteger(data.outputTokens)
      : normalizeInteger(existing?.outputTokens);
    const totalTokens = hasOwn(data, 'totalTokens')
      ? normalizeInteger(data.totalTokens)
      : (computeTotalTokens(inputTokens, outputTokens) ?? normalizeInteger(existing?.totalTokens));

    this._upsert.run(
      sessionId,
      resolveValue(data, 'provider', existing?.provider),
      resolveValue(data, 'model', existing?.model),
      resolveValue(data, 'thinkLevel', existing?.thinkLevel),
      resolveValue(data, 'slaLevel', existing?.slaLevel),
      resolveValue(data, 'agent', existing?.agent),
      JSON.stringify(summaries),
      resolveValue(data, 'lastRequest', existing?.lastRequest),
      resolveValue(data, 'lastResponse', existing?.lastResponse),
      resolveJson(data, 'promptReport', existing?.promptReport),
      resolveJson(data, 'sessionRefresh', existing?.sessionRefresh),
      resolveValue(data, 'lastError', existing?.lastError),
      resolveValue(data, 'lastErrorCode', existing?.lastErrorCode),
      resolveInteger(data, 'lastErrorAt', existing?.lastErrorAt),
      resolveBoolean(data, 'abortedLastRun', existing?.abortedLastRun),
      resolveInteger(data, 'lastRunMs', existing?.lastRunMs),
      resolveNumber(data, 'lastCostUsd', existing?.lastCostUsd),
      resolveNumber(data, 'totalCostUsd', existing?.totalCostUsd),
      inputTokens,
      outputTokens,
      totalTokens,
      resolveInteger(data, 'cacheReadTokens', existing?.cacheReadTokens),
      resolveInteger(data, 'cacheWriteTokens', existing?.cacheWriteTokens),
      resolveInteger(data, 'compactionCount', existing?.compactionCount) ?? 0,
      createdAt,
      Date.now(),
    );

    return this.get(sessionId);
  }

  recordRun(sessionId, data = {}) {
    if (!sessionId) return null;
    const existing = this.get(sessionId);
    const payload = { ...data };
    const runCost = normalizeNumber(payload.lastCostUsd);

    if (hasOwn(payload, 'lastCostUsd')) {
      payload.lastCostUsd = runCost;
    }

    if (!hasOwn(payload, 'totalCostUsd') && runCost !== null) {
      payload.totalCostUsd = (existing?.totalCostUsd || 0) + runCost;
    }

    if (hasOwn(payload, 'compactionCount')) {
      payload.compactionCount =
        (existing?.compactionCount || 0) + (normalizeInteger(payload.compactionCount) || 0);
    }

    return this.upsert(sessionId, payload);
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

function hasOwn(value, key) {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function resolveValue(data, key, fallback = null) {
  if (hasOwn(data, key)) return data[key] ?? null;
  return fallback ?? null;
}

function resolveNumber(data, key, fallback = null) {
  if (hasOwn(data, key)) return normalizeNumber(data[key]);
  return normalizeNumber(fallback);
}

function resolveInteger(data, key, fallback = null) {
  if (hasOwn(data, key)) return normalizeInteger(data[key]);
  return normalizeInteger(fallback);
}

function resolveBoolean(data, key, fallback = null) {
  const value = hasOwn(data, key) ? data[key] : fallback;
  if (value === null || value === undefined) return null;
  return value ? 1 : 0;
}

function resolveJson(data, key, fallback = null) {
  const value = hasOwn(data, key) ? data[key] : fallback;
  return serializeJson(value);
}

function normalizeSummaries(value) {
  return Array.isArray(value) ? value : [];
}

function normalizeNumber(value) {
  if (value === null || value === undefined || value === '') return null;
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : null;
}

function normalizeInteger(value) {
  if (value === null || value === undefined || value === '') return null;
  const numeric = Number(value);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : null;
}

function normalizePositiveLimit(value, fallback = 5) {
  const numeric = normalizeInteger(value);
  return numeric && numeric > 0 ? numeric : fallback;
}

function serializeJson(value) {
  if (value === null || value === undefined) return null;
  return JSON.stringify(value);
}

function computeTotalTokens(inputTokens, outputTokens) {
  if (inputTokens === null || outputTokens === null) return null;
  return inputTokens + outputTokens;
}

function safeJsonParse(value, fallback) {
  try {
    return JSON.parse(value);
  } catch (err) {
    console.debug('[agent-session-store] JSON parse failed:', err.message || err);
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
    try {
      _store.close();
    } catch (err) {
      console.warn('[agent-session-store] Store close error:', err.message);
    }
  }
  _store = null;
}
