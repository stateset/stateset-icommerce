/**
 * Unit tests for credentials.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { CredentialStore, resetCredentialStore } from '../../src/credentials.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cred-test-'));
  return path.join(dir, 'credentials.db');
}

function tmpDbDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'cred-test-dir-'));
}

// ===========================================================================
// CredentialStore
// ===========================================================================

describe('CredentialStore', () => {
  /** @type {CredentialStore|null} */
  let store = null;

  afterEach(() => {
    if (store) {
      try {
        store.close();
      } catch {}
      store = null;
    }
  });

  it('creates database and table on construction', () => {
    const dbPath = tmpDbPath();
    store = new CredentialStore({ dbPath });
    assert.ok(fs.existsSync(dbPath));
  });

  it('uses restricted permissions for credential storage', () => {
    if (process.platform === 'win32') return;
    const dbPath = tmpDbPath();
    store = new CredentialStore({ dbPath });

    const dbMode = fs.statSync(dbPath).mode & 0o777;
    const dirMode = fs.statSync(path.dirname(dbPath)).mode & 0o777;
    const keyMode = fs.statSync(path.join(path.dirname(dbPath), 'credentials.key')).mode & 0o777;

    assert.strictEqual(dbMode, 0o600);
    assert.strictEqual(dirMode, 0o700);
    assert.strictEqual(keyMode, 0o600);
  });

  it('getApiKey returns null for unknown provider', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.getApiKey('nonexistent'), null);
  });

  it('setApiKey stores and getApiKey retrieves', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    const result = store.setApiKey('openai', 'sk-test-123');
    assert.strictEqual(result, true);
    assert.strictEqual(store.getApiKey('openai'), 'sk-test-123');
  });

  it('stores encrypted api keys at rest', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.setApiKey('openai', 'sk-secret-plaintext');
    const row = store.db
      .prepare('SELECT api_key FROM provider_credentials WHERE provider = ?')
      .get('openai');
    assert.ok(row.api_key.startsWith('enc:v1:'));
    assert.notStrictEqual(row.api_key, 'sk-secret-plaintext');
  });

  it('supports reading legacy plaintext rows', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.db
      .prepare(
        `INSERT INTO provider_credentials (provider, api_key, updated_at) VALUES (?, ?, ?)`,
      )
      .run('legacy-provider', 'legacy-plain-key', Date.now());
    assert.strictEqual(store.getApiKey('legacy-provider'), 'legacy-plain-key');
  });

  it('returns null when encrypted payload is malformed', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.db
      .prepare(
        `INSERT INTO provider_credentials (provider, api_key, updated_at) VALUES (?, ?, ?)`,
      )
      .run('broken-provider', 'enc:v1:not-valid', Date.now());
    assert.strictEqual(store.getApiKey('broken-provider'), null);
  });

  it('fails fast when existing key file is corrupted', () => {
    const dir = tmpDbDir();
    const dbPath = path.join(dir, 'credentials.db');
    fs.writeFileSync(path.join(dir, 'credentials.key'), 'not-a-valid-key');
    assert.throws(
      () => new CredentialStore({ dbPath }),
      /Credential key file is invalid/,
    );
  });

  it('setApiKey returns false for empty provider', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.setApiKey('', 'sk-test'), false);
    assert.strictEqual(store.setApiKey(null, 'sk-test'), false);
  });

  it('setApiKey returns false for empty apiKey', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.setApiKey('openai', ''), false);
    assert.strictEqual(store.setApiKey('openai', null), false);
  });

  it('setApiKey upserts (overwrites existing key)', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.setApiKey('openai', 'sk-old');
    store.setApiKey('openai', 'sk-new');
    assert.strictEqual(store.getApiKey('openai'), 'sk-new');
  });

  it('removeApiKey deletes existing provider', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.setApiKey('openai', 'sk-test');
    const removed = store.removeApiKey('openai');
    assert.strictEqual(removed, true);
    assert.strictEqual(store.getApiKey('openai'), null);
  });

  it('removeApiKey returns false for non-existent provider', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.removeApiKey('nonexistent'), false);
  });

  it('listProviders returns all stored providers', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.setApiKey('openai', 'sk-openai');
    store.setApiKey('anthropic', 'sk-ant');
    store.setApiKey('gemini', 'sk-gem');

    const providers = store.listProviders();
    assert.strictEqual(providers.length, 3);
    const names = providers.map((p) => p.provider);
    assert.ok(names.includes('openai'));
    assert.ok(names.includes('anthropic'));
    assert.ok(names.includes('gemini'));
  });

  it('listProviders returns empty array when no providers', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    assert.deepStrictEqual(store.listProviders(), []);
  });

  it('listProviders includes updatedAt timestamp', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    const before = Date.now();
    store.setApiKey('openai', 'sk-test');
    const providers = store.listProviders();
    assert.strictEqual(providers.length, 1);
    assert.ok(providers[0].updatedAt >= before);
  });

  it('close() can be called without error', () => {
    store = new CredentialStore({ dbPath: tmpDbPath() });
    store.close();
    store = null; // prevent double-close in afterEach
  });

  it('persists data across instances', () => {
    const dbPath = tmpDbPath();
    store = new CredentialStore({ dbPath });
    store.setApiKey('openai', 'sk-persistent');
    store.close();

    const store2 = new CredentialStore({ dbPath });
    assert.strictEqual(store2.getApiKey('openai'), 'sk-persistent');
    store2.close();
    store = null;
  });

  it('persists encrypted credentials across instances when SQLite is unavailable', () => {
    const dbPath = tmpDbPath();
    store = new CredentialStore({ dbPath, databaseCtor: null });
    assert.strictEqual(store.backend, 'json-fallback');
    store.setApiKey('openai', 'sk-fallback-persistent');
    store.close();

    const fallbackPath = `${dbPath}.fallback.json`;
    assert.ok(fs.existsSync(fallbackPath));
    const raw = fs.readFileSync(fallbackPath, 'utf8');
    assert.ok(!raw.includes('sk-fallback-persistent'));
    assert.ok(raw.includes('enc:v1:'));

    const reopened = new CredentialStore({ dbPath, databaseCtor: null });
    assert.strictEqual(reopened.getApiKey('openai'), 'sk-fallback-persistent');
    reopened.close();
    store = null;
  });
});

// ===========================================================================
// resetCredentialStore
// ===========================================================================

describe('resetCredentialStore', () => {
  it('can be called safely even when no store is initialized', () => {
    assert.doesNotThrow(() => resetCredentialStore());
  });
});
