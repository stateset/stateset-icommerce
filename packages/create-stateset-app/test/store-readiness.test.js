import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { assessStoreConfiguration } from '../templates/storefront/lib/store-readiness.js';
import { inspectStoreDatabase } from '../templates/storefront/lib/store-database-check.js';
import { getCheckoutConfiguration } from '../templates/storefront/lib/checkout-config.js';
import { assertCheckoutAvailable } from '../templates/storefront/lib/payment-readiness.js';

const merchant = '0x1111111111111111111111111111111111111111';
const environment = {
  NEXT_PUBLIC_STORE_WALLET_ADDRESS: merchant,
  STATESET_BASE_RPC_URL: 'https://rpc.example.test/private-provider-key',
  STATESET_USDC_CONFIRMATIONS: '2',
  STATESET_TAX_RATES_JSON: '{"OR":"0","CA":"0.0725"}',
  STATESET_SHIPPING_METHODS_JSON:
    '[{"id":"ground","label":"Ground","amount":"0","countries":["US"]}]',
  NEXT_PUBLIC_BASE_URL: 'https://shop.example.test',
  ANTHROPIC_API_KEY: 'secret-test-chat-key',
};

test('launch checks accept explicit zero tax and shipping without claiming provider verification', () => {
  const report = assessStoreConfiguration(environment, { launch: true });
  assert.equal(report.configurationReady, true);
  assert.equal(report.scope, 'local-configuration');
  assert.equal(
    report.checks.every((check) => check.status === 'pass'),
    true,
  );
  const output = JSON.stringify(report);
  for (const value of ['private-provider-key', 'secret-test-chat-key', merchant]) {
    assert.equal(output.includes(value), false);
  }
  assert.match(output, /not checked|not verified|not connected/);
});

test('demo rates remain usable in development and cannot pass launch checks', () => {
  const demo = { ...environment };
  delete demo.STATESET_TAX_RATES_JSON;
  delete demo.STATESET_SHIPPING_METHODS_JSON;
  assert.equal(assessStoreConfiguration(demo).configurationReady, true);
  const report = assessStoreConfiguration(demo, { launch: true });
  assert.equal(report.configurationReady, false);
  assert.deepEqual(
    report.checks.filter((check) => check.status === 'fail').map((check) => check.id),
    ['tax', 'shipping'],
  );
});

test('optional chat can be absent without blocking checkout', () => {
  const report = assessStoreConfiguration(
    { ...environment, ANTHROPIC_API_KEY: '' },
    { launch: true },
  );
  assert.equal(report.configurationReady, true);
  assert.equal(report.checks.find((check) => check.id === 'chat').status, 'warn');
});

for (const [key, value] of [
  ['NEXT_PUBLIC_STORE_WALLET_ADDRESS', '0x' + '0'.repeat(40)],
  ['NEXT_PUBLIC_STORE_WALLET_ADDRESS', 'secret-invalid-wallet'],
  ['STATESET_BASE_RPC_URL', 'file:///secret-key'],
  ['STATESET_BASE_RPC_URL', 'not-a-url-secret'],
  ['STATESET_USDC_CONFIRMATIONS', '0'],
  ['STATESET_USDC_CONFIRMATIONS', 'NaN'],
  ['STATESET_USDC_CONFIRMATIONS', '1.5'],
  ['STATESET_USDC_CONFIRMATIONS', '9007199254740992'],
  ['STATESET_TAX_RATES_JSON', '{}'],
  ['STATESET_TAX_RATES_JSON', '{"XX":"0.1"}'],
  ['STATESET_TAX_RATES_JSON', '{"CA":0.0725}'],
  ['STATESET_TAX_RATES_JSON', '{"CA":"0.1","ca":"0.2"}'],
  ['STATESET_TAX_RATES_JSON', '{"CA":"1.01"}'],
  ['STATESET_SHIPPING_METHODS_JSON', '[{"id":"ground","label":"Ground","amount":1.25}]'],
  ['STATESET_SHIPPING_METHODS_JSON', '[{"id":"ground","label":"Ground","amount":"1.0000001"}]'],
  ['STATESET_SHIPPING_METHODS_JSON', '[{"id":"ground","label":"Ground","amount":"0.001"}]'],
  [
    'STATESET_SHIPPING_METHODS_JSON',
    '[{"id":"ground","label":"Ground","amount":"0","countries":["CA"]}]',
  ],
  ['NEXT_PUBLIC_BASE_URL', 'http://localhost:3000'],
  ['NEXT_PUBLIC_BASE_URL', 'https://user:secret@shop.example.test'],
]) {
  test(`launch refuses invalid ${key}: case ${value.slice(0, 16)}`, () => {
    const report = assessStoreConfiguration({ ...environment, [key]: value }, { launch: true });
    assert.equal(report.configurationReady, false);
    assert.ok(report.checks.some((check) => check.status === 'fail' && check.action));
    assert.equal(JSON.stringify(report).includes('secret'), false);
  });
}

