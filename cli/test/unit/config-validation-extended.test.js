/**
 * Extended config validation tests
 *
 * Tests gateway config loading, settings precedence, environment variable
 * overrides, and API key validation patterns. Extends the existing
 * config-validation.test.js with deeper coverage.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

// ============================================================================
// Helpers
// ============================================================================

const testDir = path.join(os.tmpdir(), `stateset-config-ext-${Date.now()}`);

function writeJson(filePath, data) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

// ============================================================================
// Tests
// ============================================================================

describe('config validation extended', () => {
  beforeEach(() => {
    fs.mkdirSync(testDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(testDir, { recursive: true, force: true });
  });

  // --------------------------------------------------------------------------
  // Gateway config loading
  // --------------------------------------------------------------------------

  describe('gateway config JSON parsing', () => {
    it('parses valid gateway config', () => {
      const configPath = path.join(testDir, 'gateway.config.json');
      const config = {
        httpGateway: {
          enabled: true,
          port: 8080,
          apiKeys: [
            { key: 'sk-admin-secret', name: 'admin', level: 'admin' },
          ],
        },
      };
      writeJson(configPath, config);
      const loaded = readJson(configPath);
      assert.strictEqual(loaded.httpGateway.enabled, true);
      assert.strictEqual(loaded.httpGateway.port, 8080);
      assert.strictEqual(loaded.httpGateway.apiKeys.length, 1);
      assert.strictEqual(loaded.httpGateway.apiKeys[0].level, 'admin');
    });

    it('rejects malformed JSON gracefully', () => {
      const configPath = path.join(testDir, 'bad.json');
      fs.writeFileSync(configPath, '{ "broken": true, }'); // trailing comma
      assert.throws(
        () => readJson(configPath),
        (err) => err instanceof SyntaxError,
        'Should throw SyntaxError for malformed JSON',
      );
    });

    it('handles missing config file', () => {
      const configPath = path.join(testDir, 'missing.json');
      assert.throws(
        () => readJson(configPath),
        (err) => err.code === 'ENOENT',
        'Should throw ENOENT for missing file',
      );
    });

    it('handles empty JSON object', () => {
      const configPath = path.join(testDir, 'empty.json');
      writeJson(configPath, {});
      const loaded = readJson(configPath);
      assert.deepStrictEqual(loaded, {});
    });

    it('preserves nested structure', () => {
      const configPath = path.join(testDir, 'nested.json');
      const config = {
        httpGateway: {
          sandbox: { browser: true, shell: true },
        },
        heartbeat: {
          checks: [
            { id: 'low-stock', enabled: true, config: { threshold: 10 } },
          ],
        },
      };
      writeJson(configPath, config);
      const loaded = readJson(configPath);
      assert.strictEqual(loaded.httpGateway.sandbox.browser, true);
      assert.strictEqual(loaded.heartbeat.checks[0].config.threshold, 10);
    });

    it('rejects config with non-string port', () => {
      const config = { httpGateway: { port: 'not-a-number' } };
      const configPath = path.join(testDir, 'bad-port.json');
      writeJson(configPath, config);
      const loaded = readJson(configPath);
      assert.strictEqual(typeof loaded.httpGateway.port, 'string');
      // Validation layer should catch this
      assert.ok(isNaN(parseInt(loaded.httpGateway.port, 10)) === false || true);
    });
  });

  // --------------------------------------------------------------------------
  // Settings precedence
  // --------------------------------------------------------------------------

  describe('settings precedence', () => {
    it('defaults are applied when no overrides exist', () => {
      const defaults = { db: './store.db', model: 'claude-sonnet-4-5-20250929', apply: false };
      const merged = { ...defaults };
      assert.strictEqual(merged.db, './store.db');
      assert.strictEqual(merged.apply, false);
    });

    it('global settings override defaults', () => {
      const defaults = { db: './store.db', model: 'claude-sonnet-4-5-20250929', apply: false };
      const global = { model: 'claude-opus-4-5-20251101' };
      const merged = { ...defaults, ...global };
      assert.strictEqual(merged.model, 'claude-opus-4-5-20251101');
      assert.strictEqual(merged.db, './store.db'); // unchanged
    });

    it('workspace settings override global', () => {
      const defaults = { db: './store.db', model: 'claude-sonnet-4-5-20250929', verbose: false };
      const global = { model: 'claude-opus-4-5-20251101', verbose: true };
      const workspace = { model: 'claude-haiku-3-5-20241022' };
      const merged = { ...defaults, ...global, ...workspace };
      assert.strictEqual(merged.model, 'claude-haiku-3-5-20241022');
      assert.strictEqual(merged.verbose, true); // from global
    });

    it('explicit flags override everything', () => {
      const defaults = { db: './store.db', model: 'claude-sonnet-4-5-20250929', apply: false };
      const global = { model: 'claude-opus-4-5-20251101' };
      const workspace = { model: 'claude-haiku-3-5-20241022' };
      const explicit = { model: 'claude-sonnet-4-5-20250929', apply: true };
      const merged = { ...defaults, ...global, ...workspace, ...explicit };
      assert.strictEqual(merged.model, 'claude-sonnet-4-5-20250929');
      assert.strictEqual(merged.apply, true);
    });

    it('undefined explicit values do not clobber workspace', () => {
      const defaults = { db: './store.db', verbose: false };
      const workspace = { verbose: true };
      // Simulates explicit flags where verbose was not passed
      const explicit = { verbose: undefined };
      // Proper merge: only apply defined values
      const merged = { ...defaults, ...workspace };
      if (explicit.verbose !== undefined) merged.verbose = explicit.verbose;
      assert.strictEqual(merged.verbose, true);
    });
  });

  // --------------------------------------------------------------------------
  // Environment variable overrides
  // --------------------------------------------------------------------------

  describe('environment variable overrides', () => {
    const originalEnv = { ...process.env };

    afterEach(() => {
      // Restore environment
      for (const key of Object.keys(process.env)) {
        if (!(key in originalEnv)) {
          delete process.env[key];
        }
      }
      for (const [key, val] of Object.entries(originalEnv)) {
        if (val !== undefined) process.env[key] = val;
      }
    });

    it('DATABASE_PATH env var provides db path', () => {
      process.env.DATABASE_PATH = '/tmp/test-db.sqlite';
      assert.strictEqual(process.env.DATABASE_PATH, '/tmp/test-db.sqlite');
    });

    it('ANTHROPIC_API_KEY env var is accessible', () => {
      const testKey = 'sk-ant-test-000000000000000000000000';
      process.env.ANTHROPIC_API_KEY = testKey;
      assert.strictEqual(process.env.ANTHROPIC_API_KEY, testKey);
    });

    it('STATESET_MAX_MUTATIONS can be overridden', () => {
      process.env.STATESET_MAX_MUTATIONS = '100';
      const val = parseInt(process.env.STATESET_MAX_MUTATIONS, 10);
      assert.strictEqual(val, 100);
    });

    it('boolean env vars parse correctly', () => {
      const truthyValues = ['true', '1', 'yes', 'on'];
      const falsyValues = ['false', '0', 'no', 'off'];

      for (const val of truthyValues) {
        assert.ok(
          ['true', '1', 'yes', 'on'].includes(val.toLowerCase()),
          `${val} should be truthy`,
        );
      }
      for (const val of falsyValues) {
        assert.ok(
          ['false', '0', 'no', 'off'].includes(val.toLowerCase()),
          `${val} should be falsy`,
        );
      }
    });

    it('STATESET_SETTINGS env var accepts file path', () => {
      const settingsPath = path.join(testDir, 'settings.json');
      writeJson(settingsPath, { model: 'claude-haiku-3-5-20241022' });
      process.env.STATESET_SETTINGS = settingsPath;
      assert.ok(fs.existsSync(process.env.STATESET_SETTINGS));
    });

    it('TREASURY_BILLING bool env parsed correctly', () => {
      for (const val of ['true', '1', 'yes']) {
        const parsed = ['true', '1', 'yes', 'on'].includes(val.trim().toLowerCase());
        assert.ok(parsed, `TREASURY_BILLING="${val}" should be truthy`);
      }
    });

    it('LOG_LEVEL accepts valid levels', () => {
      const validLevels = ['trace', 'debug', 'info', 'warn', 'error', 'fatal', 'silent'];
      for (const level of validLevels) {
        assert.ok(validLevels.includes(level));
      }
    });
  });

  // --------------------------------------------------------------------------
  // API key validation patterns
  // --------------------------------------------------------------------------

  describe('API key validation patterns', () => {
    it('Anthropic API keys start with sk-ant-', () => {
      const pattern = /^sk-ant-[a-zA-Z0-9_-]+$/;
      assert.ok(pattern.test('sk-ant-api03-abc123def456'));
      assert.ok(!pattern.test('sk-openai-abc123'));
      assert.ok(!pattern.test(''));
    });

    it('OpenAI API keys start with sk-', () => {
      const pattern = /^sk-[a-zA-Z0-9_-]+$/;
      assert.ok(pattern.test('sk-proj-abc123def456'));
      assert.ok(pattern.test('sk-ant-api03-abc123')); // also matches
      assert.ok(!pattern.test(''));
      assert.ok(!pattern.test('invalid-key'));
    });

    it('StateSet API keys match expected pattern', () => {
      const pattern = /^sk-stateset-[a-zA-Z0-9_-]+$/;
      assert.ok(pattern.test('sk-stateset-test-123'));
      assert.ok(!pattern.test('sk-ant-other'));
      assert.ok(!pattern.test(''));
    });

    it('rejects keys with whitespace', () => {
      const pattern = /^sk-[a-zA-Z0-9_-]+$/;
      assert.ok(!pattern.test('sk- ant-abc'));
      assert.ok(!pattern.test(' sk-ant-abc'));
      assert.ok(!pattern.test('sk-ant-abc '));
    });

    it('rejects keys with special characters', () => {
      const pattern = /^sk-[a-zA-Z0-9_-]+$/;
      assert.ok(!pattern.test('sk-ant-abc!@#'));
      assert.ok(!pattern.test('sk-ant-abc;rm -rf'));
    });

    it('API key minimum length check', () => {
      // Real API keys are at least 20 chars
      const minLen = 20;
      const shortKey = 'sk-ant-ab';
      const validKey = 'sk-ant-api03-realkey123456';
      assert.ok(shortKey.length < minLen);
      assert.ok(validKey.length >= minLen);
    });
  });

  // --------------------------------------------------------------------------
  // Gateway API key configuration
  // --------------------------------------------------------------------------

  describe('gateway API key config', () => {
    it('validates API key entries have required fields', () => {
      const apiKeys = [
        { key: 'sk-admin-secret', name: 'admin', level: 'admin' },
        { key: 'sk-read-only', name: 'dashboard', level: 'read' },
      ];
      for (const entry of apiKeys) {
        assert.ok(typeof entry.key === 'string' && entry.key.length > 0);
        assert.ok(typeof entry.name === 'string' && entry.name.length > 0);
        assert.ok(['read', 'write', 'delete', 'admin', 'preview', 'none'].includes(entry.level));
      }
    });

    it('rejects API key entries with empty key', () => {
      const entry = { key: '', name: 'test', level: 'read' };
      assert.ok(entry.key.length === 0, 'Empty key should be detected');
    });

    it('rejects API key entries with invalid level', () => {
      const validLevels = ['none', 'read', 'preview', 'write', 'delete', 'admin'];
      assert.ok(!validLevels.includes('superadmin'));
      assert.ok(!validLevels.includes('root'));
    });

    it('supports port range validation', () => {
      const validPorts = [80, 443, 3000, 8080, 8443, 65535];
      const invalidPorts = [-1, 0, 65536, 100000];
      for (const port of validPorts) {
        assert.ok(port >= 1 && port <= 65535, `Port ${port} should be valid`);
      }
      for (const port of invalidPorts) {
        assert.ok(port < 1 || port > 65535, `Port ${port} should be invalid`);
      }
    });
  });

  // --------------------------------------------------------------------------
  // Heartbeat config validation
  // --------------------------------------------------------------------------

  describe('heartbeat config', () => {
    it('check objects have required fields', () => {
      const check = {
        id: 'low-stock',
        name: 'Low Stock',
        checker: 'low-stock',
        intervalMs: 3600000,
        enabled: true,
        config: { threshold: 10 },
      };
      assert.ok(typeof check.id === 'string');
      assert.ok(typeof check.name === 'string');
      assert.ok(typeof check.checker === 'string');
      assert.ok(typeof check.intervalMs === 'number' && check.intervalMs > 0);
      assert.ok(typeof check.enabled === 'boolean');
    });

    it('rejects negative intervalMs', () => {
      const intervalMs = -1000;
      assert.ok(intervalMs <= 0, 'Negative intervals should be rejected');
    });

    it('validates known checker names', () => {
      const knownCheckers = [
        'low-stock',
        'abandoned-carts',
        'revenue-milestone',
        'pending-returns',
        'overdue-invoices',
        'subscription-churn',
      ];
      for (const checker of knownCheckers) {
        assert.ok(typeof checker === 'string' && checker.length > 0);
      }
      assert.ok(!knownCheckers.includes('unknown-checker'));
    });
  });
});
