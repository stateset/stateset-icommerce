/**
 * Agent Session Store for StateSet Harness
 *
 * Persists model/provider/think-level metadata per Claude session ID.
 * Also stores recent summaries to keep context stable across runs.
 *
 * Uses better-sqlite3 when available, and falls back to an in-process store
 * when the native module is unavailable.
 */

import { createRequire } from 'node:module';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';

export const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'agent-sessions.db');
const require = createRequire(import.meta.url);
const FALLBACK_SESSION_DATABASES = new Map();
let cachedDatabaseCtor;

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

function loadDatabaseCtor() {
  if (cachedDatabaseCtor !== undefined) {
    return cachedDatabaseCtor;
  }

  try {
    const mod = require('better-sqlite3');
    cachedDatabaseCtor = mod.default || mod;
  } catch (error) {
    if (error?.code !== 'ERR_DLOPEN_FAILED' && error?.code !== 'MODULE_NOT_FOUND') {
      throw error;
    }
    cachedDatabaseCtor = null;
  }

  return cachedDatabaseCtor;
}

function ensureDbFile(dbPath) {
  if (dbPath === ':memory:') return;
  fs.mkdirSync(path.dirname(dbPath), { recursive: true });
  const fd = fs.openSync(dbPath, 'a');
  fs.closeSync(fd);
}

function getFallbackDatabaseState(dbPath) {
  if (dbPath === ':memory:') {
    return { rows: new Map() };
  }

  let state = FALLBACK_SESSION_DATABASES.get(dbPath);
  if (!state || !fs.existsSync(dbPath)) {
    ensureDbFile(dbPath);
    state = { rows: new Map() };
    FALLBACK_SESSION_DATABASES.set(dbPath, state);
  }
  return state;
}

function sortRowsByRecency(rows) {
  return [...rows].sort(
    (a, b) =>
      b.updated_at - a.updated_at ||
      b.created_at - a.created_at ||
      String(b.session_id).localeCompare(String(a.session_id)),
  );
}

export class AgentSessionStore {
  constructor({ dbPath = DEFAULT_DB_PATH, maxSummaries = 5 } = {}) {
    if (dbPath !== ':memory:') {
      fs.mkdirSync(path.dirname(dbPath), { recursive: true });
    }
    ensureDbFile(dbPath);

    this._dbPath = dbPath;
    this.maxSummaries = maxSummaries;
    this._fallbackState = null;

    const Database = loadDatabaseCtor();
    if (!Database) {
      this._enableFallback();
      return;
    }

    try {
      this.db = new Database(dbPath);
    } catch (error) {
      if (error?.code !== 'ERR_DLOPEN_FAILED') {
        throw error;
      }
      this._enableFallback();
      return;
    }
    this.db.pragma('journal_mode = WAL');

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

  _enableFallback() {
    this._fallbackState = getFallbackDatabaseState(this._dbPath);
    this.db = {
      pragma() {
        return 'WAL';
      },
      exec() {
        return this;
      },
      close() {},
    };
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
    if (this._fallbackState) {
      return this._hydrateRow(this._fallbackState.rows.get(sessionId) || null);
    }
    return this._hydrateRow(this._get.get(sessionId));
  }

  count() {
    if (this._fallbackState) {
      return this._fallbackState.rows.size;
    }
    return this._count.get().count;
  }

  listRecent(limit = 5) {
    const safeLimit = normalizePositiveLimit(limit);
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackState.rows.values())
        .slice(0, safeLimit)
        .map((row) => this._hydrateRow(row));
    }
    return this._listRecent.all(safeLimit).map((row) => this._hydrateRow(row));
  }

  listRecentFailures(limit = 5) {
    const safeLimit = normalizePositiveLimit(limit);
    if (this._fallbackState) {
      return sortRowsByRecency(this._fallbackState.rows.values())
        .filter(
          (row) =>
            row.last_error !== null || row.last_error_code !== null || row.aborted_last_run === 1,
        )
        .slice(0, safeLimit)
        .map((row) => this._hydrateRow(row));
    }
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
    const row = {
      session_id: sessionId,
      provider: resolveValue(data, 'provider', existing?.provider),
      model: resolveValue(data, 'model', existing?.model),
      think_level: resolveValue(data, 'thinkLevel', existing?.thinkLevel),
      sla_level: resolveValue(data, 'slaLevel', existing?.slaLevel),
      agent: resolveValue(data, 'agent', existing?.agent),
      summaries: JSON.stringify(summaries),
      last_request: resolveValue(data, 'lastRequest', existing?.lastRequest),
      last_response: resolveValue(data, 'lastResponse', existing?.lastResponse),
      prompt_report: resolveJson(data, 'promptReport', existing?.promptReport),
      session_refresh: resolveJson(data, 'sessionRefresh', existing?.sessionRefresh),
      last_error: resolveValue(data, 'lastError', existing?.lastError),
      last_error_code: resolveValue(data, 'lastErrorCode', existing?.lastErrorCode),
      last_error_at: resolveInteger(data, 'lastErrorAt', existing?.lastErrorAt),
      aborted_last_run: resolveBoolean(data, 'abortedLastRun', existing?.abortedLastRun),
      last_run_ms: resolveInteger(data, 'lastRunMs', existing?.lastRunMs),
      last_cost_usd: resolveNumber(data, 'lastCostUsd', existing?.lastCostUsd),
      total_cost_usd: resolveNumber(data, 'totalCostUsd', existing?.totalCostUsd),
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      total_tokens: totalTokens,
      cache_read_tokens: resolveInteger(data, 'cacheReadTokens', existing?.cacheReadTokens),
      cache_write_tokens: resolveInteger(data, 'cacheWriteTokens', existing?.cacheWriteTokens),
      compaction_count: resolveInteger(data, 'compactionCount', existing?.compactionCount) ?? 0,
      created_at: createdAt,
      updated_at: Date.now(),
    };

    if (this._fallbackState) {
      this._fallbackState.rows.set(sessionId, row);
      return this._hydrateRow(row);
    }

    this._upsert.run(
      row.session_id,
      row.provider,
      row.model,
      row.think_level,
      row.sla_level,
      row.agent,
      row.summaries,
      row.last_request,
      row.last_response,
      row.prompt_report,
      row.session_refresh,
      row.last_error,
      row.last_error_code,
      row.last_error_at,
      row.aborted_last_run,
      row.last_run_ms,
      row.last_cost_usd,
      row.total_cost_usd,
      row.input_tokens,
      row.output_tokens,
      row.total_tokens,
      row.cache_read_tokens,
      row.cache_write_tokens,
      row.compaction_count,
      row.created_at,
      row.updated_at,
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
    if (this._fallbackState) {
      return this._fallbackState.rows.delete(sessionId);
    }
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
