/**
 * External ID ↔ StateSet ID Mapping Store
 *
 * Persists mappings between external platform IDs and internal StateSet UUIDs.
 * Used for incremental imports and cross-entity reference resolution
 * (e.g., looking up a customer's StateSet ID when importing an order that
 * references the customer's Shopify ID).
 */

const CREATE_TABLE_SQL = `
  CREATE TABLE IF NOT EXISTS id_map (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    stateset_id TEXT NOT NULL,
    external_data TEXT,
    imported_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(platform, entity_type, external_id)
  )
`;

const CREATE_INDEX_LOOKUP_SQL = `
  CREATE INDEX IF NOT EXISTS idx_id_map_lookup
  ON id_map(platform, entity_type, external_id)
`;

const CREATE_INDEX_STATESET_SQL = `
  CREATE INDEX IF NOT EXISTS idx_id_map_stateset
  ON id_map(stateset_id)
`;

/**
 * ID mapping store backed by SQLite.
 */
export class IdMapStore {
  /**
   * @param {import('better-sqlite3').Database} db - SQLite database handle
   */
  constructor(db) {
    if (!db) throw new Error('IdMapStore requires a database handle');
    this.db = db;
    this._initialized = false;
  }

  /**
   * Ensure the id_map table and indexes exist.
   */
  init() {
    if (this._initialized) return;
    this.db.exec(CREATE_TABLE_SQL);
    this.db.exec(CREATE_INDEX_LOOKUP_SQL);
    this.db.exec(CREATE_INDEX_STATESET_SQL);
    this._initialized = true;
  }

  /**
   * Look up the StateSet ID for an external record.
   * @param {string} platform
   * @param {string} entityType
   * @param {string} externalId
   * @returns {{ statesetId: string, importedAt: string, externalData?: string } | null}
   */
  lookup(platform, entityType, externalId) {
    this.init();
    const row = this.db
      .prepare(
        'SELECT stateset_id, imported_at, external_data FROM id_map WHERE platform = ? AND entity_type = ? AND external_id = ?',
      )
      .get(platform, entityType, String(externalId));

    if (!row) return null;
    return {
      statesetId: row.stateset_id,
      importedAt: row.imported_at,
      externalData: row.external_data || null,
    };
  }

  /**
   * Store a mapping between an external ID and a StateSet ID.
   * Upserts — if the mapping already exists, updates stateset_id and external_data.
   * @param {string} platform
   * @param {string} entityType
   * @param {string} externalId
   * @param {string} statesetId
   * @param {Object} [externalData] - Optional snapshot of the external record
   */
  store(platform, entityType, externalId, statesetId, externalData = null) {
    this.init();
    const dataJson = externalData ? JSON.stringify(externalData) : null;
    const id = crypto.randomUUID();

    this.db
      .prepare(
        `INSERT INTO id_map (id, platform, entity_type, external_id, stateset_id, external_data)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(platform, entity_type, external_id) DO UPDATE SET
           stateset_id = excluded.stateset_id,
           external_data = excluded.external_data,
           updated_at = datetime('now')`,
      )
      .run(id, platform, entityType, String(externalId), statesetId, dataJson);
  }

  /**
   * List all mappings for a given platform and optional entity type.
   * @param {string} platform
   * @param {string} [entityType]
   * @returns {Array<{ externalId: string, statesetId: string, entityType: string, importedAt: string }>}
   */
  listByPlatform(platform, entityType = null) {
    this.init();
    if (entityType) {
      return this.db
        .prepare(
          'SELECT external_id, stateset_id, entity_type, imported_at FROM id_map WHERE platform = ? AND entity_type = ? ORDER BY imported_at DESC',
        )
        .all(platform, entityType)
        .map((row) => ({
          externalId: row.external_id,
          statesetId: row.stateset_id,
          entityType: row.entity_type,
          importedAt: row.imported_at,
        }));
    }

    return this.db
      .prepare(
        'SELECT external_id, stateset_id, entity_type, imported_at FROM id_map WHERE platform = ? ORDER BY imported_at DESC',
      )
      .all(platform)
      .map((row) => ({
        externalId: row.external_id,
        statesetId: row.stateset_id,
        entityType: row.entity_type,
        importedAt: row.imported_at,
      }));
  }

  /**
   * Delete all mappings for a platform (for clean re-imports).
   * @param {string} platform
   * @returns {number} Number of rows deleted
   */
  deleteByPlatform(platform) {
    this.init();
    const result = this.db.prepare('DELETE FROM id_map WHERE platform = ?').run(platform);
    return result.changes;
  }

  /**
   * Get the count of mappings for a platform.
   * @param {string} platform
   * @param {string} [entityType]
   * @returns {number}
   */
  count(platform, entityType = null) {
    this.init();
    if (entityType) {
      const row = this.db
        .prepare('SELECT COUNT(*) as cnt FROM id_map WHERE platform = ? AND entity_type = ?')
        .get(platform, entityType);
      return row.cnt;
    }

    const row = this.db
      .prepare('SELECT COUNT(*) as cnt FROM id_map WHERE platform = ?')
      .get(platform);
    return row.cnt;
  }
}

import crypto from 'crypto';

export default IdMapStore;
