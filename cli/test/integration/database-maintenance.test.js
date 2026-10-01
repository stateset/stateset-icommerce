/** Real native-engine recovery tests; auxiliary SQLite runs in a child process. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { createRequire } from 'node:module';
import { DatabaseManager } from '../../src/database.js';
import { sqliteGet } from '../helpers/sqlite-process.js';

const { Commerce } = createRequire(import.meta.url)('@stateset/embedded');

function fixture(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-manager-recovery-'));
  const source = path.join(dir, 'store.db');
  const backups = path.join(dir, 'backups');
  const manager = new DatabaseManager({ defaultPath: source });
  const handles = new Set();
  const open = (file) => {
    const commerce = new Commerce(file);
    handles.add(commerce);
    return commerce;
  };
  t.after(async () => {
    for (const { commerce } of manager.connections.values()) handles.add(commerce);
    for (const commerce of handles) await commerce.close();
    fs.rmSync(dir, { recursive: true, force: true });
  });
  return { dir, source, backups, manager, open };
}

function customer(commerce, email) {
  return commerce.customers.create({ email, firstName: 'Recovery', lastName: 'Test' });
}

function assertNoStaging(dir) {
  assert.deepEqual(
    fs.readdirSync(dir).filter((name) => name.startsWith('.stateset-restore-')),
    [],
  );
}

test('manager snapshots committed WAL data while leaving the source usable', async (t) => {
  const { manager, source, backups, dir, open } = fixture(t);
  const commerce = manager.getConnection();
  const first = await customer(commerce, 'before@example.com');
  assert.ok(fs.existsSync(`${source}-wal`));
  const backup = await manager.backup(undefined, backups);
  assert.equal(backup.source, source);
  assert.equal(backup.size, fs.statSync(backup.backup).size);
  assert.match(backup.manifest.checksum, /^[a-f0-9]{64}$/);
  assert.ok(fs.existsSync(backup.manifestPath));
  assert.equal(fs.existsSync(`${backup.backup}-wal`), false);
  assert.equal(fs.existsSync(`${backup.backup}-shm`), false);
  await customer(commerce, 'after@example.com');
  assert.equal(await commerce.customers.count(), 2);
  assert.equal(manager.current(), commerce);
  const target = path.join(dir, 'restored.db');
  const restored = await manager.restore(backup.backup, target);
  assert.equal(restored.restored, target);
  assert.equal(restored.checksumVerified, true);
  assert.equal(restored.size, backup.size);
  assert.equal(sqliteGet(target, 'PRAGMA integrity_check').integrity_check, 'ok');
  const recovered = open(target);
  assert.equal(await recovered.customers.count(), 1);
  assert.equal((await recovered.customers.get(first.id)).email, 'before@example.com');
  assertNoStaging(dir);
});

test('manager backs up the default file without changing its connection cache', async (t) => {
  const { manager, source, backups, open } = fixture(t);
  const commerce = open(source);
  await customer(commerce, 'default@example.com');
  await commerce.close();
  const backup = await manager.backup(undefined, backups);
  assert.equal(backup.source, source);
  assert.equal(manager.connections.size, 0);
  assert.equal(manager.activeConnection, null);
  assert.equal(await open(backup.backup).customers.count(), 1);
});

test('manager backups use unique filenames and support an existing memory store', async (t) => {
  const { manager, backups, open } = fixture(t);
  const commerce = manager.getConnection(':memory:');
  await customer(commerce, 'memory@example.com');
  const results = await Promise.all([
    manager.backup(':memory:', backups),
    manager.backup(':memory:', backups),
  ]);
  assert.notEqual(results[0].backup, results[1].backup);
  assert.equal(manager.listBackups(backups).length, 2);
  for (const result of results) assert.equal(await open(result.backup).customers.count(), 1);
});

test('missing backup sources fail without creating databases', async (t) => {
  const { manager, source, backups } = fixture(t);
  await assert.rejects(manager.backup(undefined, backups), /does not exist/);
  await assert.rejects(manager.backup(':memory:', backups), /does not exist/);
  await assert.rejects(manager.restore(source), /Backup does not exist/);
  assert.equal(fs.existsSync(source), false);
});

test('restore refuses managed and existing paths without changing their data', async (t) => {
  const { manager, source, backups, dir } = fixture(t);
  const commerce = manager.getConnection();
  await customer(commerce, 'live@example.com');
  const backup = await manager.backup(source, backups);
  await assert.rejects(manager.restore(backup.backup, source), /open connection/);
  assert.equal(await commerce.customers.count(), 1);
  for (const suffix of ['', '-wal', '-shm', '-journal']) {
    const target = path.join(dir, `occupied${suffix || '-main'}.db`);
    fs.writeFileSync(`${target}${suffix}`, suffix ? 'preserve me' : '');
    await assert.rejects(manager.restore(backup.backup, target), /new database path/);
    assert.equal(fs.readFileSync(`${target}${suffix}`, 'utf8'), suffix ? 'preserve me' : '');
    if (suffix) assert.equal(fs.existsSync(target), false);
  }
  const symlink = path.join(dir, 'dangling.db');
  fs.symlinkSync(path.join(dir, 'missing.db'), symlink);
  await assert.rejects(manager.restore(backup.backup, symlink), /new database path/);
  assert.equal(fs.lstatSync(symlink).isSymbolicLink(), true);
  await assert.rejects(manager.restore(backup.backup, ':memory:'), /new file path/);
  assertNoStaging(dir);
});

test('restore fails closed on corrupt, missing and newer manifests', async (t) => {
  const { manager, source, backups, dir } = fixture(t);
  await customer(manager.getConnection(), 'checksum@example.com');
  const backup = await manager.backup(source, backups);
  const bytes = fs.readFileSync(backup.backup);
  const manifest = fs.readFileSync(backup.manifestPath, 'utf8');
  const target = path.join(dir, 'recovery.db');
  fs.appendFileSync(backup.backup, 'corruption');
  await assert.rejects(manager.restore(backup.backup, target), /checksum/i);
  assert.equal(fs.existsSync(target), false);
  fs.writeFileSync(backup.backup, bytes);
  fs.unlinkSync(backup.manifestPath);
  await assert.rejects(manager.restore(backup.backup, target));
  assert.equal(fs.existsSync(target), false);
  const newer = JSON.parse(manifest);
  newer.schema_version = '999_future_schema';
  fs.writeFileSync(backup.manifestPath, JSON.stringify(newer));
  await assert.rejects(manager.restore(backup.backup, target), /newer/i);
  assert.equal(fs.existsSync(target), false);
  assertNoStaging(dir);
});

test('competing restores publish exactly one complete snapshot without overwriting', async (t) => {
  const { manager, source, backups, dir, open } = fixture(t);
  const commerce = manager.getConnection();
  await customer(commerce, 'one@example.com');
  const one = await manager.backup(source, backups);
  await customer(commerce, 'two@example.com');
  const two = await manager.backup(source, backups);
  const target = path.join(dir, 'winner.db');
  const outcomes = await Promise.allSettled([
    manager.restore(one.backup, target),
    manager.restore(two.backup, target),
  ]);
  assert.equal(outcomes.filter((result) => result.status === 'fulfilled').length, 1);
  assert.equal(outcomes.filter((result) => result.status === 'rejected').length, 1);
  const winner = outcomes.findIndex((result) => result.status === 'fulfilled');
  assert.equal(await open(target).customers.count(), winner + 1);
  assertNoStaging(dir);
});
