/**
 * A2A Commerce Store (SQLite-backed)
 *
 * Persistent storage for A2A payments, payment requests, quotes,
 * escrows, disputes, feedback, reputation, and services.
 */

import path from 'node:path';
import os from 'node:os';

import { UPDATABLE_COLUMNS } from './store/columns.js';
import { A2A_SCHEMA, A2ASchemaMigrations } from './store/schema.js';
import { loadDatabaseCtor, createSqliteUnavailableError } from './store/sqlite.js';
import { applyStoreMixins } from './store/mixin.js';
import { A2APaymentsMethods } from './store/payments.js';
import { A2AQuotesMethods } from './store/quotes.js';
import { A2AEscrowMethods } from './store/escrow.js';
import { A2ADisputesMethods } from './store/disputes.js';
import { A2AReputationMethods } from './store/reputation.js';
import { A2AMarketplaceMethods } from './store/marketplace.js';
import { A2ANotificationsMethods } from './store/notifications.js';
import { A2ASubscriptionsMethods } from './store/subscriptions.js';
import { A2ASplitsMethods } from './store/splits.js';
import { A2AEventsMethods } from './store/events.js';
import { A2AAgentsMethods } from './store/agents.js';
import { A2ASLAMethods } from './store/sla.js';
import { A2AWorkflowsMethods } from './store/workflows.js';

/**
 * Default on-disk location of the A2A SQLite database.
 * @returns {string}
 */
export function defaultA2ADbPath() {
  return path.join(os.homedir(), '.stateset', 'a2a.db');
}

/**
 * A2A Store - SQLite storage for A2A commerce
 */
export class A2AStore {
  constructor(options = {}) {
    if (typeof options === 'string') {
      options = { dbPath: options };
    }
    this.dbPath = options.dbPath || defaultA2ADbPath();
    this.db = null;
  }

  init() {
    if (this.db) return;
    const Database = loadDatabaseCtor();
    if (!Database) {
      throw createSqliteUnavailableError();
    }

    try {
      this.db = new Database(this.dbPath);
    } catch (error) {
      if (error?.code === 'ERR_DLOPEN_FAILED' || error?.code === 'MODULE_NOT_FOUND') {
        throw createSqliteUnavailableError(error);
      }
      throw error;
    }
    this.db.pragma('journal_mode = WAL');
    this.db.exec(A2A_SCHEMA);
    this._migrateQuotes();
    this._migrateEscrows();
    this._migrateAgentCards();
  }

  close() {
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }

  /**
   * Validate that all keys in an update object are whitelisted columns.
   * Prevents SQL column injection via dynamic SET clauses.
   * @param {string} table - The table name
   * @param {string[]} keys - Column names from the updates object
   * @throws {Error} If any key is not in the whitelist
   */
  _validateUpdateKeys(table, keys) {
    const allowed = UPDATABLE_COLUMNS[table];
    if (!allowed) throw new Error(`Unknown table for update validation: ${table}`);
    for (const key of keys) {
      if (!allowed.has(key)) {
        throw new Error(`Column '${key}' is not allowed for update on ${table}`);
      }
    }
  }
}

// Domain methods live in ./store/*.js and are mixed onto the prototype in
// the same order they appeared in the original single-file class.
applyStoreMixins(
  A2AStore,
  A2ASchemaMigrations,
  A2APaymentsMethods,
  A2AQuotesMethods,
  A2AEscrowMethods,
  A2ADisputesMethods,
  A2AReputationMethods,
  A2AMarketplaceMethods,
  A2ANotificationsMethods,
  A2ASubscriptionsMethods,
  A2ASplitsMethods,
  A2AEventsMethods,
  A2AAgentsMethods,
  A2ASLAMethods,
  A2AWorkflowsMethods,
);

export default A2AStore;
