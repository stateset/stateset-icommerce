import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const exec = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CLI_PATH = path.join(__dirname, '..', '..', 'bin', 'stateset-agents.js');

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

async function runCli(args, opts = {}) {
  try {
    const { stdout, stderr } = await exec('node', [CLI_PATH, ...args], {
      timeout: 15000,
      env: { ...process.env, NODE_NO_WARNINGS: '1' },
      ...opts,
    });
    return { stdout, stderr, exitCode: 0 };
  } catch (err) {
    return {
      stdout: err.stdout || '',
      stderr: err.stderr || '',
      exitCode: err.code ?? 1,
    };
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('stateset-agents CLI', () => {
  describe('--help', () => {
    it('shows help text', async () => {
      const { stdout } = await runCli(['--help']);
      assert.ok(stdout.includes('stateset-agents'));
      assert.ok(stdout.includes('create'));
      assert.ok(stdout.includes('list'));
      assert.ok(stdout.includes('demo'));
      assert.ok(stdout.includes('discover'));
    });
  });

  describe('--version', () => {
    it('prints version number', async () => {
      const { stdout } = await runCli(['--version']);
      assert.match(stdout.trim(), /^\d+\.\d+\.\d+/);
    });
  });

  describe('create --help', () => {
    it('shows create subcommand options', async () => {
      const { stdout } = await runCli(['create', '--help']);
      assert.ok(stdout.includes('--strategy'));
      assert.ok(stdout.includes('--budget-daily'));
      assert.ok(stdout.includes('--auto-register-card'));
    });
  });

  describe('demo --help', () => {
    it('shows demo subcommand', async () => {
      const { stdout } = await runCli(['demo', '--help']);
      assert.ok(stdout.includes('basic-negotiation'));
    });
  });

  describe('demo basic-negotiation', () => {
    it('runs basic negotiation demo successfully', async () => {
      const { stdout, exitCode } = await runCli([
        'demo',
        'basic-negotiation',
        '--db',
        ':memory:',
        '--json',
      ]);
      assert.strictEqual(exitCode, 0, `Expected exit 0, got ${exitCode}. stderr: ${stdout}`);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.scenario, 'basic-negotiation');
      assert.ok(result.result.quoteId);
    });
  });

  describe('demo marketplace', () => {
    it('runs marketplace demo successfully', async () => {
      const { stdout, exitCode } = await runCli([
        'demo',
        'marketplace',
        '--db',
        ':memory:',
        '--json',
      ]);
      assert.strictEqual(exitCode, 0);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.scenario, 'marketplace');
      assert.ok(Array.isArray(result.result.quoteIds));
      assert.strictEqual(result.result.sellerCount, 3);
    });
  });

  describe('demo escrow-deal', () => {
    it('runs escrow deal demo successfully', async () => {
      const { stdout, exitCode } = await runCli([
        'demo',
        'escrow-deal',
        '--db',
        ':memory:',
        '--json',
      ]);
      assert.strictEqual(exitCode, 0);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.scenario, 'escrow-deal');
      assert.ok(result.result.quoteId);
    });
  });

  describe('demo invalid-scenario', () => {
    it('rejects unknown scenario', async () => {
      const { stdout, exitCode } = await runCli([
        'demo',
        'unknown-scenario',
        '--db',
        ':memory:',
        '--json',
      ]);
      assert.strictEqual(exitCode, 1);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, false);
      assert.ok(result.error.includes('Unknown scenario'));
    });
  });

  describe('create command', () => {
    it('creates agent and returns JSON', async () => {
      const { stdout, exitCode } = await runCli([
        'create',
        'TestBot',
        '--db',
        ':memory:',
        '--strategy',
        'always-accept',
        '--budget-daily',
        '500',
        '--json',
      ]);
      assert.strictEqual(exitCode, 0, `Exit code: ${exitCode}, stdout: ${stdout}`);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.agent.name, 'TestBot');
      assert.ok(result.agent.walletAddress);
      assert.ok(result.agent.agentId);
    });

    it('creates agent with auto-register-card', async () => {
      const { stdout, exitCode } = await runCli([
        'create',
        'CardBot',
        '--db',
        ':memory:',
        '--auto-register-card',
        '--json',
      ]);
      assert.strictEqual(exitCode, 0);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.success, true);
      assert.ok(result.agent.card, 'Expected card to be registered');
    });
  });

  describe('list command', () => {
    it('lists agents (empty in fresh process)', async () => {
      const { stdout, exitCode } = await runCli(['list', '--json']);
      assert.strictEqual(exitCode, 0);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.count, 0);
      assert.deepStrictEqual(result.agents, []);
    });
  });

  describe('discover command', () => {
    it('discovers agents in empty db', async () => {
      const { stdout, exitCode } = await runCli(['discover', '--db', ':memory:', '--json']);
      assert.strictEqual(exitCode, 0);
      const result = JSON.parse(stdout);
      assert.strictEqual(result.agentCount, 0);
      assert.strictEqual(result.serviceCount, 0);
    });
  });

  describe('status command (no runtime)', () => {
    it('fails for unknown agent', async () => {
      const { exitCode } = await runCli(['status', 'NonExistent', '--json']);
      assert.strictEqual(exitCode, 1);
    });
  });

  describe('stop command (no runtime)', () => {
    it('fails for unknown agent', async () => {
      const { exitCode } = await runCli(['stop', 'NonExistent', '--json']);
      assert.strictEqual(exitCode, 1);
    });
  });
});
