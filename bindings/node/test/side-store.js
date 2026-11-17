/**
 * Side stores (`commerce.openSideStore()`): a synchronous SQLite handle on the
 * engine's own database file, opened by the engine's own SQLite library.
 *
 * Two groups of tests:
 *
 * - the better-sqlite3-shaped API the CLI's side stores rely on (binding and
 *   reading values, positional and named parameters, transactions, errors,
 *   use after close);
 * - the reason it exists: a second SQLite library in the process (here
 *   better-sqlite3, when it can be found) that opens and closes the engine's
 *   file drops the engine's POSIX locks and unlinks its WAL, losing later
 *   commits. A side store must not.
 */

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');
const { Commerce, SideStore, SideStoreStatement } = require('../index.js');

function tempDir(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-side-store-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

async function customer(commerce, email) {
  return commerce.customers.create({ email, firstName: 'Side', lastName: 'Store' });
}

/**
 * Open the file in a FRESH process with the engine and report what it sees.
 * A fresh process has no lock bookkeeping of its own to confuse, so it shows
 * exactly what the file (plus its current WAL) holds. Closing it is also a
 * probe: if this process's engine still holds its shared lock, the child's
 * close cannot checkpoint and remove the WAL.
 */
function inspectInFreshProcess(file) {
  const script = `
    const { Commerce } = require(${JSON.stringify(path.join(__dirname, '..', 'index.js'))});
    (async () => {
      const commerce = await Commerce.open(process.argv[1]);
      const out = {};
      try {
        const customers = await commerce.customers.list({ limit: 1000 });
        out.emails = customers.map((c) => c.email).sort();
        const side = commerce.openSideStore();
        out.integrity = side.pragma('integrity_check', { simple: true });
        try { out.notes = side.prepare('SELECT body FROM side_notes ORDER BY id').all().map((r) => r.body); }
        catch (error) { out.notesError = error.message; }
      } catch (error) {
        out.error = error.message;
      }
      await commerce.close();
      process.stdout.write(JSON.stringify(out));
    })().catch((error) => { process.stdout.write(JSON.stringify({ error: error.message })); });
  `;
  const result = spawnSync(process.execPath, ['-e', script, file], { encoding: 'utf8', timeout: 60_000 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

/** Create customers from a FRESH process (its own engine, its own locks). */
function writeInFreshProcess(file, emails) {
  const script = `
    const { Commerce } = require(${JSON.stringify(path.join(__dirname, '..', 'index.js'))});
    (async () => {
      const commerce = await Commerce.open(process.argv[1]);
      for (const email of JSON.parse(process.argv[2])) {
        await commerce.customers.create({ email, firstName: 'Other', lastName: 'Process' });
      }
      await commerce.close();
    })().catch((error) => { console.error(error); process.exit(1); });
  `;
  const result = spawnSync(process.execPath, ['-e', script, file, JSON.stringify(emails)], {
    encoding: 'utf8',
    timeout: 60_000,
  });
  assert.equal(result.status, 0, result.stderr);
}

/**
 * Engine writes, a side-table connection opens/writes/closes, the engine
 * writes again, ANOTHER process commits, the engine writes and closes. Returns
 * every customer email the file holds afterwards.
 */
async function interleave(file, openAndCloseSideTable) {
  const commerce = await Commerce.open(file);
  await customer(commerce, 'engine-1@example.com');
  openAndCloseSideTable(commerce, file);
  await customer(commerce, 'engine-2@example.com');
  writeInFreshProcess(file, ['other-1@example.com', 'other-2@example.com']);
  await customer(commerce, 'engine-3@example.com');
  await commerce.close();
  return inspectInFreshProcess(file);
}

/** better-sqlite3 from the CLI workspace, if this checkout has it installed. */
function loadBetterSqlite3() {
  try {
    const resolved = require.resolve('better-sqlite3', {
      paths: [path.join(__dirname, '..', '..', '..', 'cli')],
    });
    return require(resolved);
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

test('openSideStore returns a SideStore whose statements are SideStoreStatements', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  assert.ok(side instanceof SideStore);
  assert.equal(side.open, true);
  assert.equal(side.inTransaction, false);
  const statement = side.prepare('SELECT 1 AS one');
  assert.ok(statement instanceof SideStoreStatement);
  assert.equal(statement.source, 'SELECT 1 AS one');
  assert.equal(statement.reader, true);
  assert.equal(side.prepare('CREATE TABLE IF NOT EXISTS t (x)').reader, false);
});

test('sharedSideStore() is one side store per engine, reopened once closed', async (t) => {
  const commerce = new Commerce(':memory:');
  const other = new Commerce(':memory:');
  t.after(() => Promise.all([commerce.close(), other.close()]));
  const shared = commerce.sharedSideStore();
  assert.ok(shared instanceof SideStore);
  assert.equal(commerce.sharedSideStore(), shared);
  assert.notEqual(other.sharedSideStore(), shared);
  // Methods reached through a wrapper are bound to the instance.
  const wrapper = { sharedSideStore: commerce.sharedSideStore.bind(commerce) };
  assert.equal(wrapper.sharedSideStore(), shared);
  shared.close();
  const reopened = commerce.sharedSideStore();
  assert.notEqual(reopened, shared);
  assert.equal(reopened.open, true);
  await commerce.close();
  assert.equal(reopened.open, false);
  assert.throws(() => commerce.sharedSideStore(), (error) => error.code === 'PRECONDITION_FAILED');
});

test('values bind and read back as better-sqlite3 binds and reads them', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE typed (i INTEGER, r REAL, s TEXT, b BLOB, n, untyped)');
  side
    .prepare('INSERT INTO typed (i, r, s, b, n, untyped) VALUES (?, ?, ?, ?, ?, ?)')
    .run(42, 1.5, 'héllo', Buffer.from([0, 1, 255]), null, 7);
  const row = side.prepare('SELECT * FROM typed').get();
  assert.deepEqual(
    { ...row, b: [...row.b] },
    { i: 42, r: 1.5, s: 'héllo', b: [0, 1, 255], n: null, untyped: 7 },
  );
  assert.ok(Buffer.isBuffer(row.b));

  // A JavaScript number binds as REAL, exactly as better-sqlite3 binds it;
  // a bigint binds as INTEGER.
  assert.deepEqual(side.prepare('SELECT typeof(?) AS t').get(5), { t: 'real' });
  assert.deepEqual(side.prepare('SELECT typeof(?) AS t').get(5n), { t: 'integer' });
  assert.deepEqual(side.prepare('SELECT typeof(?) AS t').get(undefined), { t: 'null' });
  assert.deepEqual(side.prepare('SELECT typeof(?) AS t').get(new Uint8Array([1])), { t: 'blob' });
  assert.deepEqual(side.prepare('SELECT ? AS v').get(9007199254740993n), { v: 9007199254740992 });
  assert.deepEqual(side.prepare('SELECT CAST(? AS TEXT) AS v').get(9007199254740993n), {
    v: '9007199254740993',
  });

  for (const unbindable of [true, new Date(), Symbol('x'), () => 1]) {
    assert.throws(
      () => side.prepare('SELECT ? AS v').get(unbindable),
      (error) => error.code === 'VALIDATION' && /can only bind/.test(error.message),
    );
  }
  assert.throws(
    () => side.prepare('SELECT ? AS v').get(2n ** 64n),
    (error) => error.code === 'VALIDATION' && /64-bit/.test(error.message),
  );
});

test('positional, array and named parameters follow better-sqlite3', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE p (a, b, c)');
  const insert = side.prepare('INSERT INTO p VALUES (?, ?, ?)');
  insert.run(1, 2, 3);
  insert.run([4, 5, 6]);
  insert.run(7, [8, 9]);
  assert.deepEqual(side.prepare('SELECT a, b, c FROM p ORDER BY a').all(), [
    { a: 1, b: 2, c: 3 },
    { a: 4, b: 5, c: 6 },
    { a: 7, b: 8, c: 9 },
  ]);

  for (const prefix of ['@', ':', '$']) {
    const named = side.prepare(`SELECT ${prefix}x AS x, ${prefix}y AS y, ${prefix}x AS again`);
    assert.deepEqual(named.get({ x: 'one', y: 'two' }), { x: 'one', y: 'two', again: 'one' });
  }
  const mixed = side.prepare('SELECT ? AS first, @name AS name, ? AS second');
  assert.deepEqual(mixed.get('a', { name: 'n' }, 'b'), { first: 'a', name: 'n', second: 'b' });
  // An object with no named parameters to fill is ignored, as in better-sqlite3.
  assert.deepEqual(side.prepare('SELECT ? AS v').get(1, { unused: true }), { v: 1 });

  const fails = (fn, code, pattern) =>
    assert.throws(fn, (error) => error.code === code && pattern.test(error.message));
  fails(() => insert.run(1, 2), 'VALIDATION', /Too few parameter values/);
  fails(() => insert.run(1, 2, 3, 4), 'VALIDATION', /Too many parameter values/);
  fails(() => mixed.get('a', 'b'), 'VALIDATION', /Missing named parameters/);
  fails(() => mixed.get('a', {}, 'b'), 'VALIDATION', /Missing named parameter "name"/);
  assert.throws(() => mixed.get({ name: 1 }, { name: 2 }), {
    name: 'TypeError',
    message: 'You cannot specify named parameters in two different objects',
  });
});

test('run reports changes and lastInsertRowid; get and all read rows', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE r (id INTEGER PRIMARY KEY, v TEXT)');
  const insert = side.prepare('INSERT INTO r (v) VALUES (?)');
  assert.deepEqual(insert.run('a'), { changes: 1, lastInsertRowid: 1 });
  assert.deepEqual(insert.run('b'), { changes: 1, lastInsertRowid: 2 });
  assert.deepEqual(side.prepare("UPDATE r SET v = 'z'").run(), { changes: 2, lastInsertRowid: 2 });
  // A statement that changed nothing reports 0, not the previous count.
  assert.equal(side.prepare('UPDATE r SET v = 1 WHERE id = 99').run().changes, 0);
  assert.equal(side.prepare('CREATE TABLE IF NOT EXISTS r2 (x)').run().changes, 0);
  // `INSERT ... RETURNING` through run() still writes.
  assert.equal(side.prepare("INSERT INTO r (v) VALUES ('c') RETURNING id").run().changes, 1);
  assert.equal(side.prepare('SELECT * FROM r WHERE id = ?').get(99), undefined);
  assert.deepEqual(side.prepare('SELECT id FROM r ORDER BY id').all(), [{ id: 1 }, { id: 2 }, { id: 3 }]);
  assert.deepEqual(side.prepare('SELECT id FROM r WHERE id > 5').all(), []);
  // A repeated column name keeps the last value.
  assert.deepEqual(side.prepare('SELECT 1 AS x, 2 AS x').get(), { x: 2 });
  assert.throws(
    () => side.prepare('DELETE FROM r WHERE id = 99').get(),
    (error) => error.code === 'VALIDATION' && /does not return data/.test(error.message),
  );
  assert.throws(() => side.prepare('DELETE FROM r WHERE id = 99').all(), /does not return data/);
});

test('exec runs scripts and returns the side store; pragma reads settings', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  assert.equal(side.exec('CREATE TABLE e (x); INSERT INTO e VALUES (1); INSERT INTO e VALUES (2);'), side);
  assert.deepEqual(side.prepare('SELECT count(*) AS n FROM e').get(), { n: 2 });
  assert.equal(side.pragma('journal_mode', { simple: true }), 'wal');
  assert.equal(side.pragma('foreign_keys', { simple: true }), 1);
  assert.deepEqual(side.pragma('journal_mode'), [{ journal_mode: 'wal' }]);
  assert.deepEqual(
    side.pragma('table_info(e)').map((column) => column.name),
    ['x'],
  );
  // A pragma that sets a value returns no rows.
  assert.deepEqual(side.pragma('user_version = 7'), []);
  assert.equal(side.pragma('user_version', { simple: true }), 7);
  assert.throws(() => side.pragma('journal_mode', { simple: 'yes' }), TypeError);
  assert.throws(() => side.exec(42), TypeError);
});

test('prepare refuses empty, multi-statement and invalid SQL with SQLite messages', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  assert.throws(() => side.prepare('  -- nothing\n'), /contains no statements/);
  assert.throws(() => side.prepare('SELECT 1; SELECT 2'), /more than one statement/);
  assert.throws(() => side.prepare('SELECT 1; SELECT * FROM missing'), /more than one statement/);
  // A trailing semicolon, whitespace or comment is not a second statement.
  assert.equal(side.prepare('SELECT 1 AS x; -- done\n ;').get().x, 1);
  assert.throws(
    () => side.prepare('SELEKT 1'),
    (error) => error.code === 'SQLITE_ERROR' && error.message === 'near "SELEKT": syntax error',
  );
  assert.throws(
    () => side.prepare('SELECT * FROM no_such_table'),
    (error) => error.code === 'SQLITE_ERROR' && error.message === 'no such table: no_such_table',
  );
});

