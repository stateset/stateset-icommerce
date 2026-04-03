/**
 * Treasury Store
 *
 * SQLite-backed ledger for agent funding and token purchases when available.
 * Falls back to a durable JSON ledger when the native SQLite binding is unavailable.
 */

import { createRequire } from 'node:module';
import {
  mkdirSync,
  existsSync,
  openSync,
  closeSync,
  readFileSync,
  writeFileSync,
  renameSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';

const SCHEMA = `
CREATE TABLE IF NOT EXISTS treasury_transactions (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id          TEXT    NOT NULL,
  agent_id          TEXT    NOT NULL,
  chain_id          TEXT    NOT NULL,
  token_symbol      TEXT    NOT NULL,
  token_address     TEXT,
  token_decimals    INTEGER,
  direction         TEXT    NOT NULL,
  amount_smallest   TEXT    NOT NULL,
  amount_display    TEXT    NOT NULL,
  price_usd         TEXT,
  related_event_id  TEXT,
  tx_id             TEXT,
  source            TEXT,
  metadata          TEXT,
  task_id           TEXT,
  session_id        TEXT,
  tool_name         TEXT,
  request_id        TEXT,
  created_at        INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_treasury_agent
  ON treasury_transactions(agent_id, chain_id, token_symbol, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_treasury_event
  ON treasury_transactions(event_id);
`;

const DIRECTION_SIGNS = {
  deposit: 1n,
  swap_in: 1n,
  adjust_in: 1n,
  withdraw: -1n,
  swap_out: -1n,
  fee: -1n,
  adjust_out: -1n,
};

const require = createRequire(import.meta.url);
const FALLBACK_TREASURY_DATABASES = new Map();
let cachedDatabaseCtor;

export function defaultTreasuryDir(cwd = process.cwd()) {
  return join(cwd, '.stateset', 'treasury');
}

export function defaultTreasuryDbPath(cwd = process.cwd()) {
  return join(defaultTreasuryDir(cwd), 'treasury.db');
}

function ensureDirForFile(filePath) {
  const dir = dirname(resolve(filePath));
  mkdirSync(dir, { recursive: true });
}

function ensureDbFile(dbPath) {
  if (dbPath === ':memory:') return;
  ensureDirForFile(dbPath);
  closeSync(openSync(dbPath, 'a'));
}

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

function getFallbackDatabaseState(dbPath) {
  const storagePath = dbPath === ':memory:' ? ':memory:' : `${dbPath}.fallback.json`;
  if (storagePath === ':memory:') {
    return { nextId: 1, rows: [], storagePath };
  }

  let state = FALLBACK_TREASURY_DATABASES.get(storagePath);
  if (!state) {
    let rows = [];
    let nextId = 1;

    if (existsSync(storagePath)) {
      try {
        const raw = readFileSync(storagePath, 'utf8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          if (Array.isArray(parsed?.rows)) {
            rows = parsed.rows;
          }
          if (Number.isInteger(parsed?.nextId) && parsed.nextId > 0) {
            nextId = parsed.nextId;
          } else if (rows.length > 0) {
            nextId = Math.max(...rows.map((row) => row.id || 0)) + 1;
          }
        }
      } catch (error) {
        console.warn(
          `[treasury-store] Failed to read fallback treasury store ${storagePath}: ${error.message}`,
        );
      }
    }

    state = { nextId, rows, storagePath };
    FALLBACK_TREASURY_DATABASES.set(storagePath, state);
  }
  return state;
}

function persistFallbackDatabaseState(state) {
  if (!state?.storagePath || state.storagePath === ':memory:') {
    return;
  }

  ensureDirForFile(state.storagePath);
  const tmpPath = `${state.storagePath}.tmp`;
  writeFileSync(
    tmpPath,
    JSON.stringify(
      {
        nextId: state.nextId,
        rows: state.rows,
      },
      null,
      2,
    ),
  );
  renameSync(tmpPath, state.storagePath);
}

/** @type {Record<string, string>} Allowed audit columns and their SQLite types */
const AUDIT_COLUMNS = {
  task_id: 'TEXT',
  session_id: 'TEXT',
  tool_name: 'TEXT',
  request_id: 'TEXT',
};

