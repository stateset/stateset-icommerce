/**
 * Treasury Store
 *
 * SQLite-backed ledger for agent funding and token purchases.
 */

import Database from 'better-sqlite3';
import { mkdirSync } from 'node:fs';
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
   */
  constructor(opts = {}) {
    this.dbPath = opts.dbPath || defaultTreasuryDbPath();
    this.db = null;
    this._insertStmt = null;
    this._listStmt = null;
    this._listByAgentStmt = null;
    this._findByTxStmt = null;
  }

  init() {
    ensureDirForFile(this.dbPath);
    this.db = new Database(this.dbPath);
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

  close() {
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

    const rows = this._listStmt.all(
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

    const row = this._findByTxStmt.get(agentId, chainId, tokenSymbol, direction, source, txId);
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
    const rows = this._listByAgentStmt.all(agentId, chainId, chainId);

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
