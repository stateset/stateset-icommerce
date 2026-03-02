import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import {
  DEFAULT_STANDALONE_CONFIG,
  loadStandaloneConfig,
  saveStandaloneConfig,
  isStandaloneMode,
} from '../../src/config/standalone.js';

describe('standalone config', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-standalone-'));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  describe('DEFAULT_STANDALONE_CONFIG', () => {
    it('has expected default dbPath', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.dbPath, './store.db');
    });

    it('has sync disabled by default', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.sync.enabled, false);
    });

    it('has webhooks port 3000', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.webhooks.port, 3000);
    });

    it('has empty webhook sources', () => {
      assert.deepEqual(DEFAULT_STANDALONE_CONFIG.webhooks.sources, []);
    });

    it('has empty active adapters', () => {
      assert.deepEqual(DEFAULT_STANDALONE_CONFIG.adapters.active, []);
    });

    it('has policies dir set to ./policies', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.policies.dir, './policies');
    });

    it('has policies autoLoad enabled', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.policies.autoLoad, true);
    });

    it('has unknownDomainMode set to deny', () => {
      assert.equal(DEFAULT_STANDALONE_CONFIG.policies.unknownDomainMode, 'deny');
    });
  });

  describe('loadStandaloneConfig()', () => {
    it('returns defaults when no config file exists', () => {
      const config = loadStandaloneConfig(tmpDir);
      assert.equal(config.dbPath, DEFAULT_STANDALONE_CONFIG.dbPath);
      assert.equal(config.sync.enabled, false);
    });

    it('returns defaults when .stateset dir does not exist', () => {
      const config = loadStandaloneConfig(tmpDir);
      assert.deepEqual(config.webhooks.sources, []);
    });

    it('merges partial config with defaults', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'config.json'),
        JSON.stringify({ dbPath: './custom.db', webhooks: { port: 4000 } }),
      );

      const config = loadStandaloneConfig(tmpDir);
      assert.equal(config.dbPath, './custom.db');
      assert.equal(config.webhooks.port, 4000);
      assert.deepEqual(config.webhooks.sources, []);
    });

    it('handles malformed JSON gracefully', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(path.join(configDir, 'config.json'), '{not valid json}');

      const config = loadStandaloneConfig(tmpDir);
      assert.equal(config.dbPath, DEFAULT_STANDALONE_CONFIG.dbPath);
    });

    it('preserves custom adapters list', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'config.json'),
        JSON.stringify({ adapters: { active: ['stripe', 'shopify'] } }),
      );

      const config = loadStandaloneConfig(tmpDir);
      assert.deepEqual(config.adapters.active, ['stripe', 'shopify']);
    });

    it('preserves custom policy settings', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'config.json'),
        JSON.stringify({ policies: { dir: './rules', unknownDomainMode: 'allow' } }),
      );

      const config = loadStandaloneConfig(tmpDir);
      assert.equal(config.policies.dir, './rules');
      assert.equal(config.policies.unknownDomainMode, 'allow');
      assert.equal(config.policies.autoLoad, true); // default preserved
    });
  });

  describe('saveStandaloneConfig()', () => {
    it('creates .stateset directory if missing', () => {
      saveStandaloneConfig(DEFAULT_STANDALONE_CONFIG, tmpDir);
      assert.ok(fs.existsSync(path.join(tmpDir, '.stateset')));
    });

    it('writes valid JSON', () => {
      saveStandaloneConfig(DEFAULT_STANDALONE_CONFIG, tmpDir);
      const content = fs.readFileSync(path.join(tmpDir, '.stateset', 'config.json'), 'utf-8');
      const parsed = JSON.parse(content);
      assert.equal(parsed.dbPath, './store.db');
    });

    it('round-trips config correctly', () => {
      const custom = {
        ...DEFAULT_STANDALONE_CONFIG,
        dbPath: './mydata.db',
        webhooks: { port: 9000, sources: ['stripe'] },
      };
      saveStandaloneConfig(custom, tmpDir);
      const loaded = loadStandaloneConfig(tmpDir);
      assert.equal(loaded.dbPath, './mydata.db');
      assert.equal(loaded.webhooks.port, 9000);
      assert.deepEqual(loaded.webhooks.sources, ['stripe']);
    });

    it('overwrites existing config', () => {
      saveStandaloneConfig({ ...DEFAULT_STANDALONE_CONFIG, dbPath: './first.db' }, tmpDir);
      saveStandaloneConfig({ ...DEFAULT_STANDALONE_CONFIG, dbPath: './second.db' }, tmpDir);
      const loaded = loadStandaloneConfig(tmpDir);
      assert.equal(loaded.dbPath, './second.db');
    });
  });

  describe('isStandaloneMode()', () => {
    it('returns true when no sync.json exists', () => {
      assert.equal(isStandaloneMode(tmpDir), true);
    });

    it('returns false when sync.json exists', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(path.join(configDir, 'sync.json'), '{}');
      assert.equal(isStandaloneMode(tmpDir), false);
    });

    it('returns true when config.json has sync.enabled false', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(
        path.join(configDir, 'config.json'),
        JSON.stringify({ sync: { enabled: false } }),
      );
      assert.equal(isStandaloneMode(tmpDir), true);
    });

    it('returns false when sync.json exists even if config says disabled', () => {
      const configDir = path.join(tmpDir, '.stateset');
      fs.mkdirSync(configDir, { recursive: true });
      fs.writeFileSync(path.join(configDir, 'sync.json'), '{}');
      fs.writeFileSync(
        path.join(configDir, 'config.json'),
        JSON.stringify({ sync: { enabled: false } }),
      );
      assert.equal(isStandaloneMode(tmpDir), false);
    });
  });
});
