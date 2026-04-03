/**
 * Credential Store for StateSet Providers
 *
 * SQLite-backed API key storage with WAL enabled for safe concurrent access.
 * Falls back to a durable JSON store when the native SQLite binding is unavailable.
 */

import { createRequire } from 'node:module';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import crypto from 'node:crypto';
import { PROVIDERS } from './config.js';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'credentials.db');
const DIRECTORY_MODE = 0o700;
const FILE_MODE = 0o600;
const KEY_FILE_NAME = 'credentials.key';
const CREDENTIALS_KEY_ENV = 'STATESET_CREDENTIALS_KEY';
const ENCRYPTION_PREFIX = 'enc:v1';
const KEY_SIZE_BYTES = 32;
const IV_SIZE_BYTES = 12;
const AUTH_TAG_BYTES = 16;
const PBKDF2_ITERATIONS = 200_000;
const PBKDF2_DIGEST = 'sha256';
const require = createRequire(import.meta.url);
const FALLBACK_CREDENTIAL_DATABASES = new Map();
let cachedDatabaseCtor;

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

function setPermissionIfSupported(targetPath, mode) {
  try {
    fs.chmodSync(targetPath, mode);
  } catch (chmodErr) {
    console.debug('chmod not supported on this platform:', chmodErr.message);
  }
}

function decodeBase64Key(encoded) {
  try {
    const decoded = Buffer.from(encoded, 'base64');
    return decoded.length === KEY_SIZE_BYTES ? decoded : null;
  } catch {
    return null;
  }
}

function deriveKeyFromPassphrase(passphrase, dbPath) {
  const salt = crypto.createHash('sha256').update(`stateset:${dbPath}`).digest();
  return crypto.pbkdf2Sync(passphrase, salt, PBKDF2_ITERATIONS, KEY_SIZE_BYTES, PBKDF2_DIGEST);
}

function loadOrCreateLocalKey(dir) {
  const keyPath = path.join(dir, KEY_FILE_NAME);
  if (fs.existsSync(keyPath)) {
    const existingRaw = fs.readFileSync(keyPath, 'utf8').trim();
    const existing = decodeBase64Key(existingRaw);
    if (existing) {
      setPermissionIfSupported(keyPath, FILE_MODE);
      return existing;
    }
    throw new Error(
      `Credential key file is invalid (${keyPath}). Refusing to rotate automatically to avoid data loss.`,
    );
  }

  const generated = crypto.randomBytes(KEY_SIZE_BYTES);
  fs.writeFileSync(keyPath, generated.toString('base64'), { mode: FILE_MODE });
  setPermissionIfSupported(keyPath, FILE_MODE);
  return generated;
}

function resolveEncryptionKey(dir, dbPath) {
  const envPassphrase = process.env[CREDENTIALS_KEY_ENV];
  if (envPassphrase && envPassphrase.trim().length > 0) {
    return deriveKeyFromPassphrase(envPassphrase, dbPath);
  }

  return loadOrCreateLocalKey(dir);
}

function getFallbackCredentialPath(dbPath) {
  return dbPath === ':memory:' ? ':memory:' : `${dbPath}.fallback.json`;
}

function persistFallbackCredentialDatabase(state) {
  if (!state?.storagePath || state.storagePath === ':memory:') {
    return;
  }

  fs.mkdirSync(path.dirname(state.storagePath), { recursive: true, mode: DIRECTORY_MODE });
  const tmpPath = `${state.storagePath}.tmp`;
  const payload = {
    rows: Array.from(state.rows.values()),
  };
  fs.writeFileSync(tmpPath, JSON.stringify(payload, null, 2), { mode: FILE_MODE });
  fs.renameSync(tmpPath, state.storagePath);
  setPermissionIfSupported(state.storagePath, FILE_MODE);
}

