import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import Database from 'better-sqlite3';
import { TerminalBackend } from '../../src/tui/backend.js';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { DOMAINS, TABLES, explorerRequestKey } from '../../src/tui/domains.js';
import { exploreStore } from '../../src/tui/explore-store.js';
const request = (domain, table, extra = {}) => ({
  domain,
  table,
  query: '',
  page: 0,
  sort: 0,
  relation: null,
  ...extra,
});
const digest = async (file) =>
  createHash('sha256')
    .update(await fs.readFile(file))
    .digest('hex');

test(
  'native engine resource sweep, exact finance, linked records and source/WAL immutability',
  { timeout: 240000 },
  async () => {
    const demo = new TerminalBackend();
    let live;
    try {
      console.error('Explorer acceptance: open native demo');
      const demoData = await demo.request('snapshot');
      const source = path.join(await demo.directory, 'demo.db');
      // Match the workbench's ownership boundary: wait for the native writer
      // process to exit before opening a second SQLite implementation's writer.
      // A resolved native close() alone can still leave pool cleanup in flight.
      console.error('Explorer acceptance: create exact finance fixture in an owned process');
      const moduleUrl = new URL('../../src/commerce.js', import.meta.url).href;
      const { stdout } = await promisify(execFile)(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
        import {getCommerceCtor,adaptCommerceApis} from ${JSON.stringify(moduleUrl)};
        const Commerce=getCommerceCtor();
        const commerce=adaptCommerceApis(await Commerce.open(process.argv[1],{maxConnections:1}));
        try {
          const invoice=await commerce.invoices.create({customerId:process.argv[2],items:[{description:'Exact explorer acceptance',quantity:1,unitPriceExact:'9007199254740993.0100'}]});
          const account=await commerce.generalLedger.createAccount({accountNumber:'99991',name:'Explorer acceptance cash',accountType:'asset',currency:'USD'});
          console.log(JSON.stringify({invoice,account}));
        } finally { await commerce.close(); }
      `,
          source,
          demoData.customers[0].id,
        ],
        { timeout: 45000 },
      );
      const { invoice, account } = JSON.parse(stdout);
      const writer = new Database(source);
      try {
        writer.pragma('journal_mode = WAL');
        writer.pragma('wal_autocheckpoint = 0');
        console.error('Explorer acceptance: inspect committed native records');
        assert.equal(
          writer.prepare('SELECT name FROM gl_accounts WHERE id = ?').get(account.id).name,
          'Explorer acceptance cash',
        );
        const stamp = '1900-01-01 00:00:00';
        const insert = writer.prepare(
          'INSERT INTO products (id,name,slug,created_at,updated_at) VALUES (?,?,?,?,?)',
        );
        writer.transaction(() => {
          for (let i = 0; i < 61; i++)
            insert.run(
              `explorer-product-${i}`,
              i === 60 ? 'Needle 100%_\\' : 'Paging fixture ' + i,
              `explorer-fixture-${i}`,
              stamp,
              stamp,
            );
        })();
        const before = await Promise.all([digest(source), digest(source + '-wal')]);
        let available = 0;
        const missing = [];
        for (const table of TABLES) {
          const domain = DOMAINS.find((d) => d.tables.includes(table)).id;
          const result = exploreStore(source, request(domain, table));
          if (result.unavailable) {
            assert.match(
              result.unavailable,
              /does not contain this resource/,
              `${domain}/${table}: unexpected query error`,
            );
            assert.equal(result.matched, null);
            missing.push(table);
          } else {
            available++;
            assert.equal(typeof result.matched, 'number');
            assert.ok(result.rows.length <= 50);
            assert.ok(
              result.rows.every((row) =>
                Object.values(row).every((v) => v === null || typeof v === 'string'),
              ),
            );
          }
        }
        console.error(
          `Explorer native coverage: ${available} available resources; optional/missing: ${missing.join(', ') || 'none'}`,
        );
        assert.ok(available >= 190);
        live = new TerminalBackend(source);
        console.error('Explorer acceptance: browse native finance through WAL snapshot');
        const finance = await live.request('snapshot', {
          explorer: request('invoices', 'invoices', { query: invoice.id }),
        });
        assert.deepEqual(finance.errors, []);
        assert.equal(invoice.totalExact, '9007199254740993.01');
        assert.equal(finance.explorer.rows[0].id, invoice.id);
        assert.equal(finance.explorer.rows[0].total, invoice.totalExact);
        const first = exploreStore(source, request('products', 'products'));
        assert.equal(first.matched, 65);
        assert.equal(first.rows.length, 50);
        const last = exploreStore(source, request('products', 'products', { page: 999 }));
        assert.equal(last.page, 1);
        assert.equal(
          last.requestKey,
          explorerRequestKey({ ...request('products', 'products'), mode: 'records', page: 1 }),
        );
        assert.equal(last.rows.length, 15);
        const needle = exploreStore(source, request('products', 'products', { query: '100%_\\' }));
        assert.equal(needle.matched, 1);
        assert.equal(needle.rows[0].id, 'explorer-product-60');
        const sorted = exploreStore(source, request('products', 'products', { sort: 1 }));
        assert.equal(sorted.rows[0].name, 'Ceremonial matcha');
        const product = first.rows.find((r) => r.name === 'House blend');
        assert.ok(product);
        const index = first.rows.indexOf(product);
        const link = first.links[index].find((l) => l.table === 'product_variants');
        assert.ok(link);
        assert.equal(link.value, product.id);
        const variants = exploreStore(
          source,
          request(link.domain, link.table, {
            relation: { column: link.column, value: link.value },
          }),
        );
        assert.ok(
          variants.rows.length > 0 && variants.rows.every((r) => r.product_id === product.id),
        );
        assert.equal(variants.rows[0].price, '12.50');
        const invalid = exploreStore(
          source,
          request('products', 'products', {
            relation: { column: 'id = 1; DROP TABLE products', value: 'x' },
          }),
        );
        assert.match(invalid.unavailable, /Unsupported record relationship/);
        assert.equal(invalid.matched, null);
        assert.throws(
          () => exploreStore(source, request('products', 'sqlite_schema')),
          /Unsupported engine resource/,
        );
        assert.equal(
          exploreStore(source, request('products', 'products', { query: "' OR 1=1 --" })).matched,
          0,
        );
        assert.deepEqual(await Promise.all([digest(source), digest(source + '-wal')]), before);
      } finally {
        writer.close();
      }
    } finally {
      if (live) await live.close();
      await demo.close();
    }
  },
);

test('storage inspector bounds fields, preserves 64-bit IDs, and omits credential/binary columns from search', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-explorer-fixture-'));
  const filename = path.join(directory, 'fixture.db');
  const writer = new Database(filename);
  try {
    writer.exec(
      'CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price TEXT, api_token TEXT, attachment BLOB, notes TEXT)',
    );
    writer
      .prepare('INSERT INTO products VALUES (?,?,?,?,?,?)')
      .run(
        9223372036854775807n,
        'Exact product',
        '9007199254740993.0100',
        'private-fixture-token',
        Buffer.from('binary fixture'),
        'x'.repeat(5000),
      );
    const result = exploreStore(filename, request('products', 'products'));
    assert.equal(result.rows[0].id, '9223372036854775807');
    assert.equal(result.rows[0].price, '9007199254740993.0100');
    assert.equal(result.rows[0].notes.length, 4096);
    assert.equal(result.rows[0].api_token, undefined);
    assert.equal(result.rows[0].attachment, undefined);
    assert.ok(result.warnings.some((w) => w.includes('Credential and binary')));
    assert.equal(
      exploreStore(filename, request('products', 'products', { query: 'private-fixture-token' }))
        .matched,
      0,
    );
    const unavailable = exploreStore(filename, request('general-ledger', 'gl_accounts'));
    assert.equal(unavailable.matched, null);
    assert.match(unavailable.unavailable, /does not contain/);
    const missingPage = exploreStore(
      filename,
      request('general-ledger', 'gl_accounts', { page: 9 }),
    );
    assert.equal(
      missingPage.requestKey,
      explorerRequestKey({ ...request('general-ledger', 'gl_accounts'), mode: 'records', page: 0 }),
    );
  } finally {
    writer.close();
    await fs.rm(directory, { recursive: true, force: true });
  }
});
