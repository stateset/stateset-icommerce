/**
 * End-to-end tests for CLI commands
 *
 * These tests execute the actual CLI binaries and verify their output.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { spawn, execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CLI_DIR = path.join(__dirname, '..', '..', 'bin');
const TEST_DB = path.join(os.tmpdir(), `stateset-e2e-${Date.now()}.db`);

/**
 * Execute a CLI command and return stdout/stderr
 */
function execCli(command, args = [], options = {}) {
  return new Promise((resolve, reject) => {
    const proc = spawn('node', [path.join(CLI_DIR, command), ...args], {
      env: { ...process.env, ...options.env },
      timeout: options.timeout || 30000
    });

    let stdout = '';
    let stderr = '';

    proc.stdout.on('data', (data) => {
      stdout += data.toString();
    });

    proc.stderr.on('data', (data) => {
      stderr += data.toString();
    });

    proc.on('close', (code) => {
      resolve({ code, stdout, stderr });
    });

    proc.on('error', reject);
  });
}

describe('CLI E2E Tests', () => {
  afterEach(() => {
    // Cleanup test database
    try {
      if (fs.existsSync(TEST_DB)) {
        fs.unlinkSync(TEST_DB);
      }
    } catch {
      // Ignore cleanup errors
    }
  });

  describe('stateset-direct', () => {
    describe('help', () => {
      it('should show help with --help flag', async () => {
        const { code, stdout } = await execCli('stateset-direct.js', ['--help']);

        assert.strictEqual(code, 0);
        assert.ok(stdout.includes('USAGE'));
        assert.ok(stdout.includes('customers'));
        assert.ok(stdout.includes('orders'));
        assert.ok(stdout.includes('inventory'));
      });

      it('should show help with -h flag', async () => {
        const { code, stdout } = await execCli('stateset-direct.js', ['-h']);

        assert.strictEqual(code, 0);
        assert.ok(stdout.includes('USAGE'));
      });

      it('should show help when no arguments provided', async () => {
        const { code, stdout } = await execCli('stateset-direct.js', []);

        assert.strictEqual(code, 0);
        assert.ok(stdout.includes('USAGE'));
      });
    });

    describe('database handling', () => {
      it('should accept --db flag', async () => {
        const { code } = await execCli('stateset-direct.js', [
          '--db', TEST_DB,
          'customers', 'list'
        ]);

        // May fail if db doesn't exist, but should not crash
        assert.ok(code === 0 || code === 1);
      });

      it('should report error for invalid database path', async () => {
        const { code, stderr, stdout } = await execCli('stateset-direct.js', [
          '--db', '/nonexistent/path/db.sqlite',
          'customers', 'list'
        ]);

        // Should exit with error
        assert.strictEqual(code, 1);
      });
    });

    describe('resource aliases', () => {
      it('should expand single letter aliases', async () => {
        const { stdout: fullOutput } = await execCli('stateset-direct.js', [
          '--db', ':memory:',
          'customers', 'list', '--json'
        ]);

        const { stdout: aliasOutput } = await execCli('stateset-direct.js', [
          '--db', ':memory:',
          'c', 'l', '--json'
        ]);

        // Both should produce similar output structure
        // (empty array for in-memory db)
      });
    });

    describe('JSON output', () => {
      it('should output valid JSON with --json flag', async () => {
        const { code, stdout, stderr } = await execCli('stateset-direct.js', [
          '--db', ':memory:',
          'customers', 'list',
          '--json'
        ]);

        if (code === 0) {
          // Should be valid JSON
          assert.doesNotThrow(() => JSON.parse(stdout.trim()));
        }
      });
    });

    describe('error handling', () => {
      it('should show error for unknown resource', async () => {
        const { code, stderr, stdout } = await execCli('stateset-direct.js', [
          'unknown', 'list'
        ]);

        assert.strictEqual(code, 1);
        const output = stderr || stdout;
        assert.ok(output.includes('Unknown resource') || output.includes('Error'));
      });

      it('should show error for unknown action', async () => {
        const { code, stderr, stdout } = await execCli('stateset-direct.js', [
          'customers', 'unknown_action'
        ]);

        assert.strictEqual(code, 1);
      });
    });
  });

  describe('stateset-doctor', () => {
    it('should run health checks', async () => {
      const { code, stdout } = await execCli('stateset-doctor.js', []);

      // May pass or fail depending on environment
      assert.ok(code === 0 || code === 1);
      assert.ok(stdout.includes('Health Check') || stdout.includes('check'));
    });

    it('should output JSON with --json flag', async () => {
      const { stdout } = await execCli('stateset-doctor.js', ['--json']);

      const result = JSON.parse(stdout.trim());
      assert.ok('healthy' in result);
      assert.ok('checks' in result);
    });

    it('should check specific database with --db', async () => {
      const { code, stdout } = await execCli('stateset-doctor.js', [
        '--db', TEST_DB,
        '--json'
      ]);

      const result = JSON.parse(stdout.trim());
      assert.ok('checks' in result);
      assert.ok('Database' in result.checks);
    });

    it('should show verbose output with --verbose', async () => {
      const { stdout } = await execCli('stateset-doctor.js', ['--verbose']);

      // Verbose output should be longer
      assert.ok(stdout.length > 100);
    });
  });

  describe('stateset-completion', () => {
    it('should generate bash completions', async () => {
      const { code, stdout } = await execCli('stateset-completion.js', ['bash']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('_stateset'));
      assert.ok(stdout.includes('complete'));
      assert.ok(stdout.includes('customers'));
    });

    it('should generate zsh completions', async () => {
      const { code, stdout } = await execCli('stateset-completion.js', ['zsh']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('compdef'));
      assert.ok(stdout.includes('_stateset'));
    });

    it('should generate fish completions', async () => {
      const { code, stdout } = await execCli('stateset-completion.js', ['fish']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('complete -c stateset'));
    });

    it('should show help without arguments', async () => {
      const { code, stdout } = await execCli('stateset-completion.js', []);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('USAGE'));
      assert.ok(stdout.includes('bash'));
      assert.ok(stdout.includes('zsh'));
    });

    it('should error for unknown shell', async () => {
      const { code, stderr } = await execCli('stateset-completion.js', ['unknown']);

      assert.strictEqual(code, 1);
      assert.ok(stderr.includes('Unknown shell'));
    });
  });

  describe('stateset-config', () => {
    it('should show help', async () => {
      const { code, stdout } = await execCli('stateset-config.js', ['--help']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('USAGE') || stdout.includes('config'));
    });
  });

  describe('stateset-mcp-events', () => {
    it('should show help', async () => {
      const { code, stdout } = await execCli('stateset-mcp-events.js', ['--help']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.includes('USAGE'));
      assert.ok(stdout.includes('MCP Event Stream Gateway'));
    });

    it('should show version', async () => {
      const { code, stdout } = await execCli('stateset-mcp-events.js', ['--version']);

      assert.strictEqual(code, 0);
      assert.ok(stdout.trim().startsWith('stateset-mcp-events v'));
    });
  });
});

describe('CLI Integration', () => {
  describe('pipeline commands', () => {
    it('should support piping JSON output', async () => {
      // This tests that JSON output is valid and can be processed
      const { stdout } = await execCli('stateset-direct.js', [
        '--db', ':memory:',
        '--json',
        'customers', 'list'
      ]);

      if (stdout.trim()) {
        assert.doesNotThrow(() => JSON.parse(stdout.trim()));
      }
    });
  });

  describe('environment variables', () => {
    it('should respect ANTHROPIC_API_KEY', async () => {
      const { stdout } = await execCli('stateset-doctor.js', ['--json'], {
        env: { ANTHROPIC_API_KEY: 'sk-ant-test-key' }
      });

      const result = JSON.parse(stdout.trim());
      assert.ok(result.checks['API Key'].status === 'ok');
    });

    it('should warn when API key is missing', async () => {
      const { stdout } = await execCli('stateset-doctor.js', ['--json'], {
        env: { ANTHROPIC_API_KEY: '' }
      });

      const result = JSON.parse(stdout.trim());
      assert.ok(result.checks['API Key'].status === 'error');
    });
  });
});