function ensureAuditColumns(db) {
  const columns = db.prepare('PRAGMA table_info(treasury_transactions)').all();
  const existing = new Set(columns.map((col) => col.name));

  for (const [name, type] of Object.entries(AUDIT_COLUMNS)) {
    if (!existing.has(name)) {
      // Safe: name and type come from hardcoded AUDIT_COLUMNS whitelist above
      db.exec(`ALTER TABLE treasury_transactions ADD COLUMN ${name} ${type}`);
    }
  }
}

export class TreasuryStore {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.dbPath]
   * @param {typeof import('better-sqlite3') | null} [opts.databaseCtor]
   */
  constructor(opts = {}) {
    this.dbPath = opts.dbPath || defaultTreasuryDbPath();
    this._databaseCtor = Object.prototype.hasOwnProperty.call(opts, 'databaseCtor')
      ? opts.databaseCtor
      : undefined;
    this.db = null;
    this._fallbackState = null;
    this.backend = 'sqlite';
    this._insertStmt = null;
    this._listStmt = null;
    this._listByAgentStmt = null;
    this._findByTxStmt = null;
  }

  init() {
    ensureDbFile(this.dbPath);

    const Database = this._databaseCtor === undefined ? loadDatabaseCtor() : this._databaseCtor;
    if (!Database) {
      this._enableFallback('better-sqlite3 unavailable');
      return;
    }

    try {
      this.db = new Database(this.dbPath);
    } catch (error) {
      if (error?.code !== 'ERR_DLOPEN_FAILED') {
        throw error;
      }
      this._enableFallback(error.message || 'native module load failure');
      return;
    }

    this.db.pragma('journal_mode = WAL');
    this.db.exec(SCHEMA);
    ensureAuditColumns(this.db);

    this._insertStmt = this.db.prepare(`
      INSERT INTO treasury_transactions (
        event_id, agent_id, chain_id, token_symbol, token_address, token_decimals,
        direction, amount_smallest, amount_display, price_usd, related_event_id,
        tx_id, source, metadata, task_id, session_id, tool_name, request_id, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    this._listStmt = this.db.prepare(`
      SELECT * FROM treasury_transactions
      WHERE agent_id = ?
        AND (? IS NULL OR chain_id = ?)
        AND (? IS NULL OR token_symbol = ?)
        AND (? IS NULL OR task_id = ?)
        AND (? IS NULL OR request_id = ?)
      ORDER BY created_at DESC, id DESC
      LIMIT ?
    `);

    this._listByAgentStmt = this.db.prepare(`
      SELECT chain_id, token_symbol, token_decimals, direction, amount_smallest
      FROM treasury_transactions
      WHERE agent_id = ?
        AND (? IS NULL OR chain_id = ?)
    `);

    this._findByTxStmt = this.db.prepare(`
      SELECT *
      FROM treasury_transactions
      WHERE agent_id = ?
        AND chain_id = ?
        AND token_symbol = ?
        AND direction = ?
        AND source = ?
        AND tx_id = ?
      ORDER BY created_at DESC, id DESC
      LIMIT 1
    `);
  }

  _enableFallback(reason = 'fallback requested') {
    this._fallbackState = getFallbackDatabaseState(this.dbPath);
    this.backend = 'json-fallback';
    if (this.dbPath !== ':memory:') {
      console.warn(
        `[treasury-store] ${reason}; using durable JSON fallback at ${this._fallbackState.storagePath}`,
      );
    }
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

  close() {
    if (this._fallbackState) {
      persistFallbackDatabaseState(this._fallbackState);
    }
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }

  /**
   * Record a transaction entry.
   * @param {Object} entry
   * @returns {Object}
   */
  record(entry) {
    if (!this.db) {
      this.init();
    }

    const payload = {
      ...entry,
      metadata: entry.metadata ? JSON.stringify(entry.metadata) : null,
      created_at: entry.created_at || Date.now(),
    };

    if (this._fallbackState) {
      this._fallbackState.rows.push({
        id: this._fallbackState.nextId++,
        ...payload,
      });
      persistFallbackDatabaseState(this._fallbackState);
      return payload;
    }

    this._insertStmt.run(
      payload.event_id,
      payload.agent_id,
      payload.chain_id,
      payload.token_symbol,
      payload.token_address || null,
      payload.token_decimals || null,
      payload.direction,
      payload.amount_smallest,
      payload.amount_display,
      payload.price_usd || null,
      payload.related_event_id || null,
      payload.tx_id || null,
      payload.source || null,
      payload.metadata,
      payload.task_id || null,
      payload.session_id || null,
      payload.tool_name || null,
      payload.request_id || null,
      payload.created_at,
    );

    return payload;
  }

  /**
   * List recent transactions for an agent.
   * @param {Object} query
   * @returns {Object[]}
   */
  list(query = {}) {
    if (!this.db) {
      this.init();
    }

    const {
      agentId,
      chainId = null,
      tokenSymbol = null,
      taskId = null,
      requestId = null,
      limit = 50,
    } = query;

    const rows = this._fallbackState
      ? this._fallbackState.rows
          .filter(
            (row) =>
              row.agent_id === agentId &&
              (chainId === null || row.chain_id === chainId) &&
              (tokenSymbol === null || row.token_symbol === tokenSymbol) &&
              (taskId === null || row.task_id === taskId) &&
              (requestId === null || row.request_id === requestId),
          )
          .sort((a, b) => b.created_at - a.created_at || b.id - a.id)
          .slice(0, limit)
      : this._listStmt.all(
          agentId,
          chainId,
          chainId,
          tokenSymbol,
          tokenSymbol,
          taskId,
          taskId,
          requestId,
          requestId,
          limit,
        );

    return rows.map((row) => ({
      ...row,
      metadata: row.metadata ? JSON.parse(row.metadata) : null,
    }));
  }

  /**
   * Find the latest transaction by tx hash plus ledger dimensions.
   * @param {Object} query
   * @returns {Object|null}
   */
  findByTx(query = {}) {
    if (!this.db) {
      this.init();
    }

    const { agentId, chainId, tokenSymbol, direction, source, txId } = query;
    if (!agentId || !chainId || !tokenSymbol || !direction || !source || !txId) {
      return null;
    }

    const row = this._fallbackState
      ? this._fallbackState.rows
          .filter(
            (entry) =>
              entry.agent_id === agentId &&
              entry.chain_id === chainId &&
              entry.token_symbol === tokenSymbol &&
              entry.direction === direction &&
              entry.source === source &&
              entry.tx_id === txId,
          )
          .sort((a, b) => b.created_at - a.created_at || b.id - a.id)[0]
      : this._findByTxStmt.get(agentId, chainId, tokenSymbol, direction, source, txId);
    if (!row) return null;

    return {
      ...row,
      metadata: row.metadata ? JSON.parse(row.metadata) : null,
    };
  }

  /**
   * Compute balances for an agent.
   * @param {Object} query
   * @returns {Object[]}
   */
  getBalances(query = {}) {
    if (!this.db) {
      this.init();
    }

    const { agentId, chainId = null } = query;
    const rows = this._fallbackState
      ? this._fallbackState.rows
          .filter(
            (row) =>
              row.agent_id === agentId && (chainId === null || row.chain_id === chainId),
          )
          .map((row) => ({
            chain_id: row.chain_id,
            token_symbol: row.token_symbol,
            token_decimals: row.token_decimals,
            direction: row.direction,
            amount_smallest: row.amount_smallest,
          }))
      : this._listByAgentStmt.all(agentId, chainId, chainId);

    const balances = new Map();

    for (const row of rows) {
      const key = `${row.chain_id}:${row.token_symbol}`;
      const sign = DIRECTION_SIGNS[row.direction] ?? 0n;
      const amount = BigInt(row.amount_smallest);
      const current = balances.get(key) || {
        chainId: row.chain_id,
        tokenSymbol: row.token_symbol,
        tokenDecimals: row.token_decimals,
        balanceSmallest: 0n,
      };

      current.balanceSmallest += amount * sign;
      balances.set(key, current);
    }

    return Array.from(balances.values());
  }

  /**
   * Compute a single token balance for an agent.
   * @param {Object} query
   * @returns {Object}
   */
  getBalance(query = {}) {
    const { agentId, chainId, tokenSymbol, tokenDecimals = null } = query;
    const balances = this.getBalances({ agentId, chainId });
    const match = balances.find((b) => b.tokenSymbol === tokenSymbol);
    if (!match) {
      return {
        agentId,
        chainId,
        tokenSymbol,
        tokenDecimals,
        balanceSmallest: 0n,
      };
    }

    return {
      agentId,
      chainId,
      tokenSymbol,
      tokenDecimals: match.tokenDecimals ?? tokenDecimals,
      balanceSmallest: match.balanceSmallest,
    };
  }
}

export default TreasuryStore;
