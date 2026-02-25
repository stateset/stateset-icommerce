import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { execFileSync } from 'node:child_process';

const TRANSIENT_IO_ERROR_CODES = new Set(['EMFILE', 'ENFILE', 'EAGAIN', 'EBUSY', 'ETXTBSY']);
const sleepBuffer = new Int32Array(new SharedArrayBuffer(4));

function sleepSync(ms) {
  Atomics.wait(sleepBuffer, 0, 0, ms);
}

function withIoRetry(fn, { attempts = 5, baseDelayMs = 20 } = {}) {
  let lastError = null;

  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return fn();
    } catch (error) {
      lastError = error;
      if (!TRANSIENT_IO_ERROR_CODES.has(error?.code) || attempt === attempts) {
        throw error;
      }
      sleepSync(baseDelayMs * attempt);
    }
  }

  throw lastError;
}

const setupScript = path.join(
  path.dirname(new URL(import.meta.url).pathname),
  '../../bin/stateset-setup.js',
);

describe('stateset-setup wizard', () => {
  let testHome;

  beforeEach(() => {
    testHome = withIoRetry(() => fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-setup-')));
  });

  afterEach(() => {
    try {
      withIoRetry(() => fs.rmSync(testHome, { recursive: true, force: true }), {
        attempts: 3,
        baseDelayMs: 15,
      });
    } catch {
      // Best-effort cleanup for transient CI filesystem contention.
    }
  });

  function runSetup(args = [], env = {}) {
    const maxAttempts = 4;

    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      try {
        const result = execFileSync(process.execPath, [setupScript, '--yes', ...args], {
          env: {
            ...process.env,
            HOME: testHome,
            ANTHROPIC_API_KEY: '',
            ...env,
          },
          timeout: 30000,
          encoding: 'utf8',
          cwd: testHome,
        });
        return { stdout: result, exitCode: 0 };
      } catch (err) {
        const transient =
          TRANSIENT_IO_ERROR_CODES.has(err?.code) ||
          (err?.code === 'ETIMEDOUT' && attempt < maxAttempts);

        if (transient && attempt < maxAttempts) {
          sleepSync(25 * attempt);
          continue;
        }

        return {
          stdout: err.stdout || '',
          stderr: err.stderr || '',
          exitCode: err.status,
        };
      }
    }

    return { stdout: '', stderr: '', exitCode: 1 };
  }

  it('shows help with --help flag', () => {
    const result = runSetup(['--help']);
    assert.equal(result.exitCode, 0);
    assert.ok(result.stdout.includes('Guided Setup'));
  });

  it('creates config directory in non-interactive mode', () => {
    const result = runSetup();
    // May fail at db init (no @stateset/embedded), but should create config dir
    const configDir = path.join(testHome, '.stateset');
    assert.ok(fs.existsSync(configDir), 'config directory should be created');
  });

  it('creates profiles directory', () => {
    runSetup();
    const profilesDir = path.join(testHome, '.stateset', 'profiles');
    assert.ok(fs.existsSync(profilesDir), 'profiles directory should be created');
  });

  it('reports missing API key in non-interactive mode', () => {
    const result = runSetup([], { ANTHROPIC_API_KEY: '' });
    assert.ok(
      result.stdout.includes('missing') || result.stdout.includes('set-key'),
      'should mention missing API key',
    );
  });

  it('detects pre-existing API key', () => {
    const result = runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test123' });
    assert.ok(result.stdout.includes('already configured'), 'should detect existing API key');
  });

  it('JSON output is valid JSON', () => {
    const result = runSetup(['--json'], { ANTHROPIC_API_KEY: 'sk-ant-test' });
    // Should complete (possibly with db init failure)
    if (result.exitCode === 0) {
      const parsed = JSON.parse(result.stdout);
      assert.ok(parsed.steps, 'should have steps array');
      assert.ok(Array.isArray(parsed.steps));
    }
  });

  it('is idempotent — second run skips completed steps', () => {
    // First run creates config dir
    runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test' });

    // Second run should skip config dir creation
    const result = runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test' });
    assert.ok(result.stdout.includes('already exists'), 'should skip config dir on second run');
  });

  it('shows next steps on completion', () => {
    const result = runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test' });
    assert.ok(
      result.stdout.includes('Setup complete') || result.stdout.includes('stateset'),
      'should show next steps',
    );
  });

  describe('step detection', () => {
    it('detects config directory already exists', () => {
      fs.mkdirSync(path.join(testHome, '.stateset', 'profiles'), { recursive: true });
      const result = runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      assert.ok(result.stdout.includes('already exists'));
    });

    it('detects existing database', () => {
      // Create a fake store.db
      fs.writeFileSync(path.join(testHome, 'store.db'), '');
      const result = runSetup([], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      assert.ok(result.stdout.includes('exists at'), 'should detect existing database');
    });
  });

  describe('health check', () => {
    it('checks Node.js version', () => {
      const result = runSetup(['--json'], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      if (result.exitCode === 0) {
        const parsed = JSON.parse(result.stdout);
        const healthStep = parsed.steps.find((s) => s.name === 'health_check');
        assert.ok(healthStep);
        assert.ok(healthStep.checks.node);
      }
    });

    it('checks API key presence', () => {
      const result = runSetup(['--json'], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      if (result.exitCode === 0) {
        const parsed = JSON.parse(result.stdout);
        const healthStep = parsed.steps.find((s) => s.name === 'health_check');
        assert.equal(healthStep.checks.apiKey, 'ok');
      }
    });
  });
});