test('SQLite failures carry better-sqlite3 codes and SQLite messages', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec(`
    CREATE TABLE parent (id INTEGER PRIMARY KEY);
    CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id), tag TEXT UNIQUE);
    INSERT INTO parent VALUES (1);
    INSERT INTO child VALUES (1, 1, 'a');
  `);
  const insert = side.prepare('INSERT INTO child VALUES (?, ?, ?)');
  const fails = (args, code, message) =>
    assert.throws(
      () => insert.run(...args),
      (error) =>
        error instanceof Error &&
        error.code === code &&
        error.message === message &&
        typeof error.details?.sqliteCode === 'number',
    );
  fails([2, 1, 'a'], 'SQLITE_CONSTRAINT_UNIQUE', 'UNIQUE constraint failed: child.tag');
  fails([1, 1, 'b'], 'SQLITE_CONSTRAINT_PRIMARYKEY', 'UNIQUE constraint failed: child.id');
  fails([3, null, 'c'], 'SQLITE_CONSTRAINT_NOTNULL', 'NOT NULL constraint failed: child.parent_id');
  // Foreign keys are on, as they are in better-sqlite3 and in the engine.
  fails([4, 99, 'd'], 'SQLITE_CONSTRAINT_FOREIGNKEY', 'FOREIGN KEY constraint failed');
});

