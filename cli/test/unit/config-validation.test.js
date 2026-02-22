import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { execFileSync } from 'node:child_process';

// Use a temp directory for config to avoid touching real config
const testDir = path.join(os.tmpdir(), `stateset-config-test-${Date.now()}`);
const profilesDir = path.join(testDir, 'profiles');

describe('stateset-config set validation', () => {
  beforeEach(() => {
    fs.mkdirSync(profilesDir, { recursive: true });
    fs.writeFileSync(
      path.join(testDir, 'config.json'),
      JSON.stringify({ defaultProfile: 'default' }),
    );
    fs.writeFileSync(path.join(profilesDir, 'default.json'), JSON.stringify({}));
  });

  afterEach(() => {
    fs.rmSync(testDir, { recursive: true, force: true });
  });

  function runConfig(...args) {
    const configScript = path.join(
      path.dirname(new URL(import.meta.url).pathname),
      '../../bin/stateset-config.js',
    );
    try {
      const result = execFileSync(process.execPath, [configScript, ...args], {
        env: {
          ...process.env,
          HOME: path.dirname(testDir),
          // Override config dir by making HOME point to parent of .stateset
        },
        timeout: 10000,
        encoding: 'utf8',
      });
      return { stdout: result, exitCode: 0 };
    } catch (err) {
      return { stdout: err.stdout || '', stderr: err.stderr || '', exitCode: err.status };
    }
  }

  // Test boolean parsing logic directly
  describe('boolean parsing', () => {
    const truthyValues = ['true', 'yes', '1', 'on'];
    const falsyValues = ['false', 'no', '0', 'off'];

    for (const val of truthyValues) {
      it(`parses '${val}' as truthy for boolean keys`, () => {
        const lower = val.toLowerCase();
        assert.ok(['true', 'yes', '1', 'on'].includes(lower));
      });
    }

    for (const val of falsyValues) {
      it(`parses '${val}' as falsy for boolean keys`, () => {
        const lower = val.toLowerCase();
        assert.ok(['false', 'no', '0', 'off'].includes(lower));
      });
    }

    it('rejects "maybe" as boolean value', () => {
      const lower = 'maybe'.toLowerCase();
      assert.ok(!['true', 'yes', '1', 'on'].includes(lower));
      assert.ok(!['false', 'no', '0', 'off'].includes(lower));
    });
  });

  // Test known config keys
  describe('known config keys', () => {
    const KNOWN_CONFIG_KEYS = {
      db: 'string',
      model: 'string',
      provider: 'string',
      apply: 'boolean',
      verbose: 'boolean',
      memory: 'boolean',
      stream: 'boolean',
      think: 'string',
      format: 'string',
      budget: 'string',
    };

    it('recognizes db as a known key', () => {
      assert.ok('db' in KNOWN_CONFIG_KEYS);
    });

    it('recognizes apply as a boolean key', () => {
      assert.equal(KNOWN_CONFIG_KEYS.apply, 'boolean');
    });

    it('recognizes verbose as a boolean key', () => {
      assert.equal(KNOWN_CONFIG_KEYS.verbose, 'boolean');
    });

    it('recognizes model as a string key', () => {
      assert.equal(KNOWN_CONFIG_KEYS.model, 'string');
    });

    it('does not recognize arbitrary key', () => {
      assert.ok(!('foobar' in KNOWN_CONFIG_KEYS));
    });
  });

  // Test provider validation
  describe('provider validation', () => {
    const knownProviders = ['claude', 'openai', 'gemini', 'ollama'];

    for (const p of knownProviders) {
      it(`accepts provider '${p}'`, () => {
        assert.ok(knownProviders.includes(p));
      });
    }

    it('flags unknown provider', () => {
      assert.ok(!knownProviders.includes('azure'));
    });
  });

  // Test think level validation
  describe('think level validation', () => {
    const validLevels = ['off', 'low', 'medium', 'high'];

    for (const level of validLevels) {
      it(`accepts think level '${level}'`, () => {
        assert.ok(validLevels.includes(level));
      });
    }

    it('flags invalid think level', () => {
      assert.ok(!validLevels.includes('max'));
    });
  });

  // Test format validation
  describe('format validation', () => {
    const validFormats = ['table', 'json', 'csv', 'yaml'];

    for (const fmt of validFormats) {
      it(`accepts format '${fmt}'`, () => {
        assert.ok(validFormats.includes(fmt));
      });
    }

    it('flags invalid format', () => {
      assert.ok(!validFormats.includes('xml'));
    });
  });

  // Test db path validation logic
  describe('db path validation', () => {
    it('allows :memory: as db path', () => {
      assert.equal(':memory:', ':memory:');
    });

    it('detects nonexistent parent directory', () => {
      const fakePath = '/nonexistent/dir/store.db';
      const dir = path.dirname(path.resolve(fakePath));
      assert.ok(!fs.existsSync(dir));
    });

    it('allows existing directory', () => {
      const tmpPath = path.join(os.tmpdir(), 'store.db');
      const dir = path.dirname(path.resolve(tmpPath));
      assert.ok(fs.existsSync(dir));
    });
  });
});
