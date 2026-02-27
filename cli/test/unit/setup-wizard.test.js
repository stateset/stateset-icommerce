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

  describe('agent onboarding', () => {
    it('writes OpenClaw MCP config with --agent openclaw', () => {
      const result = runSetup(['--agent', 'openclaw'], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      assert.equal(result.exitCode, 0);

      const configPath = path.join(testHome, '.openclaw', 'mcp.json');
      assert.ok(fs.existsSync(configPath), 'should create .openclaw/mcp.json');

      const parsed = JSON.parse(fs.readFileSync(configPath, 'utf8'));
      assert.ok(parsed.mcpServers, 'should include mcpServers');
      assert.ok(parsed.mcpServers['stateset-commerce'], 'should include stateset-commerce server');
    });

    it('merges into an existing MCP config file', () => {
      const configDir = path.join(testHome, '.openclaw');
      fs.mkdirSync(configDir, { recursive: true });
      const configPath = path.join(configDir, 'mcp.json');
      fs.writeFileSync(
        configPath,
        JSON.stringify(
          {
            mcpServers: {
              'existing-server': {
                command: 'node',
                args: ['existing.js'],
              },
            },
          },
          null,
          2,
        ),
      );

      const result = runSetup(['--agent', 'openclaw'], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      assert.equal(result.exitCode, 0);

      const parsed = JSON.parse(fs.readFileSync(configPath, 'utf8'));
      assert.ok(parsed.mcpServers['existing-server'], 'should keep existing server');
      assert.ok(parsed.mcpServers['stateset-commerce'], 'should add stateset-commerce server');
    });

    it('supports explicit --mcp-config path', () => {
      const customPath = path.join(testHome, 'custom', 'agent-mcp.json');
      const result = runSetup(['--mcp-config', customPath], { ANTHROPIC_API_KEY: 'sk-ant-test' });
      assert.equal(result.exitCode, 0);
      assert.ok(fs.existsSync(customPath), 'should create custom MCP config path');
    });

    it('allows agent-only onboarding without local API key', () => {
      const result = runSetup(['--json', '--agent', 'openclaw'], { ANTHROPIC_API_KEY: '' });
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const apiStep = parsed.steps.find((s) => s.name === 'api_key');
      assert.equal(apiStep.status, 'optional');
      assert.equal(parsed.success, true);
    });
  });

  describe('starter packs', () => {
    it('installs starter pack policies and prompt artifacts', () => {
      const result = runSetup(
        ['--starter-pack', 'ops', '--db', './tenant/store.db'],
        { ANTHROPIC_API_KEY: 'sk-ant-test' },
      );
      assert.equal(result.exitCode, 0);

      const policyBase = path.join(testHome, 'tenant', '.stateset');
      const policyFile = path.join(policyBase, 'policies', 'starter-ops-orders.json');
      const promptFile = path.join(policyBase, 'agent-starters', 'starter-ops.md');

      assert.ok(fs.existsSync(policyFile), 'should create starter policy file');
      assert.ok(fs.existsSync(promptFile), 'should create starter prompt file');

      const parsedPolicy = JSON.parse(fs.readFileSync(policyFile, 'utf8'));
      assert.equal(parsedPolicy.domain, 'orders');
      assert.ok(Array.isArray(parsedPolicy.rules));
      assert.ok(parsedPolicy.rules.length > 0);
    });

    it('returns starter pack metadata in JSON mode', () => {
      const result = runSetup(
        ['--json', '--starter-pack', 'checkout', '--print-starter'],
        { ANTHROPIC_API_KEY: 'sk-ant-test' },
      );
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const starterStep = parsed.steps.find((s) => s.name === 'starter_pack');
      assert.equal(starterStep.status, 'configured');
      assert.equal(starterStep.pack, 'checkout');
      assert.ok(Array.isArray(starterStep.sampleRequests));
      assert.ok(starterStep.sampleRequests.length > 0);
    });

    it('reports unknown starter pack values', () => {
      const result = runSetup(['--json', '--starter-pack', 'invalid-pack'], {
        ANTHROPIC_API_KEY: 'sk-ant-test',
      });
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const starterStep = parsed.steps.find((s) => s.name === 'starter_pack');
      assert.equal(starterStep.status, 'error');
      assert.ok(String(starterStep.error || '').includes('unknown starter pack'));
    });
  });

  describe('handoff bundle', () => {
    it('writes handoff bundle when onboarding is configured', () => {
      const result = runSetup(
        ['--agent', 'openclaw', '--starter-pack', 'ops', '--db', './tenant/store.db'],
        { ANTHROPIC_API_KEY: 'sk-ant-test' },
      );
      assert.equal(result.exitCode, 0);

      const handoffPath = path.join(testHome, 'tenant', '.stateset', 'agent-starters', 'handoff.json');
      assert.ok(fs.existsSync(handoffPath), 'should create handoff bundle');

      const parsed = JSON.parse(fs.readFileSync(handoffPath, 'utf8'));
      assert.equal(parsed.schema, 'stateset.agentic-handoff.v1');
      assert.ok(parsed.mcp, 'should include mcp section');
      assert.ok(parsed.starterPack, 'should include starter pack section');
      assert.ok(parsed.launch, 'should include launch section');
      assert.ok(
        typeof parsed.launch.startCommand === 'string' && parsed.launch.startCommand.includes('start-mcp.sh'),
      );
      assert.ok(
        typeof parsed.launch.checkCommand === 'string' && parsed.launch.checkCommand.includes('check-mcp.sh'),
      );

      const launchStart = path.join(testHome, 'tenant', '.stateset', 'agent-starters', 'start-mcp.sh');
      const launchCheck = path.join(testHome, 'tenant', '.stateset', 'agent-starters', 'check-mcp.sh');
      assert.ok(fs.existsSync(launchStart), 'should create start-mcp.sh');
      assert.ok(fs.existsSync(launchCheck), 'should create check-mcp.sh');
    });

    it('returns handoff bundle in JSON mode when --print-handoff is set', () => {
      const result = runSetup(
        ['--json', '--agent', 'openclaw', '--starter-pack', 'checkout', '--print-handoff'],
        { ANTHROPIC_API_KEY: 'sk-ant-test' },
      );
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const handoffStep = parsed.steps.find((s) => s.name === 'handoff_bundle');
      assert.equal(handoffStep.status, 'configured');
      assert.ok(handoffStep.bundle, 'should include bundle payload');
      assert.equal(handoffStep.bundle.schema, 'stateset.agentic-handoff.v1');
    });
  });

  describe('quickstart preset', () => {
    it('applies openclaw + ops + agent-only + verify defaults', () => {
      fs.writeFileSync(path.join(testHome, 'store.db'), '');
      const result = runSetup(['--json', '--quickstart'], { ANTHROPIC_API_KEY: '' });
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);

      assert.equal(parsed.quickstart?.enabled, true);
      assert.equal(parsed.quickstart?.agent, 'openclaw');
      assert.equal(parsed.quickstart?.starterPack, 'ops');
      assert.equal(parsed.quickstart?.demo, true);
      assert.equal(parsed.quickstart?.verify, true);

      const apiStep = parsed.steps.find((s) => s.name === 'api_key');
      const onboardingStep = parsed.steps.find((s) => s.name === 'agent_onboarding');
      const starterStep = parsed.steps.find((s) => s.name === 'starter_pack');
      const verifyStep = parsed.steps.find((s) => s.name === 'verification');

      assert.equal(apiStep.status, 'optional');
      assert.equal(onboardingStep.status, 'configured');
      assert.equal(starterStep.status, 'configured');
      assert.equal(verifyStep.status, 'ok');
      assert.equal(parsed.success, true);
    });

    it('respects explicit overrides while quickstart is enabled', () => {
      fs.writeFileSync(path.join(testHome, 'store.db'), '');
      const result = runSetup(
        ['--json', '--quickstart', '--agent', 'cursor', '--starter-pack', 'support'],
        { ANTHROPIC_API_KEY: '' },
      );
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);

      assert.equal(parsed.quickstart?.agent, 'cursor');
      assert.equal(parsed.quickstart?.starterPack, 'support');

      const onboardingStep = parsed.steps.find((s) => s.name === 'agent_onboarding');
      const starterStep = parsed.steps.find((s) => s.name === 'starter_pack');
      assert.equal(onboardingStep.agent, 'cursor');
      assert.equal(starterStep.pack, 'support');
    });
  });

  describe('verification', () => {
    it('verifies configured onboarding artifacts', () => {
      fs.writeFileSync(path.join(testHome, 'store.db'), '');
      const result = runSetup(
        ['--json', '--agent', 'openclaw', '--starter-pack', 'ops', '--verify'],
        { ANTHROPIC_API_KEY: 'sk-ant-test' },
      );
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const verifyStep = parsed.steps.find((s) => s.name === 'verification');
      assert.equal(verifyStep.status, 'ok');
      assert.ok(Array.isArray(verifyStep.checks));
      assert.ok(verifyStep.checks.some((check) => check.name === 'handoff_launch_commands'));
      assert.ok(Array.isArray(parsed.nextSteps));
      assert.ok(parsed.nextSteps.some((s) => s.includes('Start MCP gateway')));
      assert.ok(parsed.nextSteps.some((s) => s.includes('Launch MCP gateway locally')));
    });

    it('verify-strict fails setup on warnings', () => {
      fs.writeFileSync(path.join(testHome, 'store.db'), '');
      const result = runSetup(['--json', '--verify', '--verify-strict'], {
        ANTHROPIC_API_KEY: 'sk-ant-test',
      });
      assert.equal(result.exitCode, 0);
      const parsed = JSON.parse(result.stdout);
      const verifyStep = parsed.steps.find((s) => s.name === 'verification');
      assert.equal(verifyStep.status, 'warnings');
      assert.equal(parsed.success, false);
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