test('transaction commits, rolls back on throw and nests with savepoints', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  const other = commerce.openSideStore();
  side.exec('CREATE TABLE tx (v TEXT)');
  const insert = side.prepare('INSERT INTO tx VALUES (?)');
  const values = () => side.prepare('SELECT v FROM tx ORDER BY rowid').all().map((r) => r.v);
  const count = (store) => store.prepare('SELECT count(*) AS n FROM tx').get().n;

  const insertMany = side.transaction((items) => {
    for (const item of items) insert.run(item);
    // Every statement inside runs on this side store's one connection: the
    // writes are visible here and invisible to another connection until the
    // commit.
    assert.equal(side.inTransaction, true);
    assert.equal(count(side), items.length);
    assert.equal(count(other), 0);
    return items.length;
  });
  assert.equal(insertMany(['a', 'b']), 2);
  assert.equal(side.inTransaction, false);
  assert.equal(count(other), 2);

  const failing = side.transaction(() => {
    insert.run('lost');
    throw new Error('boom');
  });
  assert.throws(() => failing(), /boom/);
  assert.equal(side.inTransaction, false);
  assert.deepEqual(values(), ['a', 'b']);

  // A nested transaction is a savepoint: its failure undoes only its own
  // writes, and the outer transaction still commits.
  const inner = side.transaction(() => {
    insert.run('inner');
    throw new Error('inner failed');
  });
  const outer = side.transaction(() => {
    insert.run('outer');
    assert.throws(() => inner(), /inner failed/);
    assert.equal(side.inTransaction, true);
    insert.run('after');
  });
  outer();
  assert.deepEqual(values(), ['a', 'b', 'outer', 'after']);

  // The BEGIN modes, the `database` back-reference and `this` pass through.
  const mode = side.transaction(function () {
    insert.run('immediate');
    return this;
  });
  assert.equal(mode.immediate.call('ctx'), 'ctx');
  assert.equal(mode.database, side);
  assert.equal(typeof mode.deferred, 'function');
  assert.equal(typeof mode.exclusive, 'function');
  mode.exclusive();

  // A body that returns a promise is refused and rolled back.
  const asyncBody = side.transaction(async () => insert.run('async'));
  assert.throws(() => asyncBody(), /cannot return a promise/);
  assert.equal(side.inTransaction, false);
  assert.equal(values().includes('async'), false);
  assert.throws(() => side.transaction('nope'), TypeError);
});

