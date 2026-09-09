// SQLite protocol persistence. Host supplies a better-sqlite3-compatible handle.
// This stores reference protocol records, not native Commerce order aggregates.
export class SqliteProtocolStore {
  constructor(db) {
    this.db = db;
    db.exec(`
      CREATE TABLE IF NOT EXISTS _icp_records (
        namespace TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
        PRIMARY KEY(namespace,key)
      );
      CREATE TABLE IF NOT EXISTS _icp_nonces (
        signer TEXT NOT NULL, nonce TEXT NOT NULL, expires_at INTEGER NOT NULL,
        PRIMARY KEY(signer,nonce)
      );
      CREATE INDEX IF NOT EXISTS _icp_nonce_expiry ON _icp_nonces(expires_at);
    `);
    this.atomic(() => {
      const metadata = this.collection('metadata');
      const version = metadata.get('schema_version');
      if (version !== undefined && version !== 1) throw new Error('unsupported ICP store version');
      metadata.set('schema_version', 1);
    });
  }

  atomic(fn) {
    if (fn.constructor.name === 'AsyncFunction')
      throw new Error('protocol transactions must be synchronous');
    return this.db
      .transaction(() => {
        const result = fn();
        if (result?.then) throw new Error('protocol transactions must be synchronous');
        return result;
      })
      .immediate();
  }

  collection(namespace) {
    const db = this.db;
    return {
      get(key) {
        const row = db
          .prepare('SELECT value FROM _icp_records WHERE namespace=? AND key=?')
          .get(namespace, key);
        return row ? JSON.parse(row.value) : undefined;
      },
      has(key) {
        return this.get(key) !== undefined;
      },
      set(key, value) {
        db.prepare(
          `INSERT INTO _icp_records VALUES(?,?,?)
          ON CONFLICT(namespace,key) DO UPDATE SET value=excluded.value`,
        ).run(namespace, key, JSON.stringify(value));
      },
      delete(key) {
        db.prepare('DELETE FROM _icp_records WHERE namespace=? AND key=?').run(namespace, key);
      },
      values() {
        return db
          .prepare('SELECT value FROM _icp_records WHERE namespace=? ORDER BY key')
          .all(namespace)
          .map((row) => JSON.parse(row.value));
      },
      get size() {
        return db
          .prepare('SELECT COUNT(*) AS count FROM _icp_records WHERE namespace=?')
          .get(namespace).count;
      },
    };
  }

  bindIdentity(identity) {
    if (
      typeof identity.aid !== 'string' ||
      !identity.aid.trim() ||
      !/^[a-f0-9]{64}$/.test(identity.publicKey)
    )
      throw new Error('invalid merchant identity');
    this.atomic(() => {
      const metadata = this.collection('metadata');
      const prior = metadata.get('identity');
      if (prior && (prior.aid !== identity.aid || prior.publicKey !== identity.publicKey)) {
        throw new Error('merchant identity does not match this protocol database');
      }
      metadata.set('identity', identity);
    });
  }

  /**
   * Durable §5.3 replay guard. Capacity is charged PER SIGNER AID: one signer
   * flooding caller-chosen nonces can only lock itself out, never every other
   * agent sharing the handler. `maxSigners` separately bounds how many
   * distinct AIDs may hold live nonces, so freshly minted AIDs cannot grow the
   * table without limit. `maxEntries` is the deprecated spelling of
   * `maxEntriesPerSigner`.
   */
  replayGuard({
    ttlMs = 86400000,
    maxEntriesPerSigner,
    maxEntries,
    maxSigners = 100000,
    now = Date.now,
  } = {}) {
    const perSigner = maxEntriesPerSigner ?? maxEntries ?? 1000;
    if (
      !Number.isSafeInteger(ttlMs) ||
      ttlMs < 86400000 ||
      !Number.isSafeInteger(perSigner) ||
      perSigner <= 0 ||
      !Number.isSafeInteger(maxSigners) ||
      maxSigners <= 0
    )
      throw new Error('invalid durable nonce policy');
    const db = this.db;
    return {
      checkAndRecord: (signer, nonce) =>
        this.atomic(() => {
          const timestamp = now();
          db.prepare('DELETE FROM _icp_nonces WHERE expires_at<=?').run(timestamp);
          if (db.prepare('SELECT 1 FROM _icp_nonces WHERE signer=? AND nonce=?').get(signer, nonce))
            return false;
          const live = db
            .prepare('SELECT COUNT(*) AS count FROM _icp_nonces WHERE signer=?')
            .get(signer).count;
          if (live >= perSigner) return false;
          if (
            live === 0 &&
            db.prepare('SELECT COUNT(DISTINCT signer) AS count FROM _icp_nonces').get().count >=
              maxSigners
          )
            return false;
          db.prepare('INSERT INTO _icp_nonces VALUES(?,?,?)').run(signer, nonce, timestamp + ttlMs);
          return true;
        }),
      size: () =>
        db.prepare('SELECT COUNT(*) AS count FROM _icp_nonces WHERE expires_at>?').get(now()).count,
      sizeFor: (signer) =>
        db
          .prepare('SELECT COUNT(*) AS count FROM _icp_nonces WHERE signer=? AND expires_at>?')
          .get(signer, now()).count,
    };
  }
}
