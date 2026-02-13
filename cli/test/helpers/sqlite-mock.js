/**
 * In-memory SQLite mock for unit tests.
 *
 * Replaces better-sqlite3 when the native module is not available.
 * Supports the basic prepare/run/get/all API surface used across the codebase:
 *   - audit-store.js
 *   - channels/session-store.js
 *   - treasury/store.js
 *   - conflict-resolver.js
 *   - markdown-store.js
 *
 * Data is stored in plain Maps keyed by table name. This is intentionally
 * simple — no SQL parsing — but covers the patterns used in tests:
 *   db.exec('CREATE TABLE IF NOT EXISTS ...')
 *   db.prepare('INSERT ...').run(...)
 *   db.prepare('SELECT ...').get(...)
 *   db.prepare('SELECT ...').all(...)
 *   db.transaction(fn)(...)
 *   db.pragma('...')
 */

export class MockDatabase {
  /**
   * @param {string} [path=':memory:'] - Database path (ignored, always in-memory)
   */
  constructor(path = ':memory:') {
    this.path = path;
    /** @type {Map<string, object[]>} */
    this.tables = new Map();
    this.open = true;
    this.inTransaction = false;
    this._pragmas = new Map();
  }

  /**
   * Prepare a statement for execution.
   * @param {string} sql - SQL string
   * @returns {MockStatement}
   */
  prepare(sql) {
    return new MockStatement(this, sql);
  }

  /**
   * Execute raw SQL. Only processes CREATE TABLE statements (extracts table names).
   * @param {string} sql
   * @returns {this}
   */
  exec(sql) {
    if (!this.open) throw new Error('Database is closed');

    // Extract table names from CREATE TABLE IF NOT EXISTS statements
    const createPattern = /CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+(\w+)/gi;
    let match;
    while ((match = createPattern.exec(sql)) !== null) {
      const name = match[1];
      if (!this.tables.has(name)) {
        this.tables.set(name, []);
      }
    }

    // Also handle plain CREATE TABLE (without IF NOT EXISTS)
    const createPlainPattern = /CREATE\s+TABLE\s+(?!IF)(\w+)/gi;
    while ((match = createPlainPattern.exec(sql)) !== null) {
      const name = match[1];
      if (!this.tables.has(name)) {
        this.tables.set(name, []);
      }
    }

    return this;
  }

  /**
   * Wrap a function in a transaction. The mock executes synchronously.
   * Supports .deferred, .immediate, .exclusive chaining.
   * @param {Function} fn
   * @returns {Function}
   */
  transaction(fn) {
    const self = this;
    const wrapped = (...args) => {
      if (!self.open) throw new Error('Database is closed');
      self.inTransaction = true;
      try {
        const result = fn(...args);
        self.inTransaction = false;
        return result;
      } catch (err) {
        self.inTransaction = false;
        throw err;
      }
    };
    wrapped.deferred = wrapped;
    wrapped.immediate = wrapped;
    wrapped.exclusive = wrapped;
    return wrapped;
  }

  /**
   * Get or set a pragma value.
   * @param {string} setting - Pragma setting string (e.g. 'journal_mode = WAL')
   * @returns {*}
   */
  pragma(setting) {
    if (!this.open) throw new Error('Database is closed');

    // Parse "key = value" form
    const eqMatch = setting.match(/^(\w+)\s*=\s*(.+)$/);
    if (eqMatch) {
      this._pragmas.set(eqMatch[1], eqMatch[2].trim());
      return eqMatch[2].trim();
    }

    // Read-only form: return stored value or the setting name
    return this._pragmas.get(setting) || setting;
  }

  /**
   * Close the database.
   */
  close() {
    this.open = false;
  }
}

/**
 * A mock prepared statement. Tracks the SQL text and supports
 * bind/run/get/all/pluck as no-ops with sensible defaults.
 */
class MockStatement {
  /**
   * @param {MockDatabase} db
   * @param {string} sql
   */
  constructor(db, sql) {
    this.db = db;
    this.sql = sql;
    this._boundParams = [];
    this._pluck = false;
  }

  /**
   * Bind parameters to this statement.
   * @param {...*} params
   * @returns {this}
   */
  bind(...params) {
    this._boundParams = params;
    return this;
  }

  /**
   * Execute an INSERT/UPDATE/DELETE statement.
   * @param {...*} params - Positional parameters
   * @returns {{ changes: number, lastInsertRowid: number }}
   */
  run(...params) {
    if (!this.db.open) throw new Error('Database is closed');
    return { changes: 1, lastInsertRowid: 1 };
  }

  /**
   * Execute a SELECT and return the first row.
   * @param {...*} params - Positional parameters
   * @returns {object|undefined}
   */
  get(...params) {
    if (!this.db.open) throw new Error('Database is closed');
    return undefined;
  }

  /**
   * Execute a SELECT and return all rows.
   * @param {...*} params - Positional parameters
   * @returns {object[]}
   */
  all(...params) {
    if (!this.db.open) throw new Error('Database is closed');
    return [];
  }

  /**
   * Enable pluck mode (return scalar column values instead of row objects).
   * @param {boolean} [flag=true]
   * @returns {this}
   */
  pluck(flag = true) {
    this._pluck = flag;
    return this;
  }

  /**
   * Return an iterator over result rows (used by some codebase patterns).
   * @param {...*} params
   * @returns {IterableIterator<object>}
   */
  iterate(...params) {
    if (!this.db.open) throw new Error('Database is closed');
    return [][Symbol.iterator]();
  }

  /**
   * Get the columns that this statement returns.
   * @returns {object[]}
   */
  columns() {
    return [];
  }
}

export default MockDatabase;