test('close() ends this side store only; its statements then fail cleanly', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE c (x)');
  const statement = side.prepare('SELECT count(*) AS n FROM c');
  side.close();
  side.close();
  assert.equal(side.open, false);
  assert.equal(side.inTransaction, false);
  for (const call of [
    () => statement.get(),
    () => statement.all(),
    () => statement.run(),
    () => side.prepare('SELECT 1'),
    () => side.exec('SELECT 1'),
    () => side.pragma('user_version'),
  ]) {
    assert.throws(call, (error) => error.code === 'PRECONDITION_FAILED' && /side store is closed/.test(error.message));
  }
  // The engine and other side stores are unaffected.
  assert.equal(await commerce.customers.count(), 0);
  assert.deepEqual(commerce.openSideStore().prepare('SELECT count(*) AS n FROM c').get(), { n: 0 });
});

test('Commerce.close() closes its side stores; later calls fail, never crash', async () => {
  const commerce = new Commerce(':memory:');
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE k (x)');
  const statement = side.prepare('INSERT INTO k VALUES (?)');
  const pending = side.transaction(() => statement.run(1));
  await commerce.close();
  assert.equal(side.open, false);
  for (const call of [() => statement.run(1), () => side.prepare('SELECT 1'), () => pending(), () => side.exec('')]) {
    assert.throws(call, (error) => error.code === 'PRECONDITION_FAILED' && /Commerce instance is closed/.test(error.message));
  }
  side.close();
  assert.throws(() => commerce.openSideStore(), (error) => error.code === 'PRECONDITION_FAILED');
});

