/**
 * Tests for the maintenance tool module (backup, restore, export, import).
 *
 * Exercises the tools against a mocked commerce object: registration wiring,
 * the --apply gate on every write, argument pass-through, and the graceful
 * degradation path when the running binding has no maintenance accessor.
 */

import { test, describe } from 'node:test';
import assert from 'node:assert/strict';

import { maintenanceTools } from '../../src/tools/maintenance.js';
import { DOMAIN_TOOL_ARRAYS } from '../../src/tools/domain-registry.js';

const toolByName = (name) => {
  const tool = maintenanceTools.find((candidate) => candidate.name === name);
  assert.ok(tool, `tool ${name} is defined`);
  return tool;
};

/** Records every call so assertions can inspect the arguments passed through. */
function makeCommerce(overrides = {}) {
  const calls = [];
  const record =
    (method, result) =>
    (...args) => {
      calls.push({ method, args });
      return result;
    };

  return {
    calls,
    commerce: {
      maintenance: {
        backup: record('backup', {
          backupPath: '/backups/store.db',
          manifestPath: '/backups/store.db.manifest.json',
          manifest: {
            schemaVersion: '066_search_configs',
            sizeBytes: 4096,
            checksum: 'abc123',
          },
        }),
        restore: record('restore', {
          targetPath: '/restored.db',
          schemaVersion: '066_search_configs',
          sizeBytes: 4096,
          checksumVerified: true,
          replacedExisting: false,
        }),
        export: record('export', {
          total: 7,
          counts: [
            ['customers', 4],
            ['orders', 3],
          ],
        }),
        import: record('import', {
          totalCreated: 5,
          created: [['customers', 5]],
          skipped: [['customers', 2]],
          unsupportedDomains: ['invoices'],
        }),
        exportableDomains: record('exportableDomains', ['customers', 'orders', 'invoices']),
        importableDomains: record('importableDomains', ['customers', 'orders']),
        ...overrides,
      },
    },
  };
}

describe('maintenance tools registration', () => {
  test('is wired into the domain registry', () => {
    assert.equal(DOMAIN_TOOL_ARRAYS.maintenance, maintenanceTools);
  });

  test('exposes the expected tools with sane metadata', () => {
    const names = maintenanceTools.map((tool) => tool.name).sort();
    assert.deepEqual(names, [
      'backup_database',
      'export_full_data',
      'import_full_data',
      'list_portable_domains',
      'restore_database',
    ]);

    for (const tool of maintenanceTools) {
      assert.equal(tool.policyDomain, 'maintenance', `${tool.name} has a policy domain`);
      assert.ok(tool.description.length > 20, `${tool.name} has a real description`);
      assert.ok(['read', 'write'].includes(tool.permission), `${tool.name} declares a permission`);
      assert.equal(typeof tool.handler, 'function');
    }
  });

  test('mutating tools are declared as writes', () => {
    for (const name of ['backup_database', 'restore_database', 'import_full_data']) {
      assert.equal(toolByName(name).permission, 'write', `${name} must be a write`);
    }
    for (const name of ['export_full_data', 'list_portable_domains']) {
      assert.equal(toolByName(name).permission, 'read', `${name} must be a read`);
    }
  });
});

describe('apply gate', () => {
  for (const [name, params] of [
    ['backup_database', { backupPath: '/backups/store.db' }],
    ['restore_database', { backupPath: '/b.db', targetPath: '/t.db' }],
    ['import_full_data', { importPath: '/export.json' }],
  ]) {
    test(`${name} refuses to run without --apply`, async () => {
      const { commerce, calls } = makeCommerce();
      const result = await toolByName(name).handler({ commerce, params, allowApply: false });
      assert.equal(result.success, false);
      assert.match(result.error, /requires --apply/);
      assert.deepEqual(result.wouldDo, params);
      assert.equal(calls.length, 0, 'nothing should be executed without --apply');
    });
  }

  test('export_data is a read and needs no --apply', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('export_full_data').handler({
      commerce,
      params: { exportPath: '/export.json' },
      allowApply: false,
    });
    assert.equal(result.success, true);
    assert.equal(calls[0].method, 'export');
  });
});

