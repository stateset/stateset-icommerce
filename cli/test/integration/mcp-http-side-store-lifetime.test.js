/**
 * stateset-mcp-http must not open and close a second SQLite on the live store
 * once per request.
 *
 * The HTTP server builds a fresh MCP server for every request. Each one used to
 * open its own A2AStore (better-sqlite3) on the engine's store file and close
 * it when the request ended. better-sqlite3 bundles its own copy of SQLite, in
 * the same process as the engine's. POSIX fcntl locks belong to the process, so
 * that copy never sees the engine's locks: on close it believes it is the last
 * connection, checkpoints and unlinks `-wal`/`-shm`, and closing its descriptor
 * drops every lock the process holds on the file, the engine's included. Later
 * engine commits then go to an unlinked WAL that no new connection can read.
 *
 * The stopgap gives the HTTP server one long-lived side-store set per process
 * (`createStatesetMcpServer({ sharedStores })`). The process never closes it,
 * not even at shutdown: that close would checkpoint and unlink the WAL under
 * the still-open engine. Process exit releases it.
 *
 * The first block drives the real binary as a separate server process. A
 * `--import` probe (test/helpers/side-store-probe.mjs) reports each
 * better-sqlite3 open/close in that process. The store is inspected only from
 * other processes (test/helpers/sqlite-process.js) and through /proc/locks;
 * this process never opens it with better-sqlite3.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createMcpHandler } from '@modelcontextprotocol/server';

import { A2AStore } from '../../src/a2a/store.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { createStatesetV2McpServer } from '../../src/mcp/v2-server.js';
import { sqliteOperations } from '../helpers/sqlite-process.js';
import { startupBudgetMs } from '../helpers/startup-budget.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, '../../bin/stateset-mcp-http.js');
// Preloaded into the SERVER process only; importing it here would patch this
// process's better-sqlite3 as well.
const PROBE = pathToFileURL(path.resolve(__dirname, '../helpers/side-store-probe.mjs')).href;
/** Must match `PROBE_TAG` in test/helpers/side-store-probe.mjs. */
const PROBE_TAG = '[side-store-probe] ';

/** At least five requests per side-store tool. */
const REQUESTS = 6;

/** SQLite's SHARED lock range on the main database file (unix VFS). */
const SHARED_FIRST = 1073741826; // PENDING_BYTE (0x40000000) + 2
const SHARED_LAST = SHARED_FIRST + 510 - 1;

const ENVELOPE = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'side-store-lifetime-test', version: '0' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

function toolRequest(name, args, id) {
  return {
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
      'Mcp-Method': 'tools/call',
      'Mcp-Name': name,
      'MCP-Protocol-Version': '2026-07-28',
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: { name, arguments: args, _meta: ENVELOPE },
    }),
  };
}

function parseBody(text) {
  if (text.includes('data:')) {
    const datas = text.split('\n').filter((line) => line.startsWith('data:'));
    return datas.length > 0 ? JSON.parse(datas[datas.length - 1].slice(5).trim()) : {};
  }
  return text.trim() ? JSON.parse(text) : {};
}

/** The tool's own JSON result, failing on any transport, schema or tool error. */
function toolResult(name, rpc) {
  const result = rpc?.result;
  assert.ok(
    result && result.isError !== true,
    `${name} failed: ${(result?.content?.[0]?.text ?? JSON.stringify(rpc)).slice(0, 600)}`,
  );
  const parsed = JSON.parse(result.content[0].text);
  assert.notEqual(
    parsed.success,
    false,
    `${name} refused: ${JSON.stringify(parsed).slice(0, 600)}`,
  );
  return parsed;
}

let nextId = 1;
async function callTool(base, name, args = {}) {
  const { headers, body } = toolRequest(name, args, nextId++);
  const res = await fetch(`${base}/mcp`, { method: 'POST', headers, body });
  return toolResult(name, parseBody(await res.text()));
}

/** Environment for the server: isolated HOME and no ambient kernel/treasury/auth config. */
function serverEnv(home) {
  const env = { ...process.env, HOME: home, USERPROFILE: home };
  for (const key of Object.keys(env)) {
    if (/^(STATESET_KERNEL_|TREASURY_)/.test(key) || key === 'STATESET_MCP_API_KEYS') {
      delete env[key];
    }
  }
  return env;
}

function startServer(args, env) {
  const child = spawn(process.execPath, ['--import', PROBE, BIN, ...args], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env,
  });
  const state = { child, stderr: '', exit: null };
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => {
    state.stderr += chunk;
  });
  state.exited = new Promise((resolve) => {
    child.once('exit', (code, signal) => {
      state.exit = { code, signal };
      resolve(state.exit);
    });
  });
  return state;
}

