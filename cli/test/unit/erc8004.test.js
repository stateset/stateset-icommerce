/**
 * Tests for cli/src/erc8004/index.js
 *
 * Covers: registerIdentity, setAgentWallet, getIdentity,
 * getIdentityByWallet, listIdentities.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'fs';
import path from 'path';
import os from 'os';

import {
  registerIdentity,
  setAgentWallet,
  getIdentity,
  getIdentityByWallet,
  listIdentities,
} from '../../src/erc8004/index.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpDir;
let dbPath;

function setup() {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'erc8004-test-'));
  dbPath = path.join(tmpDir, 'identities.db');
}

function cleanup() {
  try {
    fs.rmSync(tmpDir, { recursive: true });
  } catch {
    /* ok */
  }
}

function baseInput(overrides = {}) {
  return {
    agentRegistry: 'stateset',
    agentId: 'agent-001',
    agentUri: 'https://agent.example.com',
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// registerIdentity
// ---------------------------------------------------------------------------

describe('registerIdentity', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('creates a new identity', () => {
    const result = registerIdentity(dbPath, baseInput());
    assert.ok(result);
    assert.equal(result.agent_registry, 'stateset');
    assert.equal(result.agent_id, 'agent-001');
    assert.equal(result.active, true);
    assert.ok(result.id);
    assert.ok(result.created_at);
  });

  it('uses provided ID', () => {
    const result = registerIdentity(dbPath, baseInput({ id: 'custom-id' }));
    assert.equal(result.id, 'custom-id');
  });

  it('upserts on conflict', () => {
    registerIdentity(dbPath, baseInput({ agentUri: 'https://old.com' }));
    const result = registerIdentity(dbPath, baseInput({ agentUri: 'https://new.com' }));
    assert.equal(result.agent_uri, 'https://new.com');
  });

  it('stores wallet info', () => {
    const result = registerIdentity(
      dbPath,
      baseInput({
        agentWallet: '0x1234',
        walletProofType: 'eip712',
        walletProof: 'proof-data',
      }),
    );
    assert.equal(result.agent_wallet, '0x1234');
    assert.equal(result.wallet_proof_type, 'eip712');
  });

  it('rejects invalid proof type', () => {
    assert.throws(
      () => registerIdentity(dbPath, baseInput({ walletProofType: 'invalid' })),
      /Invalid proof type/,
    );
  });

  it('sets active to false', () => {
    const result = registerIdentity(dbPath, baseInput({ active: false }));
    assert.equal(result.active, false);
  });
});

// ---------------------------------------------------------------------------
// setAgentWallet
// ---------------------------------------------------------------------------

describe('setAgentWallet', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('updates the wallet for an existing identity', () => {
    registerIdentity(dbPath, baseInput());
    const result = setAgentWallet(dbPath, {
      agentRegistry: 'stateset',
      agentId: 'agent-001',
      agentWallet: '0xABCD',
      walletProofType: 'erc1271',
    });
    assert.equal(result.agent_wallet, '0xABCD');
    assert.equal(result.wallet_proof_type, 'erc1271');
  });

  it('throws for non-existent identity', () => {
    assert.throws(
      () =>
        setAgentWallet(dbPath, {
          agentRegistry: 'stateset',
          agentId: 'nonexistent',
          agentWallet: '0x123',
        }),
      /not found/,
    );
  });
});

// ---------------------------------------------------------------------------
// getIdentity
// ---------------------------------------------------------------------------

describe('getIdentity', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('returns identity by registry + id', () => {
    registerIdentity(dbPath, baseInput());
    const result = getIdentity(dbPath, 'stateset', 'agent-001');
    assert.ok(result);
    assert.equal(result.agent_id, 'agent-001');
  });

  it('returns null for unknown identity', () => {
    const result = getIdentity(dbPath, 'stateset', 'unknown');
    assert.equal(result, null);
  });
});

// ---------------------------------------------------------------------------
// getIdentityByWallet
// ---------------------------------------------------------------------------

describe('getIdentityByWallet', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('finds identity by wallet address', () => {
    registerIdentity(dbPath, baseInput({ agentWallet: '0xWALLET' }));
    const result = getIdentityByWallet(dbPath, '0xWALLET');
    assert.ok(result);
    assert.equal(result.agent_wallet, '0xWALLET');
  });

  it('returns null for unknown wallet', () => {
    const result = getIdentityByWallet(dbPath, '0xNOPE');
    assert.equal(result, null);
  });
});

// ---------------------------------------------------------------------------
// listIdentities
// ---------------------------------------------------------------------------

describe('listIdentities', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('lists all identities', () => {
    registerIdentity(dbPath, baseInput({ agentId: 'a1' }));
    registerIdentity(dbPath, baseInput({ agentId: 'a2' }));
    const results = listIdentities(dbPath);
    assert.equal(results.length, 2);
  });

  it('filters by registry', () => {
    registerIdentity(dbPath, baseInput({ agentRegistry: 'reg1', agentId: 'a1' }));
    registerIdentity(dbPath, baseInput({ agentRegistry: 'reg2', agentId: 'a2' }));
    const results = listIdentities(dbPath, { agentRegistry: 'reg1' });
    assert.equal(results.length, 1);
    assert.equal(results[0].agent_registry, 'reg1');
  });

  it('filters by active status', () => {
    registerIdentity(dbPath, baseInput({ agentId: 'a1', active: true }));
    registerIdentity(dbPath, baseInput({ agentId: 'a2', active: false }));
    const active = listIdentities(dbPath, { active: true });
    assert.equal(active.length, 1);
  });

  it('respects limit', () => {
    for (let i = 0; i < 5; i++) {
      registerIdentity(dbPath, baseInput({ agentId: `a${i}` }));
    }
    const results = listIdentities(dbPath, { limit: 2 });
    assert.equal(results.length, 2);
  });

  it('persists fallback identities across calls when SQLite is unavailable', () => {
    registerIdentity(dbPath, baseInput({ agentWallet: '0xFALLBACK' }), { databaseCtor: null });

    assert.ok(fs.existsSync(`${dbPath}.fallback.json`));

    const identity = getIdentity(dbPath, 'stateset', 'agent-001', { databaseCtor: null });
    assert.ok(identity);
    assert.equal(identity.agent_wallet, '0xFALLBACK');

    const byWallet = getIdentityByWallet(dbPath, '0xFALLBACK', { databaseCtor: null });
    assert.ok(byWallet);
    assert.equal(byWallet.agent_id, 'agent-001');
  });
});

