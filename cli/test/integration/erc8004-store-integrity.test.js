/**
 * ERC-8004 tools must not disturb the live commerce store.
 *
 * The erc8004 tools receive the server's own `dbPath`, which is the native
 * engine's store file. The JS module opened that file with a second bundled
 * SQLite (better-sqlite3) in the engine's process. POSIX fcntl locks belong to
 * the process, so that copy cannot see the engine's locks, and closing any
 * descriptor on the file drops all of them. The second copy then believes it is
 * the last connection and may checkpoint and unlink `-wal`/`-shm` while the
 * engine still has them open; later engine commits go to an unlinked WAL that
 * no new connection can read.
 *
 * This drives the real MCP server on a file-backed store and inspects the
 * store only from a separate process (`sqlite-process.js`). It never opens the
 * store with better-sqlite3 here: doing that is the defect under test.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { sqliteOperations } from '../helpers/sqlite-process.js';

// HOME must be isolated before any module computes a path from it.
const dir = mkdtempSync(path.join(os.tmpdir(), 'erc8004-store-integrity-'));
const home = path.join(dir, 'home');
mkdirSync(home);
const savedHome = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE };
process.env.HOME = home;
process.env.USERPROFILE = home;

const { createStatesetMcpServer } = await import('../../src/mcp-server.js');

const dbPath = path.join(dir, 'store.db');
const REGISTRY = 'eip155:8453:0x0000000000000000000000000000000000008004';
const AGENT_ID = 'agent-42';
const WALLET = '0x00000000000000000000000000000000000000aa';

let server;

/** Call a tool and return the tool's own result, failing on any refusal. */
async function call(name, params = {}) {
  const response = await server.executeTool(name, params);
  const result = response?.result;
  assert.ok(
    response?.success === true && result?.success !== false,
    `${name} failed: ${JSON.stringify(response?.error ?? result).slice(0, 400)}`,
  );
  return result;
}

/**
 * POSIX locks this process holds on the store file, from /proc/locks.
 * Returns null where /proc/locks is unavailable (non-Linux).
 */
function storeLocksHeldByThisProcess() {
  if (!existsSync('/proc/locks')) return null;
  const inode = String(statSync(dbPath).ino);
  return readFileSync('/proc/locks', 'utf8')
    .split('\n')
    .map((line) => line.trim().split(/\s+/))
    .filter(
      (fields) =>
        fields[1] === 'POSIX' &&
        fields[4] === String(process.pid) &&
        fields[5]?.split(':')[2] === inode,
    );
}

/** Read the store from a separate process (its own SQLite, its own locks). */
function inspect() {
  const [customers, identity] = sqliteOperations(dbPath, [
    { mode: 'all', sql: 'SELECT email FROM customers ORDER BY email' },
    {
      mode: 'get',
      sql: 'SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?',
      params: [REGISTRY, AGENT_ID],
    },
  ]);
  return { emails: customers.map((row) => row.email), identity };
}

before(async () => {
  server = createStatesetMcpServer({ dbPath, allowApply: true, toolProfile: 'all' });
});

after(async () => {
  await server?.close?.();
  for (const [key, value] of Object.entries(savedHome)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
  rmSync(dir, { recursive: true, force: true });
});

// Reproduces deterministically on master (Linux, better-sqlite3 3.49.x): the
// erc8004 calls unlink store.db-wal/-shm and drop the engine's lock, and the
// next engine write never reaches other connections. The fix (route the tools
// through `commerce.erc8004`) is blocked on stored-encoding differences between
// the JS module and the engine: wallet_proof_type 'eip712' vs 'eip_712',
// free-form wallet_proof_deadline, REAL/negative wallet_proof_chain_id and
// caller-chosen ids that the engine cannot read. Drop `todo` with the fix.
const BLOCKED =
  'erc8004 tools open the engine store with a second SQLite; fix blocked on legacy row encodings';

describe('erc8004 tools on a live engine store', () => {
  it(
    'keep the engine WAL, its locks and every engine write visible to other processes',
    { todo: BLOCKED },
    async () => {
      await call('create_customer', {
        email: 'before@example.com',
        firstName: 'Before',
        lastName: 'Identity',
      });

      const locksBefore = storeLocksHeldByThisProcess();
      if (locksBefore !== null) {
        assert.ok(
          locksBefore.length > 0,
          'precondition: the engine holds a POSIX lock on its WAL-mode store',
        );
      }
      assert.ok(existsSync(`${dbPath}-wal`), 'precondition: the engine store is in WAL mode');

      const registered = await call('erc8004_register_identity', {
        registry: REGISTRY,
        agentId: AGENT_ID,
        agentUri: 'https://agents.example/42.json',
        agentWallet: WALLET,
        walletProofType: 'eip712',
        walletProofChainId: 8453,
      });
      assert.equal(registered.identity.agent_id, AGENT_ID);

      const fetched = await call('erc8004_get_identity', { registry: REGISTRY, agentId: AGENT_ID });
      assert.equal(fetched.identity.agent_wallet, WALLET);
      assert.equal(fetched.identity.wallet_proof_chain_id, 8453);
      assert.equal(fetched.identity.active, true);

      const problems = [];
      if (!existsSync(`${dbPath}-wal`)) {
        problems.push('the engine WAL was unlinked while the engine still had it open');
      }
      const locksAfter = storeLocksHeldByThisProcess();
      if (locksAfter !== null && locksAfter.length === 0) {
        problems.push('the engine POSIX lock on its own store was released');
      }

      // A separate process reads (and then closes) the store. If the engine's
      // locks are gone, that close may also checkpoint and unlink the WAL.
      const middle = inspect();
      assert.deepEqual(middle.emails, ['before@example.com']);
      assert.equal(middle.identity?.agent_wallet, WALLET);

      await call('create_customer', {
        email: 'after@example.com',
        firstName: 'After',
        lastName: 'Identity',
      });

      const final = inspect();
      if (!final.emails.includes('after@example.com')) {
        problems.push(
          'an engine write made after the erc8004 calls is invisible to new connections',
        );
      }
      assert.deepEqual(problems, []);

      assert.deepEqual(final.emails, ['after@example.com', 'before@example.com']);
      assert.equal(final.identity?.agent_wallet, WALLET);
      // The JS module stores 'eip712'; the engine stores 'eip_712'.
      assert.match(final.identity?.wallet_proof_type ?? '', /^eip_?712$/);
      assert.equal(final.identity?.wallet_proof_chain_id, 8453);
      assert.equal(final.identity?.active, 1);
    },
  );
});
