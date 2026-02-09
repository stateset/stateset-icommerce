/**
 * Tests for cli/src/erc8004/index.js
 *
 * Covers: registerIdentity, setAgentWallet, getIdentity,
 * getIdentityByWallet, listIdentities.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
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
});