test('Commerce.close() still releases the file when a side store is open', async (t) => {
  const file = path.join(tempDir(t), 'store.db');
  const commerce = await Commerce.open(file);
  await customer(commerce, 'release@example.com');
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE side_notes (id INTEGER PRIMARY KEY, body TEXT)');
  side.prepare('INSERT INTO side_notes (body) VALUES (?)').run('kept');
  assert.ok(fs.existsSync(`${file}-wal`));
  await commerce.close();
  // The last connection to close checkpointed and removed the WAL, which only
  // happens once the side store's connection is closed too.
  assert.equal(fs.existsSync(`${file}-wal`), false);
  assert.equal(fs.existsSync(`${file}-shm`), false);
  const seen = inspectInFreshProcess(file);
  assert.deepEqual(seen.emails, ['release@example.com']);
  assert.deepEqual(seen.notes, ['kept']);
  assert.equal(seen.integrity, 'ok');
});

test(':memory: side stores reach the engine’s private database, and only it', async (t) => {
  const commerce = new Commerce(':memory:');
  const unrelated = new Commerce(':memory:');
  t.after(() => Promise.all([commerce.close(), unrelated.close()]));
  await customer(commerce, 'memory@example.com');
  const side = commerce.openSideStore();
  // The side store sees the engine's committed rows...
  assert.deepEqual(side.prepare('SELECT email FROM customers').all(), [{ email: 'memory@example.com' }]);
  // ...and another side store of the same engine sees its writes.
  side.exec('CREATE TABLE shared (v TEXT)');
  side.prepare('INSERT INTO shared VALUES (?)').run('x');
  assert.deepEqual(commerce.openSideStore().prepare('SELECT v FROM shared').all(), [{ v: 'x' }]);
  // A different :memory: engine is a different database.
  const elsewhere = unrelated.openSideStore();
  assert.deepEqual(elsewhere.prepare('SELECT count(*) AS n FROM customers').get(), { n: 0 });
  assert.throws(() => elsewhere.prepare('SELECT v FROM shared'), /no such table: shared/);
  // Engine writes after the side store opened are visible to it too.
  await customer(commerce, 'later@example.com');
  assert.equal(side.prepare('SELECT count(*) AS n FROM customers').get().n, 2);
  assert.equal(side.pragma('integrity_check', { simple: true }), 'ok');
});

test('a :memory: side store keeps working while the engine writes concurrently', async (t) => {
  const commerce = new Commerce(':memory:');
  t.after(() => commerce.close());
  const side = commerce.openSideStore();
  side.exec('CREATE TABLE ticks (n INTEGER)');
  const tick = side.prepare('INSERT INTO ticks VALUES (?)');
  const writes = Array.from({ length: 20 }, (_, i) => customer(commerce, `c${i}@example.com`));
  const batch = side.transaction((n) => {
    for (let i = 0; i < n; i += 1) tick.run(i);
  });
  for (let round = 0; round < 10; round += 1) batch(10);
  await Promise.all(writes);
  assert.equal(side.prepare('SELECT count(*) AS n FROM ticks').get().n, 100);
  assert.equal(await commerce.customers.count(), 20);
});

// ---------------------------------------------------------------------------
// The corruption this exists to prevent
// ---------------------------------------------------------------------------

