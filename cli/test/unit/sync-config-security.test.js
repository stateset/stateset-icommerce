import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import os from 'os';
import path from 'path';

import { createSyncConfig } from '../../src/sync/config.js';

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
