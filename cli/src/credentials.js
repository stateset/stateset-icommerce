/**
 * Credential Store for StateSet Providers
 *
 * SQLite-backed API key storage with WAL enabled for safe concurrent access.
 */

import Database from 'better-sqlite3';
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

function setPermissionIfSupported(targetPath, mode) {
  try {
    fs.chmodSync(targetPath, mode);
  } catch {
    // ignore platforms/filesystems that do not support chmod
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

export class CredentialStore {
  constructor({ dbPath = DEFAULT_DB_PATH } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true, mode: DIRECTORY_MODE });
    setPermissionIfSupported(dir, DIRECTORY_MODE);

    if (!fs.existsSync(dbPath)) {
      const fd = fs.openSync(dbPath, 'w', FILE_MODE);
      fs.closeSync(fd);
    }
    setPermissionIfSupported(dbPath, FILE_MODE);

    this.db = new Database(dbPath);
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