function getFallbackCredentialDatabase(dbPath) {
  const storagePath = getFallbackCredentialPath(dbPath);
  if (storagePath === ':memory:') {
    return { rows: new Map(), storagePath };
  }

  let state = FALLBACK_CREDENTIAL_DATABASES.get(storagePath);
  if (!state) {
    const rows = new Map();
    if (fs.existsSync(storagePath)) {
      try {
        const raw = fs.readFileSync(storagePath, 'utf8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          for (const row of parsed?.rows || []) {
            if (row?.provider) {
              rows.set(row.provider, row);
            }
          }
        }
      } catch (error) {
        console.warn(
          `[credentials] Failed to read fallback credential store ${storagePath}: ${error.message}`,
        );
      }
    }
    state = { rows, storagePath };
    FALLBACK_CREDENTIAL_DATABASES.set(storagePath, state);
  }
  return state;
}

function createFallbackCredentialDb(state) {
  return {
    pragma() {
      return 'WAL';
    },
    exec() {
      return this;
    },
    prepare(sql) {
      const normalizedSql = sql.trim().replace(/\s+/g, ' ').toUpperCase();
      return {
        get(provider) {
          const row = state.rows.get(provider);
          if (!row) return undefined;
          if (normalizedSql.startsWith('SELECT API_KEY, UPDATED_AT FROM PROVIDER_CREDENTIALS')) {
            return { api_key: row.api_key, updated_at: row.updated_at };
          }
          if (normalizedSql.startsWith('SELECT API_KEY FROM PROVIDER_CREDENTIALS')) {
            return { api_key: row.api_key };
          }
          return undefined;
        },
        run(...params) {
          if (normalizedSql.startsWith('INSERT INTO PROVIDER_CREDENTIALS')) {
            const [provider, apiKey, updatedAt] = params;
            state.rows.set(provider, {
              provider,
              api_key: apiKey,
              updated_at: updatedAt,
            });
            return { changes: 1, lastInsertRowid: 1 };
          }
          if (normalizedSql.startsWith('DELETE FROM PROVIDER_CREDENTIALS')) {
            const [provider] = params;
            const existed = state.rows.delete(provider);
            return { changes: existed ? 1 : 0, lastInsertRowid: 0 };
          }
          return { changes: 0, lastInsertRowid: 0 };
        },
        all() {
          if (normalizedSql.startsWith('SELECT PROVIDER, UPDATED_AT FROM PROVIDER_CREDENTIALS')) {
            return [...state.rows.values()]
              .sort((a, b) => b.updated_at - a.updated_at)
              .map((row) => ({ provider: row.provider, updated_at: row.updated_at }));
          }
          return [];
        },
      };
    },
    close() {},
  };
}

