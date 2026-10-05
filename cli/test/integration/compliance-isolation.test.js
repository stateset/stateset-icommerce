import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '../../../bindings/node/index.js';
import { createIsolatedComplianceService } from '../../src/compliance/isolated.js';
import { complianceTools } from '../../src/tools/compliance.js';
import { sqliteGet, sqliteOperations, sqliteExec } from '../helpers/sqlite-process.js';

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-compliance-isolation-'));
  const dbPath = join(directory, 'store.db');
  let commerce = new Commerce(dbPath);
  t.after(async () => {
    await commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  return {
    directory,
    dbPath,
    commerce,
    service: createIsolatedComplianceService(dbPath),
    async reopen() {
      await commerce.close();
      commerce = new Commerce(dbPath);
      return commerce;
    },
  };
}
const customerInput = (email) => ({
  email,
  firstName: 'Ada',
  lastName: 'Private',
  phone: '555-0100',
});
const tool = (name) => complianceTools.find((entry) => entry.name === name);

function seedNotice(dbPath, customerId) {
  sqliteOperations(dbPath, [
    {
      mode: 'exec',
      sql: `INSERT INTO a2a_notification_log
    (id, recipient_address, endpoint_url, event_type, payload, created_at, updated_at)
    VALUES ('notice', '${customerId}', 'https://example.invalid', 'test', '{}', datetime('now'), datetime('now'))`,
    },
  ]);
}

test('isolated exports observe native writes before and after engine reopen', async (t) => {
  const f = fixture(t);
  const original = await f.commerce.customers.create(customerInput('first@example.com'));
  let result = await f.service.generateGDPRExport(original.id);
  assert.equal(result.commerceData.customers[0].email, 'first@example.com');
  const reopened = await f.reopen();
  await reopened.customers.update(original.id, { firstName: 'Updated' });
  result = await f.service.generateGDPRExport(original.id);
  assert.equal(result.commerceData.customers[0].first_name, 'Updated');
  const second = await reopened.customers.create(customerInput('second@example.com'));
  assert.equal((await f.service.generateGDPRExport(second.id)).commerceData.customers.length, 1);
  assert.equal(await reopened.customers.count(), 2);
});

test('isolated erasure rolls back A2A and commerce mutations when a later write fails', async (t) => {
  const f = fixture(t);
  const customer = await f.commerce.customers.create(customerInput('rollback@example.com'));
  await f.service.generateGDPRExport(customer.id);
  seedNotice(f.dbPath, customer.id);
  sqliteExec(
    f.dbPath,
    `CREATE TRIGGER fail_compliance BEFORE UPDATE ON orders
    BEGIN SELECT RAISE(ABORT, 'injected erasure failure'); END;`,
  );
  await f.commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'privacy-1', name: 'Test', quantity: 1, unitPriceExact: '10.00' }],
  });
  await assert.rejects(f.service.deleteGDPRData(customer.id), /injected erasure failure/);
  assert.equal((await f.commerce.customers.get(customer.id)).email, 'rollback@example.com');
  assert.equal(sqliteGet(f.dbPath, 'SELECT COUNT(*) AS n FROM a2a_notification_log').n, 1);
  sqliteExec(f.dbPath, 'DROP TRIGGER fail_compliance');
  await f.service.deleteGDPRData(customer.id, { keepTransactions: true });
  assert.equal(sqliteGet(f.dbPath, 'SELECT COUNT(*) AS n FROM a2a_notification_log').n, 0);
  assert.equal((await f.commerce.customers.get(customer.id)).status, 'deleted');
  assert.equal(await f.commerce.customers.getByEmail('rollback@example.com'), null);
  const erased = sqliteGet(
    f.dbPath,
    'SELECT email_key, version, tags FROM customers WHERE id = ?',
    customer.id,
  );
  assert.equal(erased.email_key, null);
  assert.equal(erased.version, 2);
  assert.equal(erased.tags, '[]');
  const replacement = await f.commerce.customers.create(customerInput('rollback@example.com'));
  assert.notEqual(replacement.id, customer.id);
});

test('file compliance tools preserve store scope and do not initialize a local A2A driver', async (t) => {
  const f = fixture(t);
  const customer = await f.commerce.customers.create(customerInput('scope@example.com'));
  const ctx = {
    a2aStore: {
      dbPath: f.dbPath,
      init() {
        throw new Error('must not open local driver');
      },
    },
  };
  const result = await tool('export_gdpr_data').handler({
    ...ctx,
    params: { customerId: customer.id },
  });
  assert.equal(result.success, true, result.error);
  assert.equal(result.commerceData.customers[0].email, 'scope@example.com');
  const preview = await tool('delete_gdpr_data').handler({
    ...ctx,
    params: { customerId: customer.id },
  });
  assert.equal(preview.success, false);
  assert.match(preview.error, /--apply/);
  assert.equal((await f.commerce.customers.get(customer.id)).email, 'scope@example.com');
  const applied = await tool('delete_gdpr_data').handler({
    ...ctx,
    allowApply: true,
    params: { customerId: customer.id },
  });
  assert.equal(applied.success, true, applied.error);
});

test('missing configured stores fail instead of exporting an empty substitute', async (t) => {
  const f = fixture(t);
  const missing = join(f.directory, 'missing.db');
  await assert.rejects(
    createIsolatedComplianceService(missing).generateGDPRExport('nobody'),
    /ENOENT/,
  );
  await assert.rejects(
    createIsolatedComplianceService(f.dbPath, { commerceDbPath: missing }).deleteGDPRData('nobody'),
    /ENOENT/,
  );
  assert.equal(existsSync(missing), false);
  assert.throws(() => createIsolatedComplianceService(':memory:'), /file path/);
  await assert.rejects(f.service.generateGDPRExport('x'.repeat(65_536)), /64 KiB/);
});
