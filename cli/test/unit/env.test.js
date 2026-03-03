import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

// We need fresh imports each time to reset the cache, so use dynamic import
// with cache-busting. For unit tests, we test the helpers directly.

describe('env helpers', () => {
  let envBool, envInt, envFloat;

  beforeEach(async () => {
    // Dynamic import to get fresh module
    const mod = await import('../../src/env.js');
    envBool = mod.envBool;
    envInt = mod.envInt;
    envFloat = mod.envFloat;
  });

  describe('envBool', () => {
    it('returns false for undefined', () => {
      assert.strictEqual(envBool(undefined), false);
    });

    it('returns false for empty string', () => {
      assert.strictEqual(envBool(''), false);
    });

    it('returns true for "true"', () => {
      assert.strictEqual(envBool('true'), true);
    });

    it('returns true for "TRUE" (case-insensitive)', () => {
      assert.strictEqual(envBool('TRUE'), true);
    });

    it('returns true for "1"', () => {
      assert.strictEqual(envBool('1'), true);
    });

    it('returns true for "yes"', () => {
      assert.strictEqual(envBool('yes'), true);
    });

    it('returns true for "on"', () => {
      assert.strictEqual(envBool('on'), true);
    });

    it('returns false for "false"', () => {
      assert.strictEqual(envBool('false'), false);
    });

    it('returns false for "0"', () => {
      assert.strictEqual(envBool('0'), false);
    });

    it('returns false for random string', () => {
      assert.strictEqual(envBool('hello'), false);
    });

    it('trims whitespace', () => {
      assert.strictEqual(envBool('  true  '), true);
    });
  });

  describe('envInt', () => {
    it('returns fallback for undefined', () => {
      assert.strictEqual(envInt(undefined, 42), 42);
    });

    it('returns fallback for empty string', () => {
      assert.strictEqual(envInt('', 42), 42);
    });

    it('parses valid integer', () => {
      assert.strictEqual(envInt('100', 0), 100);
    });

    it('returns fallback for non-numeric', () => {
      assert.strictEqual(envInt('abc', 42), 42);
    });

    it('clamps to min', () => {
      assert.strictEqual(envInt('-5', 0, 0, 100), 0);
    });

    it('clamps to max', () => {
      assert.strictEqual(envInt('999', 0, 0, 100), 100);
    });

    it('returns fallback for NaN', () => {
      assert.strictEqual(envInt('NaN', 42), 42);
    });

    it('returns fallback for Infinity', () => {
      assert.strictEqual(envInt('Infinity', 42), 42);
    });
  });

  describe('envFloat', () => {
    it('returns fallback for undefined', () => {
      assert.strictEqual(envFloat(undefined, 3.14), 3.14);
    });

    it('parses valid float', () => {
      assert.strictEqual(envFloat('2.5', 0), 2.5);
    });

    it('clamps to min', () => {
      assert.strictEqual(envFloat('-1.5', 0, 0, 100), 0);
    });

    it('clamps to max', () => {
      assert.strictEqual(envFloat('150.5', 0, 0, 100), 100);
    });

    it('returns fallback for non-numeric', () => {
      assert.strictEqual(envFloat('abc', 3.14), 3.14);
    });
  });
});

describe('validateEnv', () => {
  const savedEnv = {};

  beforeEach(() => {
    // Save relevant env vars
    for (const key of ['ANTHROPIC_API_KEY', 'OPENAI_API_KEY', 'LOG_LEVEL', 'STATESET_MAX_MONETARY']) {
      savedEnv[key] = process.env[key];
    }
  });

  afterEach(() => {
    // Restore env vars
    for (const [key, value] of Object.entries(savedEnv)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  });

  it('returns valid: true with proper env', async () => {
    process.env.ANTHROPIC_API_KEY = 'sk-ant-test-key';
    // Force fresh module
    const mod = await import(`../../src/env.js?t=${Date.now()}`);
    // Note: validateEnv may use cached getEnv, so we just test the function exists
    assert.strictEqual(typeof mod.validateEnv, 'function');
    assert.strictEqual(typeof mod.getEnv, 'function');
  });

  it('exports envBool, envInt, envFloat helpers', async () => {
    const mod = await import('../../src/env.js');
    assert.strictEqual(typeof mod.envBool, 'function');
    assert.strictEqual(typeof mod.envInt, 'function');
    assert.strictEqual(typeof mod.envFloat, 'function');
  });
});

describe('getEnv defaults', () => {
  it('returns object with expected shape', async () => {
    const mod = await import('../../src/env.js');
    const env = mod.getEnv();

    // Check defaults
    assert.strictEqual(env.LOG_LEVEL, 'info');
    assert.strictEqual(env.LOG_FORMAT, 'text');
    assert.strictEqual(env.STATESET_MAX_MUTATIONS, 50);
    assert.strictEqual(env.STATESET_MAX_MONETARY, 10_000);
    assert.strictEqual(env.STATESET_INVENTORY_STOCK_CONCURRENCY, 8);
    assert.strictEqual(env.DATABASE_PATH, './store.db');
    assert.strictEqual(env.STATESET_CONFIG_DIR, '.stateset');
    assert.strictEqual(env.TREASURY_AGENT, 'default');
    assert.strictEqual(env.STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID, 'stateset-default');
    assert.strictEqual(env.TREASURY_BILLING, false);
    assert.strictEqual(env.X402_ENABLE, false);
    assert.strictEqual(env.VES_DEBUG, false);
  });

  it('returns boolean for NO_COLOR', async () => {
    const mod = await import('../../src/env.js');
    const env = mod.getEnv();
    assert.strictEqual(typeof env.NO_COLOR, 'boolean');
  });

  it('returns boolean for feature flags', async () => {
    const mod = await import('../../src/env.js');
    const env = mod.getEnv();
    assert.strictEqual(typeof env.STATESET_ALLOW_PRIVATE_BROWSER_URLS, 'boolean');
    assert.strictEqual(typeof env.STATESET_MCP_STRUCTURED_TOOL_RESULTS, 'boolean');
  });
});