export class CredentialStore {
  constructor({ dbPath = DEFAULT_DB_PATH, databaseCtor } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true, mode: DIRECTORY_MODE });
    setPermissionIfSupported(dir, DIRECTORY_MODE);

    if (!fs.existsSync(dbPath)) {
      const fd = fs.openSync(dbPath, 'w', FILE_MODE);
      fs.closeSync(fd);
    }
    setPermissionIfSupported(dbPath, FILE_MODE);

    const Database = databaseCtor === undefined ? loadDatabaseCtor() : databaseCtor;
    this._fallbackState = null;
    this.backend = 'sqlite';

    if (!Database) {
      this._enableFallback(dbPath, 'better-sqlite3 unavailable');
    } else {
      try {
        this.db = new Database(dbPath);
      } catch (error) {
        if (error?.code !== 'ERR_DLOPEN_FAILED') {
          throw error;
        }
        this._enableFallback(dbPath, error.message || 'native module load failure');
      }
    }

    this.db.pragma('journal_mode = WAL');
    setPermissionIfSupported(dbPath, FILE_MODE);
    this.encryptionKey = resolveEncryptionKey(dir, dbPath);

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

  _enableFallback(dbPath, reason = 'fallback requested') {
    this._fallbackState = getFallbackCredentialDatabase(dbPath);
    this.backend = 'json-fallback';
    console.warn(
      `[credentials] ${reason}; using durable JSON fallback at ${this._fallbackState.storagePath}`,
    );
    this.db = createFallbackCredentialDb(this._fallbackState);
  }

  encryptApiKey(apiKey) {
    const iv = crypto.randomBytes(IV_SIZE_BYTES);
    const cipher = crypto.createCipheriv('aes-256-gcm', this.encryptionKey, iv);
    const encrypted = Buffer.concat([cipher.update(apiKey, 'utf8'), cipher.final()]);
    const authTag = cipher.getAuthTag();
    return `${ENCRYPTION_PREFIX}:${iv.toString('base64')}:${authTag.toString('base64')}:${encrypted.toString('base64')}`;
  }

  decryptApiKey(storedValue) {
    if (!storedValue?.startsWith(`${ENCRYPTION_PREFIX}:`)) {
      return storedValue;
    }

    const payload = storedValue.slice(ENCRYPTION_PREFIX.length + 1);
    const parts = payload.split(':');
    if (parts.length !== 3) {
      return null;
    }

    const iv = Buffer.from(parts[0], 'base64');
    const authTag = Buffer.from(parts[1], 'base64');
    const encrypted = Buffer.from(parts[2], 'base64');
    if (
      iv.length !== IV_SIZE_BYTES ||
      authTag.length !== AUTH_TAG_BYTES ||
      encrypted.length === 0
    ) {
      return null;
    }

    try {
      const decipher = crypto.createDecipheriv('aes-256-gcm', this.encryptionKey, iv);
      decipher.setAuthTag(authTag);
      const decrypted = Buffer.concat([decipher.update(encrypted), decipher.final()]);
      return decrypted.toString('utf8');
    } catch {
      return null;
    }
  }

  getApiKey(provider) {
    const row = this._get.get(provider);
    if (!row) return null;

    const decrypted = this.decryptApiKey(row.api_key);
    if (decrypted === null) {
      console.warn(`[credentials] Unable to decrypt key for provider ${provider}`);
      return null;
    }
    return decrypted;
  }

  setApiKey(provider, apiKey) {
    if (!provider || !apiKey) return false;
    this._upsert.run(provider, this.encryptApiKey(apiKey), Date.now());
    if (this._fallbackState) {
      persistFallbackCredentialDatabase(this._fallbackState);
    }
    return true;
  }

  removeApiKey(provider) {
    const removed = this._delete.run(provider).changes > 0;
    if (removed && this._fallbackState) {
      persistFallbackCredentialDatabase(this._fallbackState);
    }
    return removed;
  }

  listProviders() {
    return this._list.all().map((row) => ({
      provider: row.provider,
      updatedAt: row.updated_at,
    }));
  }

  close() {
    if (this._fallbackState) {
      persistFallbackCredentialDatabase(this._fallbackState);
    }
    this.db.close();
  }
}

let _store = null;
let _storeInitError = null;
let _storeWarningShown = false;

function normalizeStoreError(err) {
  return err instanceof Error ? err : new Error(String(err));
}

function warnStoreFallbackOnce(err) {
  if (_storeWarningShown) {
    return;
  }

  const message = normalizeStoreError(err).message;
  console.warn(
    '[credentials] Credential store unavailable, falling back to environment variables:',
    message,
  );
  _storeWarningShown = true;
}

export function getCredentialStore(options = {}) {
  if (_store) {
    return _store;
  }
  if (_storeInitError) {
    throw _storeInitError;
  }

  try {
    _store = new CredentialStore(options);
    return _store;
  } catch (err) {
    _storeInitError = normalizeStoreError(err);
    throw _storeInitError;
  }
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
    warnStoreFallbackOnce(err);
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
  _storeInitError = null;
  _storeWarningShown = false;
}
