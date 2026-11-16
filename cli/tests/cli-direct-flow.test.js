import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { existsSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const BIN_DIR = join(__dirname, '..', 'bin');

function runDirect(args = []) {
  return runNodeScript(join(BIN_DIR, 'stateset-direct.js'), args);
}

function newDbPath() {
  return join(
    tmpdir(),
    `stateset-direct-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.db`,
  );
}

function cleanupDb(dbPath) {
  if (!dbPath) return;
  for (const suffix of ['', '-wal', '-shm']) {
    const path = `${dbPath}${suffix}`;
    if (existsSync(path)) {
      try {
        unlinkSync(path);
      } catch {
        // Ignore cleanup errors
      }
    }
  }
}

describe('stateset-direct flows', () => {
  let dbPath;

  afterEach(() => {
    cleanupDb(dbPath);
    dbPath = null;
  });

  it('creates and lists customers', () => {
    dbPath = newDbPath();

    const create = runDirect([
      '--db',
      dbPath,
      '--apply',
      '--yes',
      '--json',
      'customers',
      'create',
      'flow@example.com',
      'Flow',
      'Test',
    ]);

    assert.equal(create.status, 0, create.stderr || create.stdout);
    const created = JSON.parse(create.stdout);
    assert.equal(created.email, 'flow@example.com');

    const list = runDirect(['--db', dbPath, '--json', 'customers', 'list']);

    assert.equal(list.status, 0, list.stderr || list.stdout);
    const customers = JSON.parse(list.stdout);
    assert.ok(customers.some((c) => c.email === 'flow@example.com'));
  });

  it('creates inventory and reports stock', () => {
    dbPath = newDbPath();

    const create = runDirect([
      '--db',
      dbPath,
      '--apply',
      '--yes',
      '--json',
      'inventory',
      'create',
      'INV-TEST',
      'TestItem',
      '7',
    ]);

    assert.equal(create.status, 0, create.stderr || create.stdout);
    const item = JSON.parse(create.stdout);
    assert.equal(item.sku, 'INV-TEST');

    const stockRes = runDirect(['--db', dbPath, '--json', 'inventory', 'stock', 'INV-TEST']);

    assert.equal(stockRes.status, 0, stockRes.stderr || stockRes.stdout);
    const stock = JSON.parse(stockRes.stdout);
    assert.equal(stock.sku, 'INV-TEST');
    const onHand = stock.totalOnHand ?? stock.total_on_hand ?? stock.totalOnhand;
    assert.equal(onHand, '7');

    // `inventory list` lists inventory items, so an item no product carries
    // appears (it used to walk product variants and show nothing here).
    const listRes = runDirect(['--db', dbPath, '--json', 'inventory', 'list']);
    assert.equal(listRes.status, 0, listRes.stderr || listRes.stdout);
    assert.deepEqual(JSON.parse(listRes.stdout), [
      { sku: 'INV-TEST', name: 'TestItem', onHand: '7', available: '7', allocated: '0' },
    ]);
    const tableRes = runDirect(['--db', dbPath, 'inventory', 'list']);
    assert.equal(tableRes.status, 0, tableRes.stderr || tableRes.stdout);
    assert.match(tableRes.stdout, /INV-TEST/);
  });

  it('requires --apply for write actions', () => {
    dbPath = newDbPath();

    const result = runDirect([
      '--db',
      dbPath,
      '--json',
      'customers',
      'create',
      'preview@example.com',
      'Preview',
      'Mode',
    ]);

    assert.equal(result.status, 1);
    const payload = JSON.parse(result.stdout);
    assert.ok(/preview mode/i.test(payload.error));
  });

  it('requires confirmation without --yes in non-interactive mode', () => {
    dbPath = newDbPath();

    const result = runDirect([
      '--db',
      dbPath,
      '--apply',
      'customers',
      'create',
      'confirm@example.com',
      'Confirm',
      'Mode',
    ]);

    assert.equal(result.status, 1);
    const output = `${result.stdout}${result.stderr}`;
    assert.ok(/confirmation required/i.test(output));
  });
});