/** Resolve the bound base URL from the startup line (`--port 0`). */
async function waitForListen(state, timeoutMs = startupBudgetMs(60_000)) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const match = state.stderr.match(/ on (http:\/\/\S+?)\/mcp /);
    if (match) return match[1];
    if (state.exit) {
      throw new Error(`server exited early (${JSON.stringify(state.exit)}):\n${state.stderr}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`server did not start within ${timeoutMs}ms:\n${state.stderr}`);
}

/** better-sqlite3 open/close events the server process reported for `file`. */
function probeEvents(state, file) {
  const wanted = realpathSync(file);
  return state.stderr
    .split('\n')
    .filter((line) => line.startsWith(PROBE_TAG))
    .map((line) => JSON.parse(line.slice(PROBE_TAG.length)))
    .filter((event) => existsSync(event.file) && realpathSync(event.file) === wanted)
    .map((event) => event.event);
}

/**
 * POSIX locks `pid` holds on `file`, from /proc/locks; null where unavailable.
 * Each entry is { type, start, end }.
 */
function posixLocks(pid, file) {
  if (process.platform !== 'linux' || !existsSync('/proc/locks')) return null;
  const inode = String(statSync(file).ino);
  return readFileSync('/proc/locks', 'utf8')
    .split('\n')
    .map((line) => line.trim().split(/\s+/))
    .filter(
      (fields) =>
        fields[1] === 'POSIX' && fields[4] === String(pid) && fields[5]?.split(':')[2] === inode,
    )
    .map((fields) => ({ type: fields[3], start: Number(fields[6]), end: Number(fields[7]) }));
}

const holdsSharedLock = (locks) =>
  locks.some((lock) => lock.start <= SHARED_FIRST && lock.end >= SHARED_LAST);

describe('stateset-mcp-http keeps one side-store connection per process', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-http-side-store-'));
  const home = path.join(dir, 'home');
  const dbPath = path.join(dir, 'store.db');
  const emails = [];
  let server;
  let base;

  before(async () => {
    mkdirSync(home);
    server = startServer(
      [
        '--port',
        '0',
        '--db',
        dbPath,
        '--apply',
        '--kernel-allow-legacy-writes',
        // a2a: the per-server A2AStore. circuit-breaker: a scoped-store service.
        '--domains',
        'a2a,circuit-breaker',
      ],
      serverEnv(home),
    );
    base = await waitForListen(server);
  });

  after(async () => {
    if (server && !server.exit) {
      server.child.kill('SIGKILL');
      await server.exited;
    }
    rmSync(dir, { recursive: true, force: true });
  });

  async function engineWrite(label) {
    const email = `side-store-${label}@example.test`;
    const created = await callTool(base, 'create_customer', {
      email,
      firstName: 'Side',
      lastName: `Store ${label}`,
    });
    assert.equal(created.success, true, JSON.stringify(created).slice(0, 400));
    emails.push(email);
  }

  it('(a) opens the side store once and does not close it between requests', async () => {
    // Preconditions: the engine store is in WAL mode and the engine holds its
    // SHARED lock, so the checks below are not vacuous.
    assert.ok(existsSync(`${dbPath}-wal`), 'precondition: the engine store is in WAL mode');
    const locksBefore = posixLocks(server.child.pid, dbPath);
    if (locksBefore !== null) {
      assert.ok(
        holdsSharedLock(locksBefore),
        `precondition: the engine holds a SHARED lock on its store: ${JSON.stringify(locksBefore)}`,
      );
    }

    // Engine writes interleaved with side-store requests. Failures are
    // collected so the connection count below is reported either way.
    const failures = [];
    const attempt = async (label, fn) => {
      try {
        await fn();
      } catch (error) {
        failures.push(`${label}: ${error.message.slice(0, 300)}`);
      }
    };
    for (let i = 1; i <= REQUESTS; i += 1) {
      const label = String(i).padStart(2, '0');
      await attempt(`engine write ${label}`, () => engineWrite(label));
      await attempt(`a2a_list_escrows ${label}`, async () => {
        assert.equal((await callTool(base, 'a2a_list_escrows')).count, 0);
      });
      await attempt(`agent_get_all_breaker_states ${label}`, async () => {
        assert.equal((await callTool(base, 'agent_get_all_breaker_states')).success, true);
      });
    }

    const events = probeEvents(server, dbPath);
    assert.deepEqual(
      events,
      ['open'],
      `${REQUESTS * 2} side-store requests must share one connection on the store, opened once ` +
        `and never closed while serving; the server reported ${JSON.stringify(events)}`,
    );
    assert.deepEqual(failures, []);
  });

  it('(c) keeps the store WAL and the server process lock on the store inode', () => {
    const problems = [];
    if (!existsSync(`${dbPath}-wal`)) {
      problems.push('store.db-wal was unlinked while the engine still had it open');
    }
    const locks = posixLocks(server.child.pid, dbPath);
    if (locks === null) {
      // Non-Linux: /proc/locks is unavailable; the WAL check above still applies.
    } else if (!holdsSharedLock(locks)) {
      problems.push(
        `the server process no longer holds a SHARED lock on the store inode: ${JSON.stringify(locks)}`,
      );
    }
    assert.deepEqual(problems, []);
  });

  it('(b) every interleaved engine write is visible from a separate process', async () => {
    const read = () =>
      sqliteOperations(dbPath, [
        {
          mode: 'all',
          sql: "SELECT email FROM customers WHERE email LIKE 'side-store-%@example.test' ORDER BY email",
        },
      ])[0].map((row) => row.email);

    assert.deepEqual(read(), [...emails].sort());

    // A write after another process has opened and closed the store must still
    // reach new connections (if the engine's locks were gone, that close could
    // itself checkpoint and unlink the WAL).
    await engineWrite('after-read');
    await callTool(base, 'a2a_list_escrows');
    assert.deepEqual(read(), [...emails].sort());
  });

  it('leaves the shared side store open at shutdown and keeps the store intact', async () => {
    server.child.kill('SIGTERM');
    const exit = await server.exited;
    assert.equal(exit.code, 0, `graceful shutdown must exit 0: ${JSON.stringify(exit)}`);
    // Closing better-sqlite3 while the engine is live would checkpoint and
    // unlink the WAL under it; exit, not the shutdown handler, releases it.
    assert.deepEqual(
      probeEvents(server, dbPath),
      ['open'],
      'the shutdown handler must not close the shared side store',
    );
    assert.ok(
      existsSync(`${dbPath}-wal`),
      'store.db-wal must survive SIGTERM (no side-store checkpoint/unlink at shutdown)',
    );
    // Every acknowledged engine write is there when the store is reopened.
    const emailsOnReopen = sqliteOperations(dbPath, [
      {
        mode: 'all',
        sql: "SELECT email FROM customers WHERE email LIKE 'side-store-%@example.test' ORDER BY email",
      },
    ])[0].map((row) => row.email);
    assert.deepEqual(emailsOnReopen, [...emails].sort());
  });
});

describe('createStatesetMcpServer side-store ownership', () => {
  const tempDirs = [];
  const tempDb = () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-side-store-owner-'));
    tempDirs.push(dir);
    return path.join(dir, 'store.db');
  };
  after(() => {
    for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true });
  });

  /** Record the stores that actually open and close. */
  function spyOnStores(t) {
    const opened = [];
    const closed = [];
    const init = A2AStore.prototype.init;
    const close = A2AStore.prototype.close;
    t.mock.method(A2AStore.prototype, 'init', function (...args) {
      const wasOpen = Boolean(this.db);
      const result = init.apply(this, args);
      if (!wasOpen && this.db) opened.push(this);
      return result;
    });
    t.mock.method(A2AStore.prototype, 'close', function (...args) {
      if (this.db) closed.push(this);
      return close.apply(this, args);
    });
    return { opened, closed };
  }

  it('(d) the default (stdio) path still opens and closes its own A2AStore', async (t) => {
    const dbPath = tempDb();
    const { opened, closed } = spyOnStores(t);
    const server = createStatesetMcpServer({
      commerce: {},
      dbPath,
      toolProfile: 'core',
      toolDomains: ['a2a'],
    });
    try {
      await server.servicesReady;
      const response = await server.executeTool('a2a_list_escrows', {});
      assert.equal(response.success, true, JSON.stringify(response).slice(0, 400));
      assert.equal(opened.length, 1);
      assert.equal(opened[0].dbPath, dbPath);
    } finally {
      await server.close();
    }
    assert.deepEqual(closed, opened, 'the server closes the store it opened');
    assert.equal(opened[0].db, null);
  });

  it('per-request servers use an injected shared store and never close it', async (t) => {
    const dbPath = tempDb();
    const shared = new A2AStore({ dbPath });
    const { opened, closed } = spyOnStores(t);
    const handler = createMcpHandler(() =>
      createStatesetV2McpServer({
        createServer: createStatesetMcpServer,
        commerce: {},
        dbPath,
        sharedStores: { a2aStore: shared },
        toolProfile: 'core',
        toolDomains: ['a2a'],
      }),
    );
    try {
      for (let id = 1; id <= 5; id += 1) {
        const { headers, body } = toolRequest('a2a_list_escrows', {}, id);
        const response = await handler.fetch(
          new Request('http://localhost/mcp', { method: 'POST', headers, body }),
        );
        toolResult('a2a_list_escrows', await response.json());
        // Per-request cleanup runs after the response; let it settle.
        await new Promise((resolve) => setImmediate(resolve));
        assert.deepEqual(closed, [], `request ${id} must not close any side store`);
        assert.notEqual(shared.db, null, `request ${id} must leave the shared store open`);
      }
      assert.deepEqual(opened, [shared], 'every request used the one injected store');
    } finally {
      await handler.close();
      shared.close();
    }
  });
});