test('doctor reads existing SQLite headers without creating stores or journal sidecars', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'store-doctor-'));
  try {
    const options = { cwd: directory, launch: true };
    const config = { STATESET_DB_PATH: './store.db' };
    assert.equal(inspectStoreDatabase(config, options).status, 'fail');
    assert.deepEqual(fs.readdirSync(directory), []);
    fs.writeFileSync(path.join(directory, 'store.db'), 'SQLite format 3\0');
    const before = fs.statSync(path.join(directory, 'store.db'));
    assert.equal(inspectStoreDatabase(config, options).status, 'pass');
    assert.equal(fs.statSync(path.join(directory, 'store.db')).mtimeMs, before.mtimeMs);
    assert.deepEqual(fs.readdirSync(directory), ['store.db']);
    assert.equal(inspectStoreDatabase({ STATESET_DB_PATH: ':memory:' }, options).status, 'fail');
    assert.equal(
      inspectStoreDatabase({ STATESET_DB_PATH: ':memory:' }, { ...options, launch: false }).status,
      'warn',
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('public doctor CLI returns machine-readable refusal and success without exposing configuration values', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'store-doctor-cli-'));
  const script = path.resolve(
    import.meta.dirname,
    '../templates/storefront/scripts/check-store.js',
  );
  try {
    fs.writeFileSync(path.join(directory, 'store.db'), 'SQLite format 3\0');
    const run = (overrides = {}) =>
      spawnSync(process.execPath, [script, '--launch', '--json', '--no-env-files'], {
        cwd: directory,
        encoding: 'utf8',
        timeout: 10000,
        env: { ...process.env, ...environment, STATESET_DB_PATH: './store.db', ...overrides },
      });
    const success = run();
    assert.equal(success.status, 0, success.stderr || success.stdout);
    assert.equal(JSON.parse(success.stdout).configurationReady, true);
    const refusal = run({
      STATESET_DB_PATH: './missing.db',
      STATESET_BASE_RPC_URL: 'file:///secret-token',
    });
    assert.equal(refusal.status, 1);
    assert.equal(JSON.parse(refusal.stdout).configurationReady, false);
    for (const output of [success.stdout, refusal.stdout, success.stderr, refusal.stderr]) {
      assert.doesNotMatch(output, /secret-test-chat-key|private-provider-key|secret-token/);
    }
    assert.deepEqual(fs.readdirSync(directory), ['store.db']);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('payment preflight requires server availability and exact chain/merchant agreement', async () => {
  const status = {
    checkoutAvailable: true,
    merchantAddress: merchant.toUpperCase().replace('0X', '0x'),
    chainId: 8453,
  };
  await assertCheckoutAvailable({
    merchantAddress: merchant,
    chainId: 8453,
    fetchImpl: async (url, options) => {
      assert.equal(url, '/api/store/status');
      assert.equal(options.cache, 'no-store');
      return { ok: true, json: async () => status };
    },
  });
  for (const response of [
    { ok: false },
    { ok: true, json: async () => ({ ...status, checkoutAvailable: false }) },
    { ok: true, json: async () => ({ ...status, checkoutAvailable: 'true' }) },
    { ok: true, json: async () => ({ ...status, merchantAddress: '0x' + '2'.repeat(40) }) },
    { ok: true, json: async () => ({ ...status, chainId: 1 }) },
    {
      ok: true,
      json: async () => {
        throw new Error('private API error');
      },
    },
  ]) {
    await assert.rejects(
      assertCheckoutAvailable({
        merchantAddress: merchant,
        chainId: 8453,
        fetchImpl: async () => response,
      }),
      /Checkout is temporarily unavailable/,
    );
  }
  await assert.rejects(
    assertCheckoutAvailable({
      merchantAddress: merchant,
      chainId: 8453,
      fetchImpl: async () => {
        throw new Error('private RPC credentials');
      },
    }),
    /Checkout is temporarily unavailable/,
  );
});

test('payment configuration accepts only safe positive confirmation counts', () => {
  assert.equal(getCheckoutConfiguration(environment).minimumConfirmations, 2);
  assert.equal(
    getCheckoutConfiguration({ ...environment, STATESET_USDC_CONFIRMATIONS: undefined })
      .minimumConfirmations,
    2,
  );
});
