/**
 * Maintenance (backup / restore / export / import) tests for the Node bindings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

test('Maintenance: backup, export and portable domains', async (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-maintenance-'));
  const dbPath = path.join(dir, 'store.db');
  const commerce = new Commerce(dbPath);

  try {
    await t.test('API exists and reports backup support', async () => {
      assert.ok(commerce.maintenance, 'maintenance API should exist');
      assert.equal(await commerce.maintenance.supportsBackup(), true);
      assert.equal(await commerce.maintenance.isSupported(), true);
    });

    await t.test('seed a couple of records', async () => {
      const customer = await commerce.customers.create({
        email: `maint-${Date.now()}@example.com`,
        firstName: 'Maint',
        lastName: 'Tester',
      });
      assert.ok(customer.id);
      const product = await commerce.products.create({
        name: 'Maintenance Widget',
        sku: `MAINT-${Date.now()}`,
        price: '19.99',
      });
      assert.ok(product.id);
    });

    await t.test('backup writes a database plus manifest', async () => {
      const backupPath = path.join(dir, 'backup.db');
      const report = await commerce.maintenance.backup(backupPath);

      assert.equal(report.backupPath, backupPath);
      assert.ok(fs.existsSync(report.backupPath));
      assert.ok(fs.existsSync(report.manifestPath));

      const manifest = report.manifest;
      assert.equal(manifest.manifestVersion, 1);
      assert.ok(manifest.schemaVersion.length > 0);
      assert.ok(manifest.migrationCount > 0);
      assert.ok(manifest.sizeBytes > 0);
      assert.match(manifest.checksum, /^[0-9a-f]{64}$/);
      assert.ok(!Number.isNaN(Date.parse(manifest.createdAt)));

      const onDisk = JSON.parse(fs.readFileSync(report.manifestPath, 'utf8'));
      assert.equal(onDisk.checksum, manifest.checksum);
    });

    await t.test('export writes a parseable JSON envelope', async () => {
      const exportPath = path.join(dir, 'export.json');
      const report = await commerce.maintenance.exportToFile(exportPath, { pretty: true, pageSize: 100 });

      assert.ok(report.total >= 2, `expected records exported, got ${report.total}`);
      assert.ok(Array.isArray(report.counts) && report.counts.length > 0);
      for (const entry of report.counts) {
        assert.equal(typeof entry.domain, 'string');
        assert.equal(typeof entry.count, 'number');
      }

      const envelope = JSON.parse(fs.readFileSync(exportPath, 'utf8'));
      assert.equal(typeof envelope, 'object');
      for (const key of ['format_version', 'engine_version', 'exported_at', 'schema_version', 'domains']) {
        assert.ok(key in envelope, `envelope should have '${key}' (got ${Object.keys(envelope)})`);
      }
    });

    await t.test('listPortableDomains returns non-empty lists', async () => {
      const domains = await commerce.maintenance.listPortableDomains();
      assert.ok(Array.isArray(domains.exportable) && domains.exportable.length > 0);
      assert.ok(Array.isArray(domains.importable) && domains.importable.length > 0);

      assert.deepEqual(await commerce.maintenance.exportableDomains(), domains.exportable);
      assert.deepEqual(await commerce.maintenance.importableDomains(), domains.importable);
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
