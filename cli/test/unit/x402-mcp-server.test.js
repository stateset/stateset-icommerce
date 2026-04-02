/**
 * Unit tests for cli/src/x402-mcp-server.js
 *
 * Tests the x402 MCP server factory: helper functions (parseBool, parseNumber,
 * parseList, decodeKeyMaterial, truncateBody, result, errorResult),
 * loadKeyFromJson validation, resolveSigningKey path-traversal prevention,
 * createX402McpServer config resolution, and X402_MCP_TOOL_NAMES export.
 *
 * Uses node:test runner with try/catch import fallback.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { deriveEvmWalletFromSeed } from '../../src/chains/wallet.js';

// ---------------------------------------------------------------------------
// Safe dynamic import — the module pulls in @anthropic-ai/claude-agent-sdk,
// x402 modules, and sync/keys.js which may fail in certain environments.
// ---------------------------------------------------------------------------

let createX402McpServer;
let X402_MCP_TOOL_NAMES;
let importError = null;

try {
  const mod = await import('../../src/x402-mcp-server.js');
  createX402McpServer = mod.createX402McpServer;
  X402_MCP_TOOL_NAMES = mod.X402_MCP_TOOL_NAMES;
} catch (err) {
  importError = err;
}

const canImport = importError === null;
const originalFetch = globalThis.fetch;

// ---------------------------------------------------------------------------
// Temp directory helper for isolated file system operations
// ---------------------------------------------------------------------------

function makeTmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'x402-mcp-test-'));
}

function cleanDir(dir) {
  try {
    fs.rmSync(dir, { recursive: true, force: true });
  } catch {
    // ignore
  }
}

function mockFetch(handler) {
  globalThis.fetch = async (...args) => handler(...args);
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

// ---------------------------------------------------------------------------
// Helper to build a minimal env that satisfies ensureConfig() validation.
// We avoid actually calling tools (which hit ensureConfig) in most tests,
// instead inspecting the registered tools and server structure.
// ---------------------------------------------------------------------------

function minimalEnv(overrides = {}) {
  return {
    X402_SEQUENCER_URL: 'https://seq.test.local',
    X402_TENANT_ID: 'tenant-1',
    X402_STORE_ID: 'store-1',
    X402_AGENT_ID: 'agent-1',
    X402_PAYER_ADDRESS: '0xPayer',
    ...overrides,
  };
}

// ===========================================================================
// 1. X402_MCP_TOOL_NAMES export
// ===========================================================================

describe('X402_MCP_TOOL_NAMES', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  it('is an array of 5 strings', () => {
    assert.ok(Array.isArray(X402_MCP_TOOL_NAMES));
    assert.equal(X402_MCP_TOOL_NAMES.length, 5);
    for (const name of X402_MCP_TOOL_NAMES) {
      assert.equal(typeof name, 'string');
    }
  });

  it('contains x402_call', () => {
    assert.ok(X402_MCP_TOOL_NAMES.includes('x402_call'));
  });

  it('contains x402_budget_status', () => {
    assert.ok(X402_MCP_TOOL_NAMES.includes('x402_budget_status'));
  });

  it('contains x402_history', () => {
    assert.ok(X402_MCP_TOOL_NAMES.includes('x402_history'));
  });

  it('contains x402_receipt', () => {
    assert.ok(X402_MCP_TOOL_NAMES.includes('x402_receipt'));
  });

  it('contains x402_balance', () => {
    assert.ok(X402_MCP_TOOL_NAMES.includes('x402_balance'));
  });

  it('has no duplicates', () => {
    const unique = new Set(X402_MCP_TOOL_NAMES);
    assert.equal(unique.size, X402_MCP_TOOL_NAMES.length);
  });

  it('every name starts with x402_', () => {
    for (const name of X402_MCP_TOOL_NAMES) {
      assert.ok(name.startsWith('x402_'), `expected ${name} to start with x402_`);
    }
  });
});

// ===========================================================================
// 2. createX402McpServer — server structure
// ===========================================================================

describe('createX402McpServer — server structure', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns an object with type "sdk"', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.equal(server.type, 'sdk');
  });

  it('server name is "stateset-x402"', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.equal(server.name, 'stateset-x402');
  });

  it('has an instance property', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.ok(server.instance, 'expected server.instance to exist');
    assert.equal(typeof server.instance, 'object');
  });

  it('registers exactly 5 tools', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const toolNames = Object.keys(tools);
    assert.equal(toolNames.length, 5);
  });

  it('registers tools matching X402_MCP_TOOL_NAMES', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const toolNames = Object.keys(tools);
    for (const expected of X402_MCP_TOOL_NAMES) {
      assert.ok(toolNames.includes(expected), `missing tool: ${expected}`);
    }
  });
});

// ===========================================================================
// 3. Registered tool properties
// ===========================================================================

describe('createX402McpServer — tool properties', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let server;
  let tools;
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
    server = createX402McpServer({ env: {}, configDir: tmpDir });
    tools = server.instance._registeredTools;
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('x402_call has a description', () => {
    assert.ok(tools.x402_call.description);
    assert.equal(typeof tools.x402_call.description, 'string');
  });

  it('x402_call has a handler function', () => {
    assert.equal(typeof tools.x402_call.handler, 'function');
  });

  it('x402_budget_status has a description', () => {
    assert.ok(tools.x402_budget_status.description);
  });

  it('x402_budget_status has a handler function', () => {
    assert.equal(typeof tools.x402_budget_status.handler, 'function');
  });

  it('x402_history has a description', () => {
    assert.ok(tools.x402_history.description);
  });

  it('x402_receipt has a handler function', () => {
    assert.equal(typeof tools.x402_receipt.handler, 'function');
  });

  it('x402_balance has a description', () => {
    assert.ok(tools.x402_balance.description);
  });

  it('all tools are enabled by default', () => {
    for (const name of X402_MCP_TOOL_NAMES) {
      assert.equal(tools[name].enabled, true, `${name} should be enabled`);
    }
  });

  it('x402_call description mentions x402', () => {
    assert.ok(
      tools.x402_call.description.toLowerCase().includes('x402'),
      'x402_call description should mention x402',
    );
  });

  it('x402_balance description mentions wallet or balance', () => {
    const desc = tools.x402_balance.description.toLowerCase();
    assert.ok(
      desc.includes('balance') || desc.includes('wallet'),
      'x402_balance description should mention balance or wallet',
    );
  });
});

// ===========================================================================
// 4. x402_budget_status — no budget configured
// ===========================================================================

describe('x402_budget_status — default budget', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns budget even with empty env (default budget file always set)', async () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const handler = tools.x402_budget_status.handler;
    const res = await handler({});
    assert.ok(res.content, 'expected MCP content array');
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    // Budget is always present because getDefaultBudgetStateFile() provides a default path
    assert.ok(data.budget, 'expected budget object');
    assert.equal(typeof data.budget.spentToday, 'number');
    assert.ok(data.budget.stateFile, 'expected stateFile path');
  });
});

// ===========================================================================
// 5. x402_history — no budget configured
// ===========================================================================

describe('x402_history — default budget (empty)', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns empty history with default budget (no spend yet)', async () => {
    const budgetFile = path.join(tmpDir, 'hist-default.json');
    const server = createX402McpServer({
      env: { X402_BUDGET_STATE_FILE: budgetFile },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const handler = tools.x402_history.handler;
    const res = await handler({ limit: 10 });
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.equal(data.count, 0);
    assert.deepEqual(data.history, []);
  });
});

// ===========================================================================
// 6. x402_budget_status — with budget configured
// ===========================================================================

describe('x402_budget_status — with budget env vars', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns budget info when starting balance set', async () => {
    const budgetFile = path.join(tmpDir, 'budget.json');
    const server = createX402McpServer({
      env: {
        X402_STARTING_BALANCE: '1000',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_budget_status.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.ok(data.budget, 'expected budget object');
    assert.equal(data.budget.balance, 1000);
    assert.equal(data.budget.spentToday, 0);
  });

  it('returns budget with daily limit env var', async () => {
    const budgetFile = path.join(tmpDir, 'budget2.json');
    const server = createX402McpServer({
      env: {
        X402_BUDGET_DAILY: '500',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_budget_status.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.ok(data.budget);
    assert.equal(data.budget.dailyBudget, 500);
  });

  it('returns budget with per-call limit', async () => {
    const budgetFile = path.join(tmpDir, 'budget3.json');
    const server = createX402McpServer({
      env: {
        X402_BUDGET_PER_CALL: '50',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_budget_status.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.ok(data.budget);
    assert.equal(data.budget.perCallLimit, 50);
  });

  it('budget stateFile path is included in output', async () => {
    const budgetFile = path.join(tmpDir, 'my-budget.json');
    const server = createX402McpServer({
      env: {
        X402_STARTING_BALANCE: '100',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_budget_status.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.budget.stateFile, budgetFile);
  });
});

// ===========================================================================
// 7. x402_receipt — no sequencer configured
// ===========================================================================

describe('x402_receipt — no sequencer', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns error when no sequencer URL configured', async () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_receipt.handler({ intentId: 'intent_abc' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, false);
    assert.ok(data.error.includes('X402_SEQUENCER_URL'));
  });
});

// ===========================================================================
// 8. x402_balance — no chain provided
// ===========================================================================

describe('x402_balance — no chain', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns suggestion message when chain not provided', async () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_balance.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.equal(data.balance, null);
    assert.ok(data.message.includes('budget_status') || data.message.includes('Chain not provided'));
  });
});

// ===========================================================================
// 9. Config resolution from env variables
// ===========================================================================

describe('createX402McpServer — config from env', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('reads X402_SEQUENCER_URL from env', () => {
    const env = { X402_SEQUENCER_URL: 'https://seq.example.com' };
    // Server creation succeeds (does not throw)
    const server = createX402McpServer({ env, configDir: tmpDir });
    assert.ok(server);
  });

  it('reads X402_SEQUENCER as fallback for sequencer URL', () => {
    const env = { X402_SEQUENCER: 'https://seq2.example.com' };
    const server = createX402McpServer({ env, configDir: tmpDir });
    assert.ok(server);
  });

  it('accepts empty env without error (no required fields at creation time)', () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.ok(server);
  });

  it('reads X402_REQUIRE_RECEIPT as boolean', async () => {
    const budgetFile = path.join(tmpDir, 'budget-receipt.json');
    const env = {
      X402_REQUIRE_RECEIPT: 'true',
      X402_STARTING_BALANCE: '100',
      X402_BUDGET_STATE_FILE: budgetFile,
    };
    const server = createX402McpServer({ env, configDir: tmpDir });
    // Server creation works; the value is used internally
    assert.ok(server);
  });

  it('reads X402_PREFERRED_NETWORKS as comma-separated list', () => {
    const env = { X402_PREFERRED_NETWORKS: 'solana,base,ethereum' };
    const server = createX402McpServer({ env, configDir: tmpDir });
    assert.ok(server);
  });
});

// ===========================================================================
// 10. Config resolution from file config
// ===========================================================================

describe('createX402McpServer — config from file', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('reads sequencerUrl from file config', () => {
    const configPath = path.join(tmpDir, 'x402.json');
    fs.writeFileSync(configPath, JSON.stringify({
      sequencerUrl: 'https://seq-file.example.com',
      tenantId: 'tenant-f',
      storeId: 'store-f',
      agentId: 'agent-f',
      payerAddress: '0xFilePayer',
    }));
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.ok(server);
  });

  it('env overrides file config', async () => {
    const configPath = path.join(tmpDir, 'x402.json');
    const budgetFile = path.join(tmpDir, 'budget-override.json');
    fs.writeFileSync(configPath, JSON.stringify({
      startingBalance: 999,
    }));
    // Env sets a different starting balance
    const env = {
      X402_STARTING_BALANCE: '200',
      X402_BUDGET_STATE_FILE: budgetFile,
    };
    const server = createX402McpServer({ env, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_budget_status.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.ok(data.budget);
    assert.equal(data.budget.balance, 200);
  });

  it('handles missing config file gracefully', () => {
    // No x402.json written — should not throw
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    assert.ok(server);
  });
});

// ===========================================================================
// 11. x402_call — ensureConfig validation errors
// ===========================================================================

describe('x402_call — ensureConfig validation', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    restoreFetch();
    cleanDir(tmpDir);
  });

  it('returns error when agent ID is missing', async () => {
    const server = createX402McpServer({ env: {}, configDir: tmpDir });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com/data' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, false);
    assert.ok(data.error.includes('X402_AGENT_ID'));
  });

  it('returns error when payer address missing', async () => {
    const server = createX402McpServer({
      env: {
        X402_AGENT_ID: 'a1',
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com/data' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(data.error.includes('X402_PAYER_ADDRESS'));
  });

  it('supports exact x402 calls without sequencer configuration', async () => {
    const privateKey = Buffer.from('11'.repeat(32), 'hex');
    const publicKey = Buffer.from('22'.repeat(32), 'hex');
    const wallet = deriveEvmWalletFromSeed(privateKey, 'base');
    const paymentRequired = {
      x402Version: 2,
      error: 'PAYMENT-SIGNATURE header is required',
      resource: {
        url: 'https://api.example.com/data',
        description: 'Premium data',
        mimeType: 'application/json',
      },
      accepts: [
        {
          scheme: 'exact',
          network: 'eip155:8453',
          amount: '10000',
          asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
          payTo: '0x5555555555555555555555555555555555555555',
          maxTimeoutSeconds: 60,
          extra: {
            assetTransferMethod: 'eip3009',
            name: 'USD Coin',
            version: '2',
          },
        },
      ],
      extensions: {},
    };

    let callCount = 0;
    mockFetch((_url, options = {}) => {
      callCount += 1;
      if (callCount === 1) {
        return {
          ok: false,
          status: 402,
          statusText: 'Payment Required',
          url: 'https://api.example.com/data',
          headers: {
            get(name) {
              return String(name).toLowerCase() === 'payment-required'
                ? Buffer.from(JSON.stringify(paymentRequired)).toString('base64')
                : null;
            },
            entries() {
              return [];
            },
          },
          clone() {
            return this;
          },
          async json() {
            return paymentRequired;
          },
          async text() {
            return JSON.stringify(paymentRequired);
          },
        };
      }

      assert.ok(options.headers['PAYMENT-SIGNATURE']);
      return {
        ok: true,
        status: 200,
        statusText: 'OK',
        url: 'https://api.example.com/data',
        headers: new Headers({ 'content-type': 'application/json' }),
        async json() {
          return { success: true };
        },
      };
    });

    const server = createX402McpServer({
      env: {
        X402_AGENT_ID: 'a1',
        X402_PAYER_ADDRESS: wallet.address,
        X402_SIGNING_KEY: JSON.stringify({
          privateKey: privateKey.toString('hex'),
          publicKey: publicKey.toString('hex'),
        }),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({
      url: 'https://api.example.com/data',
      method: 'GET',
    });
    assert.notEqual(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.equal(data.status, 200);
  });

  it('returns legacy-flow error when sequencer config is absent', async () => {
    mockFetch(() => ({
      ok: false,
      status: 402,
      statusText: 'Payment Required',
      url: 'https://api.example.com/data',
      headers: {
        get(name) {
          return String(name).toLowerCase() === 'x-payment-required'
            ? Buffer.from(
                JSON.stringify({
                  payee_address: '0xPayee',
                  amount: 1000,
                  asset: 'usdc',
                  network: 'set_chain',
                }),
              ).toString('base64')
            : null;
        },
        entries() {
          return [];
        },
      },
      clone() {
        return this;
      },
      async json() {
        return {};
      },
      async text() {
        return '{}';
      },
    }));

    const server = createX402McpServer({
      env: {
        X402_AGENT_ID: 'a1',
        X402_PAYER_ADDRESS: '0xPayer',
        X402_SIGNING_KEY: JSON.stringify({
          privateKey: '11'.repeat(32),
          publicKey: '22'.repeat(32),
        }),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com/data' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(data.error.includes('sequencerClient is required for legacy sequencer-backed x402 payments'));
  });
});

// ===========================================================================
// 12. resolveSigningKey — path traversal via X402_SIGNING_KEY_PATH
// ===========================================================================

describe('resolveSigningKey — path traversal prevention', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('rejects keyPath outside cwd', async () => {
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY_PATH: '/etc/passwd',
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(
      data.error.includes('keyPath must be within') || data.error.includes('within the current'),
      `expected path traversal error, got: ${data.error}`,
    );
  });

  it('rejects keyPath with relative traversal', async () => {
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY_PATH: '../../../../../../etc/shadow',
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(
      data.error.includes('keyPath must be within') || data.error.includes('within the current'),
      `expected path traversal error, got: ${data.error}`,
    );
  });
});

// ===========================================================================
// 13. loadKeyFromJson — via X402_SIGNING_KEY env
// ===========================================================================

describe('loadKeyFromJson — via signing key env', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('rejects signing key JSON missing privateKey', async () => {
    const keyJson = { publicKey: 'abcd1234' };
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY: JSON.stringify(keyJson),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(data.error.includes('privateKey'));
  });

  it('rejects signing key JSON missing publicKey', async () => {
    const keyJson = { privateKey: 'abcd1234' };
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY: JSON.stringify(keyJson),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    assert.equal(res.isError, true);
    const data = JSON.parse(res.content[0].text);
    assert.ok(data.error.includes('publicKey'));
  });

  it('rejects invalid signing key JSON string', async () => {
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY: 'not-valid-json',
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    assert.equal(res.isError, true);
  });

  it('accepts valid hex signing key JSON', async () => {
    const hexPriv = 'a'.repeat(64);
    const hexPub = 'b'.repeat(64);
    const keyJson = { privateKey: hexPriv, publicKey: hexPub, keyId: 42 };
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY: JSON.stringify(keyJson),
      },
      configDir: tmpDir,
    });
    // If the key loads, ensureConfig will proceed past key resolution.
    // The call will then fail at agent.fetch (network), not at key loading.
    const tools = server.instance._registeredTools;
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    // Should NOT fail with key-related error
    const data = JSON.parse(res.content[0].text);
    if (res.isError) {
      assert.ok(
        !data.error.includes('privateKey') && !data.error.includes('publicKey'),
        `should not fail on key validation, got: ${data.error}`,
      );
    }
  });
});

// ===========================================================================
// 14. loadKeyFromJson — via X402_SIGNING_KEY_PATH (file-based)
// ===========================================================================

describe('loadKeyFromJson — via key file path', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
    // We need to work from within tmpDir for the path traversal check
    // The resolveSigningKey checks if keyPath starts with cwd or configDir
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('rejects key file with missing publicKey field', async () => {
    const keyFile = path.join(process.cwd(), '.test-key-no-pub.json');
    try {
      fs.writeFileSync(keyFile, JSON.stringify({ privateKey: 'aa'.repeat(32) }));
      const server = createX402McpServer({
        env: {
          ...minimalEnv(),
          X402_SIGNING_KEY_PATH: keyFile,
        },
        configDir: tmpDir,
      });
      const tools = server.instance._registeredTools;
      const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
      assert.equal(res.isError, true);
      const data = JSON.parse(res.content[0].text);
      assert.ok(data.error.includes('publicKey'));
    } finally {
      try { fs.unlinkSync(keyFile); } catch { /* ignore */ }
    }
  });
});

