/**
 * ERC-8004 tools must not disturb the live commerce store.
 *
 * The erc8004 tools and the treasury identity resolver receive the server's
 * own `dbPath`, which is the native engine's store file. The JS module used to
 * open that file with a second bundled SQLite (better-sqlite3) in the engine's
 * process. POSIX fcntl locks belong to the process, so that copy could not see
 * the engine's locks, and closing any descriptor on the file dropped all of
 * them. The second copy then believed it was the last connection and
 * checkpointed and unlinked `-wal`/`-shm` while the engine still had them open;
 * later engine commits went to an unlinked WAL that no new connection could
 * read. The module now does that work in a child process.
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
import { createTreasuryIdentityResolver } from '../../src/mcp/tool-wrappers.js';
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

describe('erc8004 tools on a live engine store', () => {
  it('keep the engine WAL, its locks and every engine write visible to other processes', async () => {
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
    assert.ok(existsSync(`${dbPath}-wal`), 'precondition: the engine store has a WAL');
    assert.ok(existsSync(`${dbPath}-shm`), 'precondition: the engine store has a WAL index');

    const registered = await call('erc8004_register_identity', {
      registry: REGISTRY,
      agentId: AGENT_ID,
      agentUri: 'https://agents.example/42.json',
    });
    assert.equal(registered.identity.agent_id, AGENT_ID);
    assert.equal(registered.identity.agent_wallet, null);

    const linked = await call('erc8004_link_wallet', {
      registry: REGISTRY,
      agentId: AGENT_ID,
      agentWallet: WALLET,
      walletProofType: 'eip712',
      walletProofChainId: 8453,
    });
    assert.equal(linked.identity.agent_wallet, WALLET);

    const fetched = await call('erc8004_get_identity', { registry: REGISTRY, agentId: AGENT_ID });
    assert.equal(fetched.identity.agent_wallet, WALLET);
    assert.equal(fetched.identity.wallet_proof_chain_id, 8453);
    assert.equal(fetched.identity.active, true);

    const byWallet = await call('erc8004_get_by_wallet', { wallet: WALLET });
    assert.equal(byWallet.identity.agent_id, AGENT_ID);
    const listed = await call('erc8004_list_identities', { registry: REGISTRY });
    assert.equal(listed.count, 1);

    // The treasury charger resolves its identity from the same store.
    const resolver = createTreasuryIdentityResolver({
      registry: REGISTRY,
      dbPath,
      agentId: AGENT_ID,
    });
    assert.deepEqual(await resolver.getMetadata(), {
      erc8004: { registry: REGISTRY, agentId: AGENT_ID, wallet: WALLET, owner: null },
    });

    const problems = [];
    for (const suffix of ['-wal', '-shm']) {
      if (!existsSync(`${dbPath}${suffix}`)) {
        problems.push(`the engine ${suffix} file was unlinked while the engine still had it open`);
      }
    }
    const locksAfter = storeLocksHeldByThisProcess();
    if (locksAfter !== null && locksAfter.length === 0) {
      problems.push('the engine POSIX lock on its own store was released');
    }

    // A separate process reads (and then closes) the store. If the engine's
    // locks were gone, that close could also checkpoint and unlink the WAL.
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
      problems.push('an engine write made after the erc8004 calls is invisible to new connections');
    }
    assert.deepEqual(problems, []);

    assert.deepEqual(final.emails, ['after@example.com', 'before@example.com']);
    // Stored exactly as the JS module always stored it.
    assert.equal(final.identity?.agent_wallet, WALLET);
    assert.equal(final.identity?.wallet_proof_type, 'eip712');
    assert.equal(final.identity?.wallet_proof_chain_id, 8453);
    assert.equal(final.identity?.active, 1);
    assert.match(final.identity?.created_at ?? '', /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/);
  });
});
