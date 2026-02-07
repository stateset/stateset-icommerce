/**
 * Credential Store for StateSet Providers
 *
 * SQLite-backed API key storage with WAL enabled for safe concurrent access.
 */

import Database from 'better-sqlite3';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { PROVIDERS } from './config.js';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'credentials.db');

export class CredentialStore {
  constructor({ dbPath = DEFAULT_DB_PATH } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');

    this.db.exec(`
      CREATE TABLE IF NOT EXISTS provider_credentials (
        provider   TEXT PRIMARY KEY,
        api_key    TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      )
    `);

    this._get = this.db.prepare(
      `SELECT api_key, updated_at FROM provider_credentials WHERE provider = ?`,
    );
    this._upsert = this.db.prepare(
      `INSERT INTO provider_credentials (provider, api_key, updated_at)
       VALUES (?, ?, ?)
       ON CONFLICT(provider)
       DO UPDATE SET api_key = excluded.api_key, updated_at = excluded.updated_at`,
    );
    this._delete = this.db.prepare(`DELETE FROM provider_credentials WHERE provider = ?`);
    this._list = this.db.prepare(
      `SELECT provider, updated_at FROM provider_credentials ORDER BY updated_at DESC`,
    );
  }

  getApiKey(provider) {
    const row = this._get.get(provider);
    return row ? row.api_key : null;
  }

  setApiKey(provider, apiKey) {
    if (!provider || !apiKey) return false;
    this._upsert.run(provider, apiKey, Date.now());
    return true;
  }

  removeApiKey(provider) {
    return this._delete.run(provider).changes > 0;
  }

  listProviders() {
    return this._list.all().map((row) => ({
      provider: row.provider,
      updatedAt: row.updated_at,
    }));
  }

  close() {
    this.db.close();
  }
}

let _store = null;

export function getCredentialStore(options = {}) {
  if (!_store) {
    _store = new CredentialStore(options);
  }
  return _store;
}

/**
 * Resolve API key for a provider with precedence:
 * 1) Credential store
 * 2) Environment variable (from PROVIDERS config)
 */
export function resolveProviderApiKey(provider) {
  if (!provider) return null;
  try {
    const storeKey = getCredentialStore().getApiKey(provider);
    if (storeKey) return storeKey;
  } catch (err) {
    console.warn(`[credentials] Store lookup failed for ${provider}:`, err.message);
  }

  const envKey = PROVIDERS[provider]?.envKey;
  if (!envKey) return null;
  return process.env[envKey] || null;
}

export function resetCredentialStore() {
  if (_store) {
    try {
      _store.close();
    } catch (err) {
      console.warn('[credentials] Store close error:', err.message);
    }
  }
  _store = null;
}
