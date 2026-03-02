import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import {
  TIERS,
  detectTier,
  getTierCapabilities,
  getTierLabel,
  hasCapability,
} from '../../src/tiers.js';

describe('tier detection', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-tiers-'));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  describe('TIERS constants', () => {
    it('defines STANDALONE', () => {
      assert.equal(TIERS.STANDALONE, 'standalone');
    });

    it('defines SEQUENCER', () => {
      assert.equal(TIERS.SEQUENCER, 'sequencer');
    });

    it('defines FULL', () => {
      assert.equal(TIERS.FULL, 'full');
    });

    it('is frozen', () => {
      assert.throws(() => {
        TIERS.NEW = 'new';
      });
    });
  });

  describe('detectTier()', () => {
    it('returns STANDALONE when no .stateset dir exists', () => {
      assert.equal(detectTier(tmpDir), TIERS.STANDALONE);
    });

    it('returns STANDALONE when .stateset exists but no sync.json', () => {
      fs.mkdirSync(path.join(tmpDir, '.stateset'), { recursive: true });
      assert.equal(detectTier(tmpDir), TIERS.STANDALONE);
    });

    it('returns SEQUENCER when sync.json exists without chain config', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'sync.json'),
        JSON.stringify({ sequencer: { url: 'grpc://localhost:50051' } }),
      );
      assert.equal(detectTier(tmpDir), TIERS.SEQUENCER);
    });

    it('returns FULL when sync.json has chain.rpcUrl', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'sync.json'),
        JSON.stringify({
          sequencer: { url: 'grpc://localhost:50051' },
          chain: { rpcUrl: 'https://rpc.setchain.io' },
        }),
      );
      assert.equal(detectTier(tmpDir), TIERS.FULL);
    });

    it('returns FULL when sync.json has settlement.rpcUrl', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'sync.json'),
        JSON.stringify({ settlement: { rpcUrl: 'https://mainnet.base.org' } }),
      );
      assert.equal(detectTier(tmpDir), TIERS.FULL);
    });

    it('returns FULL when sync.json has anchor.l2RpcUrl', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'sync.json'),
        JSON.stringify({ anchor: { l2RpcUrl: 'https://rpc.setchain.io' } }),
      );
      assert.equal(detectTier(tmpDir), TIERS.FULL);
    });

    it('returns SEQUENCER for malformed sync.json', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(path.join(configDir, 'sync.json'), 'not json');
      assert.equal(detectTier(tmpDir), TIERS.SEQUENCER);
    });

    it('returns SEQUENCER for empty sync.json object', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(path.join(configDir, 'sync.json'), '{}');
      assert.equal(detectTier(tmpDir), TIERS.SEQUENCER);
    });
  });

  describe('getTierCapabilities()', () => {
    it('standalone tier includes commerce basics', () => {
      const caps = getTierCapabilities(TIERS.STANDALONE);
      assert.ok(caps.includes('commerce'));
      assert.ok(caps.includes('policies'));
      assert.ok(caps.includes('adapters'));
      assert.ok(caps.includes('webhooks'));
      assert.ok(caps.includes('analytics'));
    });

    it('standalone tier does NOT include sync', () => {
      const caps = getTierCapabilities(TIERS.STANDALONE);
      assert.ok(!caps.includes('sync'));
      assert.ok(!caps.includes('chain'));
      assert.ok(!caps.includes('x402'));
    });

    it('sequencer tier includes sync and multi-agent', () => {
      const caps = getTierCapabilities(TIERS.SEQUENCER);
      assert.ok(caps.includes('sync'));
      assert.ok(caps.includes('crypto'));
      assert.ok(caps.includes('multi-agent'));
      assert.ok(caps.includes('audit-trail'));
    });

    it('sequencer tier does NOT include chain features', () => {
      const caps = getTierCapabilities(TIERS.SEQUENCER);
      assert.ok(!caps.includes('chain'));
      assert.ok(!caps.includes('x402'));
      assert.ok(!caps.includes('stablecoin'));
    });

    it('full tier includes everything', () => {
      const caps = getTierCapabilities(TIERS.FULL);
      assert.ok(caps.includes('commerce'));
      assert.ok(caps.includes('sync'));
      assert.ok(caps.includes('chain'));
      assert.ok(caps.includes('x402'));
      assert.ok(caps.includes('stablecoin'));
      assert.ok(caps.includes('anchoring'));
      assert.ok(caps.includes('stark-proofs'));
    });
  });

  describe('getTierLabel()', () => {
    it('returns label for standalone', () => {
      assert.equal(getTierLabel(TIERS.STANDALONE), 'iCommerce Standalone');
    });

    it('returns label for sequencer', () => {
      assert.equal(getTierLabel(TIERS.SEQUENCER), 'iCommerce + Sequencer');
    });

    it('returns label for full', () => {
      assert.match(getTierLabel(TIERS.FULL), /Full Trilogy/);
    });

    it('returns Unknown for invalid tier', () => {
      assert.equal(getTierLabel('invalid'), 'Unknown');
    });
  });

  describe('hasCapability()', () => {
    it('reports commerce as available in standalone', () => {
      assert.equal(hasCapability('commerce', tmpDir), true);
    });

    it('reports sync as unavailable in standalone', () => {
      assert.equal(hasCapability('sync', tmpDir), false);
    });

    it('reports sync as available when sync.json exists', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'sync.json'),
        JSON.stringify({ sequencer: { url: 'grpc://localhost:50051' } }),
      );
      assert.equal(hasCapability('sync', tmpDir), true);
    });
  });
});