describe('backup_database', () => {
  test('surfaces the manifest details', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('backup_database').handler({
      commerce,
      params: { backupPath: '/backups/store.db' },
      allowApply: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.checksum, 'abc123');
    assert.equal(result.schemaVersion, '066_search_configs');
    assert.equal(result.sizeBytes, 4096);
    assert.equal(result.manifestPath, '/backups/store.db.manifest.json');
    assert.deepEqual(calls[0], { method: 'backup', args: ['/backups/store.db'] });
  });
});

describe('restore_database', () => {
  test('defaults overwrite to false', async () => {
    const { commerce, calls } = makeCommerce();
    await toolByName('restore_database').handler({
      commerce,
      params: { backupPath: '/b.db', targetPath: '/t.db' },
      allowApply: true,
    });
    assert.deepEqual(calls[0].args, ['/b.db', '/t.db', { overwrite: false }]);
  });

  test('passes overwrite through when explicitly requested', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('restore_database').handler({
      commerce,
      params: { backupPath: '/b.db', targetPath: '/t.db', overwrite: true },
      allowApply: true,
    });
    assert.deepEqual(calls[0].args[2], { overwrite: true });
    assert.equal(result.checksumVerified, true);
    assert.equal(result.replacedExisting, false);
  });

  test('propagates engine refusals rather than swallowing them', async () => {
    const { commerce } = makeCommerce({
      restore: () => {
        throw new Error('refusing to overwrite existing database at /t.db');
      },
    });
    await assert.rejects(
      () =>
        toolByName('restore_database').handler({
          commerce,
          params: { backupPath: '/b.db', targetPath: '/t.db' },
          allowApply: true,
        }),
      /refusing to overwrite/,
    );
  });
});

describe('export_data and import_data', () => {
  test('export reports per-domain counts', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('export_full_data').handler({
      commerce,
      params: { exportPath: '/export.json', domains: ['customers'] },
      allowApply: true,
    });
    assert.equal(result.total, 7);
    assert.deepEqual(result.counts, [
      ['customers', 4],
      ['orders', 3],
    ]);
    assert.deepEqual(calls[0].args, ['/export.json', { domains: ['customers'] }]);
  });

  test('export defaults to all domains', async () => {
    const { commerce, calls } = makeCommerce();
    await toolByName('export_full_data').handler({
      commerce,
      params: { exportPath: '/export.json' },
      allowApply: true,
    });
    assert.deepEqual(calls[0].args[1], { domains: [] });
  });

  test('import defaults to skip-on-conflict and reports unsupported domains', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('import_full_data').handler({
      commerce,
      params: { importPath: '/export.json' },
      allowApply: true,
    });
    assert.deepEqual(calls[0].args[1], {
      domains: [],
      onConflict: 'skip',
      dryRun: false,
    });
    assert.equal(result.totalCreated, 5);
    assert.deepEqual(result.unsupportedDomains, ['invoices']);
  });

  test('import honours fail-on-conflict and dry-run', async () => {
    const { commerce, calls } = makeCommerce();
    const result = await toolByName('import_full_data').handler({
      commerce,
      params: { importPath: '/export.json', onConflict: 'fail', dryRun: true },
      allowApply: true,
    });
    assert.equal(calls[0].args[1].onConflict, 'fail');
    assert.equal(calls[0].args[1].dryRun, true);
    assert.match(result.message, /Dry run/);
  });
});

describe('list_portable_domains', () => {
  test('separates export-only domains from importable ones', async () => {
    const { commerce } = makeCommerce();
    const result = await toolByName('list_portable_domains').handler({ commerce, params: {} });
    assert.deepEqual(result.exportable, ['customers', 'orders', 'invoices']);
    assert.deepEqual(result.importable, ['customers', 'orders']);
    assert.deepEqual(result.exportOnly, ['invoices']);
  });
});

describe('graceful degradation', () => {
  test('every tool reports a clear error when the binding lacks maintenance', async () => {
    const commerce = {};
    const cases = [
      ['backup_database', { backupPath: '/b.db' }],
      ['restore_database', { backupPath: '/b.db', targetPath: '/t.db' }],
      ['export_full_data', { exportPath: '/e.json' }],
      ['import_full_data', { importPath: '/e.json' }],
      ['list_portable_domains', {}],
    ];
    for (const [name, params] of cases) {
      const result = await toolByName(name).handler({ commerce, params, allowApply: true });
      assert.equal(result.success, false, `${name} should fail cleanly`);
      assert.match(result.error, /not available in this build/);
      assert.match(result.hint, /@stateset\/embedded/);
    }
  });
});