// ---------------------------------------------------------------------------
// Process isolation: a file path is usually the native engine's live store, so
// SQLite must never open it in the calling process (see src/erc8004/index.js).
// ---------------------------------------------------------------------------

describe('process isolation', () => {
  beforeEach(setup);
  afterEach(cleanup);

  it('never loads better-sqlite3 or opens the store file in the calling process', () => {
    const moduleUrl = new URL('../../src/erc8004/index.js', import.meta.url).href;
    const script = `
      import fs from 'node:fs';
      import { createRequire } from 'node:module';
      const opened = [];
      for (const name of ['openSync', 'closeSync', 'writeFileSync', 'renameSync']) {
        const original = fs[name];
        fs[name] = (...args) => { opened.push(String(args[0])); return original(...args); };
      }
      const erc8004 = await import(${JSON.stringify(moduleUrl)});
      const dbPath = process.argv[1];
      erc8004.registerIdentity(dbPath, { agentRegistry: 'r', agentId: 'a', agentUri: 'u' });
      erc8004.setAgentWallet(dbPath, { agentRegistry: 'r', agentId: 'a', agentWallet: '0xw' });
      erc8004.getIdentity(dbPath, 'r', 'a');
      erc8004.getIdentityByWallet(dbPath, '0xw');
      const rows = erc8004.listIdentities(dbPath);
      const loaded = Object.keys(createRequire(import.meta.url).cache)
        .filter((key) => key.includes('better-sqlite3'));
      process.stdout.write(JSON.stringify({ rows: rows.length, loaded, opened }));
    `;
    const child = spawnSync(process.execPath, ['--input-type=module', '-e', script, dbPath], {
      encoding: 'utf8',
      timeout: 60_000,
    });
    assert.equal(child.status, 0, child.stderr);
    const report = JSON.parse(child.stdout);
    assert.equal(report.rows, 1);
    assert.deepEqual(report.loaded, []);
    assert.deepEqual(
      report.opened.filter((file) => file.startsWith(dbPath)),
      [],
    );
    assert.ok(fs.existsSync(dbPath), 'the worker created the store');
  });

  it('keeps :memory: in-process, where an injected constructor is still honoured', () => {
    const constructed = [];
    function FakeDatabase(file) {
      constructed.push(file);
      return {
        pragma() {},
        exec() {},
        prepare: () => ({ get: () => undefined }),
        close() {},
      };
    }
    assert.equal(getIdentity(':memory:', 'r', 'a', { databaseCtor: FakeDatabase }), null);
    assert.deepEqual(constructed, [':memory:']);
  });

  it('refuses an injected constructor for a file path instead of opening it here', () => {
    assert.throws(
      () => getIdentity(dbPath, 'r', 'a', { databaseCtor: function Database() {} }),
      /databaseCtor must be null or undefined for a file-backed ERC-8004 store/,
    );
    assert.equal(fs.existsSync(dbPath), false);
  });

  it('rethrows store errors with their message, name and code', () => {
    assert.throws(
      () =>
        setAgentWallet(dbPath, {
          agentRegistry: 'stateset',
          agentId: 'nonexistent',
          agentWallet: '0x123',
        }),
      (error) =>
        error.constructor === Error &&
        error.message === 'Agent identity not found for stateset:nonexistent',
    );
    assert.throws(() => registerIdentity(dbPath, { agentRegistry: 'r', agentId: 'a' }), {
      name: 'SqliteError',
      code: 'SQLITE_CONSTRAINT_NOTNULL',
      message: 'NOT NULL constraint failed: agent_identities.agent_uri',
    });
  });

  it('hands argument values to better-sqlite3 unchanged', () => {
    // A Date is not bindable; JSON would have turned it into a string.
    assert.throws(() => registerIdentity(dbPath, baseInput({ walletProofDeadline: new Date(0) })), {
      name: 'TypeError',
      message: /can only bind numbers, strings, bigints, buffers, and null/,
    });
    const stored = registerIdentity(dbPath, baseInput({ walletProofChainId: 8453n }));
    assert.equal(stored.wallet_proof_chain_id, 8453);
  });

  it('surfaces the JSON-fallback warning in the calling process', (t) => {
    const warn = t.mock.method(console, 'warn', () => {});
    registerIdentity(dbPath, baseInput(), { databaseCtor: null });
    assert.equal(warn.mock.callCount(), 1);
    assert.match(
      String(warn.mock.calls[0].arguments[0]),
      /^\[erc8004\] better-sqlite3 unavailable; using durable JSON fallback at .*identities\.db\.fallback\.json$/,
    );
  });
});