test('closing a side store keeps the engine’s locks and WAL, so no commit is lost', async (t) => {
  const file = path.join(tempDir(t), 'store.db');
  const commerce = await Commerce.open(file);
  t.after(() => commerce.close());
  await customer(commerce, 'before@example.com');
  assert.ok(fs.existsSync(`${file}-wal`), 'the engine runs in WAL mode');

  const side = commerce.openSideStore();
  side.exec('CREATE TABLE side_notes (id INTEGER PRIMARY KEY, body TEXT)');
  side.prepare('INSERT INTO side_notes (body) VALUES (?)').run('side write');
  side.close();

  // Closing the side store left the engine's WAL in place...
  assert.ok(fs.existsSync(`${file}-wal`), 'side store close must not unlink the engine WAL');
  assert.ok(fs.existsSync(`${file}-shm`), 'side store close must not unlink the engine shm');

  // ...and the engine keeps committing where everyone can see it.
  await customer(commerce, 'after@example.com');
  const seen = inspectInFreshProcess(file);
  assert.deepEqual(seen, {
    emails: ['after@example.com', 'before@example.com'],
    integrity: 'ok',
    notes: ['side write'],
  });

  // The fresh process's close could not checkpoint and remove the WAL: this
  // process's engine still holds its shared lock on the file.
  assert.ok(fs.existsSync(`${file}-wal`), 'the engine must still hold its shared lock');
  await customer(commerce, 'last@example.com');
  assert.deepEqual(inspectInFreshProcess(file).emails, [
    'after@example.com',
    'before@example.com',
    'last@example.com',
  ]);
});

test('the same sequence through a second SQLite library loses engine commits', async (t) => {
  const Database = loadBetterSqlite3();
  if (!Database) {
    t.skip('better-sqlite3 is not installed in the CLI workspace');
    return;
  }
  const file = path.join(tempDir(t), 'store.db');
  const commerce = await Commerce.open(file);
  t.after(() => commerce.close());
  await customer(commerce, 'before@example.com');

  // The pre-fix pattern: a side store on the engine's file through another
  // SQLite copy. Its close believes it is the last connection, checkpoints,
  // unlinks the WAL and releases the process's locks.
  const foreign = new Database(file);
  foreign.exec('CREATE TABLE side_notes (id INTEGER PRIMARY KEY, body TEXT)');
  foreign.prepare('INSERT INTO side_notes (body) VALUES (?)').run('side write');
  foreign.close();
  assert.equal(fs.existsSync(`${file}-wal`), false, 'the foreign close unlinked the engine WAL');

  // The engine still "commits" -- into the unlinked WAL nobody else can see.
  await customer(commerce, 'after@example.com');
  const seen = inspectInFreshProcess(file);
  assert.equal(seen.emails?.includes('after@example.com') ?? false, false, JSON.stringify(seen));
});

test('another process’s commits survive a side store; a second SQLite library loses them', async (t) => {
  const everyone = [
    'engine-1@example.com',
    'engine-2@example.com',
    'engine-3@example.com',
    'other-1@example.com',
    'other-2@example.com',
  ];
  const viaSideStore = await interleave(path.join(tempDir(t), 'store.db'), (commerce) => {
    const side = commerce.openSideStore();
    side.exec('CREATE TABLE side_notes (id INTEGER PRIMARY KEY, body TEXT)');
    side.prepare('INSERT INTO side_notes (body) VALUES (?)').run('side write');
    side.close();
  });
  assert.deepEqual(viaSideStore.emails, everyone);
  assert.equal(viaSideStore.integrity, 'ok');

  const Database = loadBetterSqlite3();
  if (!Database) {
    t.diagnostic('better-sqlite3 is not installed in the CLI workspace; skipped the contrast');
    return;
  }
  const viaSecondLibrary = await interleave(path.join(tempDir(t), 'store.db'), (_, file) => {
    const foreign = new Database(file);
    foreign.exec('CREATE TABLE side_notes (id INTEGER PRIMARY KEY, body TEXT)');
    foreign.close();
  });
  // The foreign close unlinked the WAL and dropped the engine's locks. The
  // other process then committed into a new WAL and, believing itself the
  // last connection, checkpointed it into the file; the engine kept writing
  // its unlinked WAL and, at close, checkpointed THAT over the file. The other
  // process's committed customers are gone.
  assert.deepEqual(
    viaSecondLibrary.emails,
    everyone.filter((email) => email.startsWith('engine-')),
    JSON.stringify(viaSecondLibrary),
  );
});