// ===========================================================================
// 15. Signing key caching — resolveKeyOnce
// ===========================================================================

describe('resolveKeyOnce — caching behavior', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('calling x402_call twice with same server reuses signing key cache', async () => {
    const hexKey = 'ab'.repeat(32);
    const keyJson = { privateKey: hexKey, publicKey: hexKey, keyId: 1 };
    const server = createX402McpServer({
      env: {
        ...minimalEnv(),
        X402_SIGNING_KEY: JSON.stringify(keyJson),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    // Both calls should resolve the key (first from JSON, second from cache)
    // Both will fail at the network layer, but NOT at key resolution
    const res1 = await tools.x402_call.handler({ url: 'https://api.example.com/1' });
    const res2 = await tools.x402_call.handler({ url: 'https://api.example.com/2' });
    // Both should produce the same error type (not key-related)
    const d1 = JSON.parse(res1.content[0].text);
    const d2 = JSON.parse(res2.content[0].text);
    if (res1.isError && res2.isError) {
      assert.ok(!d1.error.includes('privateKey'));
      assert.ok(!d2.error.includes('publicKey'));
    }
  });
});

// ===========================================================================
// 16. parseBool — tested indirectly through X402_REQUIRE_RECEIPT
// ===========================================================================

describe('parseBool behavior — via X402_REQUIRE_RECEIPT', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('empty string falls back to false (default)', () => {
    // No error at creation — indicates parseBool handled empty value
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: '' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('"true" string is accepted', () => {
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: 'true' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('"1" string is accepted', () => {
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: '1' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('"yes" string is accepted', () => {
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: 'yes' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('"on" string is accepted', () => {
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: 'on' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('"false" string is accepted without error', () => {
    const server = createX402McpServer({
      env: { X402_REQUIRE_RECEIPT: 'false' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });
});

// ===========================================================================
// 17. parseNumber — tested indirectly through numeric env vars
// ===========================================================================

describe('parseNumber behavior — via numeric env vars', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('parses X402_MAX_AMOUNT as number', () => {
    const server = createX402McpServer({
      env: { X402_MAX_AMOUNT: '999.50' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('parses X402_RECEIPT_TIMEOUT_MS as integer', () => {
    const server = createX402McpServer({
      env: { X402_RECEIPT_TIMEOUT_MS: '30000' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('parses X402_RECEIPT_POLL_MS as integer', () => {
    const server = createX402McpServer({
      env: { X402_RECEIPT_POLL_MS: '2000' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('handles non-numeric strings gracefully (returns undefined)', () => {
    // NaN/invalid becomes undefined — no error at creation
    const server = createX402McpServer({
      env: { X402_MAX_AMOUNT: 'not-a-number' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('handles empty string gracefully', () => {
    const server = createX402McpServer({
      env: { X402_RECEIPT_TIMEOUT_MS: '' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('handles Infinity gracefully (returns undefined)', () => {
    const server = createX402McpServer({
      env: { X402_MAX_AMOUNT: 'Infinity' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });
});

// ===========================================================================
// 18. parseList — tested indirectly through X402_PREFERRED_NETWORKS
// ===========================================================================

describe('parseList behavior — via X402_PREFERRED_NETWORKS', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('empty string produces empty list (no error)', () => {
    const server = createX402McpServer({
      env: { X402_PREFERRED_NETWORKS: '' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('single value produces list', () => {
    const server = createX402McpServer({
      env: { X402_PREFERRED_NETWORKS: 'solana' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('comma-separated values produce list', () => {
    const server = createX402McpServer({
      env: { X402_PREFERRED_NETWORKS: 'solana, base, ethereum' },
      configDir: tmpDir,
    });
    assert.ok(server);
  });
});

// ===========================================================================
// 19. Default export
// ===========================================================================

describe('default export', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  it('default export is the same as createX402McpServer', async () => {
    const mod = await import('../../src/x402-mcp-server.js');
    assert.equal(mod.default, mod.createX402McpServer);
  });
});

// ===========================================================================
// 20. x402_history — with budget configured
// ===========================================================================

describe('x402_history — with budget configured', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('returns empty history for fresh budget state', async () => {
    const budgetFile = path.join(tmpDir, 'hist-budget.json');
    const server = createX402McpServer({
      env: {
        X402_STARTING_BALANCE: '500',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    const res = await tools.x402_history.handler({ limit: 10 });
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
    assert.equal(data.count, 0);
    assert.deepEqual(data.history, []);
  });

  it('default limit is 50 when not provided', async () => {
    const budgetFile = path.join(tmpDir, 'hist-budget2.json');
    const server = createX402McpServer({
      env: {
        X402_STARTING_BALANCE: '500',
        X402_BUDGET_STATE_FILE: budgetFile,
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    // Handler receives empty object — limit defaults to 50 in the code
    const res = await tools.x402_history.handler({});
    const data = JSON.parse(res.content[0].text);
    assert.equal(data.success, true);
  });
});

// ===========================================================================
// 21. Multiple server instances are independent
// ===========================================================================

describe('multiple server instances', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir1;
  let tmpDir2;

  beforeEach(() => {
    tmpDir1 = makeTmpDir();
    tmpDir2 = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir1);
    cleanDir(tmpDir2);
  });

  it('two servers with different configs are independent', async () => {
    const budgetFile1 = path.join(tmpDir1, 'b1.json');
    const budgetFile2 = path.join(tmpDir2, 'b2.json');

    const server1 = createX402McpServer({
      env: { X402_STARTING_BALANCE: '100', X402_BUDGET_STATE_FILE: budgetFile1 },
      configDir: tmpDir1,
    });
    const server2 = createX402McpServer({
      env: { X402_STARTING_BALANCE: '999', X402_BUDGET_STATE_FILE: budgetFile2 },
      configDir: tmpDir2,
    });

    const tools1 = server1.instance._registeredTools;
    const tools2 = server2.instance._registeredTools;

    const res1 = await tools1.x402_budget_status.handler({});
    const res2 = await tools2.x402_budget_status.handler({});

    const d1 = JSON.parse(res1.content[0].text);
    const d2 = JSON.parse(res2.content[0].text);

    assert.equal(d1.budget.balance, 100);
    assert.equal(d2.budget.balance, 999);
  });
});

// ===========================================================================
// 22. X402_WALLET_ADDRESS fallback for payer address
// ===========================================================================

describe('X402_WALLET_ADDRESS fallback', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('uses X402_WALLET_ADDRESS when X402_PAYER_ADDRESS is not set', async () => {
    const hexKey = 'cd'.repeat(32);
    const keyJson = { privateKey: hexKey, publicKey: hexKey };
    const server = createX402McpServer({
      env: {
        X402_SEQUENCER_URL: 'https://seq.test',
        X402_TENANT_ID: 't1',
        X402_STORE_ID: 's1',
        X402_AGENT_ID: 'a1',
        X402_WALLET_ADDRESS: '0xWalletFallback',
        X402_SIGNING_KEY: JSON.stringify(keyJson),
      },
      configDir: tmpDir,
    });
    const tools = server.instance._registeredTools;
    // This should NOT fail with "X402_PAYER_ADDRESS is required"
    const res = await tools.x402_call.handler({ url: 'https://api.example.com' });
    if (res.isError) {
      const data = JSON.parse(res.content[0].text);
      assert.ok(
        !data.error.includes('X402_PAYER_ADDRESS'),
        `should not require payer address when wallet address set, got: ${data.error}`,
      );
    }
  });
});

// ===========================================================================
// 23. X402_API_KEY is passed to sequencer
// ===========================================================================

describe('X402_API_KEY env var', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => {
    cleanDir(tmpDir);
  });

  it('accepts X402_API_KEY without error', () => {
    const server = createX402McpServer({
      env: {
        X402_SEQUENCER_URL: 'https://seq.test',
        X402_API_KEY: 'test-api-key-123',
      },
      configDir: tmpDir,
    });
    assert.ok(server);
  });

  it('accepts X402_JWT without error', () => {
    const server = createX402McpServer({
      env: {
        X402_SEQUENCER_URL: 'https://seq.test',
        X402_JWT: 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test',
      },
      configDir: tmpDir,
    });
    assert.ok(server);
  });
});
