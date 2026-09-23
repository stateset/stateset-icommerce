import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import os from 'os';
import path from 'path';

import {
  createSyncConfig,
  loadSyncConfig,
  normalizeSequencerPublicKey,
  updateSyncConfig,
  validateSyncConfig,
} from '../../src/sync/config.js';

const tempDirs = [];

function makeTempDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-sync-config-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    fs.rmSync(tempDirs.pop(), { recursive: true, force: true });
  }
});

describe('createSyncConfig security defaults', () => {
  it('defaults new configs to hybrid on secure sequencer URLs', () => {
    const config = createSyncConfig(
      {
        sequencerUrl: 'https://sequencer.example.com',
        tenantId: '550e8400-e29b-41d4-a716-446655440001',
        storeId: '550e8400-e29b-41d4-a716-446655440002',
      },
      makeTempDir(),
    );

    assert.equal(config.sync.securityProfile, 'hybrid');
    assert.equal(config.sequencer.tls, true);
    assert.equal(config.sequencer.insecure, false);
  });

  it('rejects insecure legacy transport unless explicitly allowed', () => {
    assert.throws(
      () =>
        createSyncConfig(
          {
            sequencerUrl: 'http://localhost:50051',
            tenantId: '550e8400-e29b-41d4-a716-446655440001',
            storeId: '550e8400-e29b-41d4-a716-446655440002',
            securityProfile: 'legacy',
          },
          makeTempDir(),
        ),
      /explicitly allowed/,
    );
  });

  it('allows insecure legacy transport when explicitly requested', () => {
    const config = createSyncConfig(
      {
        sequencerUrl: 'http://localhost:50051',
        tenantId: '550e8400-e29b-41d4-a716-446655440001',
        storeId: '550e8400-e29b-41d4-a716-446655440002',
        securityProfile: 'legacy',
        allowInsecureTransport: true,
      },
      makeTempDir(),
    );

    assert.equal(config.sync.securityProfile, 'legacy');
    assert.equal(config.sequencer.tls, false);
    assert.equal(config.sequencer.insecure, true);
  });
});

/**
 * `sequencerPublicKey` had no supported way to be set: `init` did not accept
 * it, `updateSyncConfig` had no callers, and its encoding was undocumented. On
 * a stock deployment that made the receive path store nothing.
 */
describe('sequencerPublicKey configuration', () => {
  const IDENTITY = {
    sequencerUrl: 'https://sequencer.example.com',
    tenantId: '550e8400-e29b-41d4-a716-446655440001',
    storeId: '550e8400-e29b-41d4-a716-446655440002',
  };
  const RAW_HEX = 'a'.repeat(64);

  it('normalizes hex with or without the 0x prefix, and Buffers', () => {
    assert.equal(normalizeSequencerPublicKey(RAW_HEX), `0x${RAW_HEX}`);
    assert.equal(normalizeSequencerPublicKey(`0x${RAW_HEX.toUpperCase()}`), `0x${RAW_HEX}`);
    assert.equal(normalizeSequencerPublicKey(Buffer.from(RAW_HEX, 'hex')), `0x${RAW_HEX}`);
    assert.equal(normalizeSequencerPublicKey(null), null);
    assert.equal(normalizeSequencerPublicKey(undefined), null);
  });

  it('refuses a key that is not 32 raw bytes', () => {
    assert.throws(() => normalizeSequencerPublicKey('deadbeef'), /64 hex characters/);
    assert.throws(() => normalizeSequencerPublicKey({ ed25519PublicKey: RAW_HEX }), /hex string/);
  });

  it('is written by createSyncConfig and read back by loadSyncConfig', () => {
    const dir = makeTempDir();
    const created = createSyncConfig({ ...IDENTITY, sequencerPublicKey: RAW_HEX }, dir);

    assert.equal(created.sequencerPublicKey, `0x${RAW_HEX}`);
    assert.equal(loadSyncConfig(dir).sequencerPublicKey, `0x${RAW_HEX}`);
  });

  it('can be set afterwards through updateSyncConfig, the way `config set` does', () => {
    const dir = makeTempDir();
    const created = createSyncConfig(IDENTITY, dir);
    assert.equal(created.sequencerPublicKey, null, 'unset is still the default');

    const updated = updateSyncConfig({ sequencerPublicKey: `0x${RAW_HEX}` }, dir);
    assert.equal(updated.sequencerPublicKey, `0x${RAW_HEX}`);
    assert.equal(loadSyncConfig(dir).sequencerPublicKey, `0x${RAW_HEX}`);
  });

  it('defaults peerKeyTtlSeconds to the 300s the spec specifies', () => {
    const config = createSyncConfig(IDENTITY, makeTempDir());
    assert.equal(config.peerKeyTtlSeconds, 300);
    assert.equal(config.peerKeyMaxStaleSeconds, 86400);
  });

  it('accepts per-agent overrides of both peer-key windows', () => {
    const config = createSyncConfig(
      { ...IDENTITY, peerKeyTtlSeconds: 60, peerKeyMaxStaleSeconds: 120 },
      makeTempDir(),
    );
    assert.equal(config.peerKeyTtlSeconds, 60);
    assert.equal(config.peerKeyMaxStaleSeconds, 120);
  });

  it('reports a malformed key through validateSyncConfig', () => {
    const dir = makeTempDir();
    const config = createSyncConfig(IDENTITY, dir);
    assert.equal(validateSyncConfig(config).valid, true, 'unset is valid: push-only agents exist');

    const broken = { ...config, sequencerPublicKey: 'not-a-key' };
    const result = validateSyncConfig(broken);
    assert.equal(result.valid, false);
    assert.ok(result.errors.some((error) => /64 hex characters/.test(error)));
  });
});
