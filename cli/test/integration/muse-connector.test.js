import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { Commerce } from '../../../bindings/node/index.js';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { createMuseConnector, MUSE_DEFAULT_TOOLS } from '../../src/connectors/muse.js';
import { runMuseSmoke } from '../../deploy/muse/smoke.mjs';

const KEY = 'muse-integration-placeholder-key'; // gitleaks:allow
const OTHER_KEY = 'muse-other-integration-placeholder-key'; // gitleaks:allow

for (const scoped of [false, true]) {
  test(`CLI boots ${scoped ? 'account-scoped' : 'shared-key'} discovery and authenticated store reads`, async (t) => {
    const directory = mkdtempSync(join(tmpdir(), 'stateset-muse-cli-'));
    const dbPath = join(directory, 'store.db');
    const keysPath = join(directory, 'keys.txt');
    writeFileSync(keysPath, KEY, { mode: 0o600 });
    const accountsPath = join(directory, 'accounts.json');
    writeFileSync(
      accountsPath,
      JSON.stringify({
        accounts: [{ id: 'catalog-reviewer', apiKeyFile: './keys.txt', tools: ['list_products'] }],
      }),
    );
    const commerce = new Commerce(dbPath);
    await commerce.products.create({ name: 'CLI Muse Coffee' });
    commerce.close();
    const reservation = createServer();
    await new Promise((resolve) => reservation.listen(0, '127.0.0.1', resolve));
    const port = reservation.address().port;
    await new Promise((resolve) => reservation.close(resolve));
    const child = spawn(
      process.execPath,
      [
        new URL('../../bin/stateset-muse.js', import.meta.url).pathname,
        '--db',
        dbPath,
        scoped ? '--accounts-file' : '--api-key-file',
        scoped ? accountsPath : keysPath,
        '--port',
        String(port),
      ],
      {
        cwd: directory,
        stdio: ['ignore', 'pipe', 'pipe'],
        env: {
          ...process.env,
          STATESET_MUSE_API_KEYS: '',
          STATESET_KERNEL_POLICY: '',
          STATESET_KERNEL_PRINCIPAL: '',
          STATESET_KERNEL_STORE_ID: '',
        },
      },
    );
    let stderrOutput = '';
    t.after(async () => {
      if (child.exitCode === null && child.signalCode === null) {
        const exited = new Promise((resolve) => child.once('close', resolve));
        child.kill('SIGTERM');
        await exited;
      }
      rmSync(directory, { recursive: true, force: true });
      assert.ok(!stderrOutput.includes(KEY));
      assert.ok(!stderrOutput.includes('CLI Muse Coffee'));
      const records = stderrOutput
        .split('\n')
        .filter((line) => line.startsWith('{'))
        .map((line) => JSON.parse(line));
      assert.ok(
        records.some(
          (record) =>
            record.type === 'muse_request' &&
            record.accountId === (scoped ? 'catalog-reviewer' : 'muse-store') &&
            record.tool === 'list_products' &&
            record.outcome === 'tool_completed',
        ),
      );
    });
    await new Promise((resolve, reject) => {
      let output = '';
      const timeout = setTimeout(
        () => reject(new Error(`CLI startup timed out: ${output}`)),
        10000,
      );
      const onExit = () => {
        clearTimeout(timeout);
        reject(new Error(`CLI exited: ${output}`));
      };
      child.once('exit', onExit);
      child.stderr.on('data', (chunk) => {
        output += chunk;
        stderrOutput += chunk;
      });
      child.stdout.on('data', (chunk) => {
        output += chunk;
        if (output.includes('connector listening')) {
          clearTimeout(timeout);
          child.removeListener('exit', onExit);
          resolve();
        }
      });
    });
    const base = `http://127.0.0.1:${port}`;
    const schema = await (await fetch(`${base}/openapi.json`)).json();
    assert.equal(Object.keys(schema.paths).length, MUSE_DEFAULT_TOOLS.length);
    assert.equal((await fetch(`${base}/v1/tools`)).status, 401);
    const read = await fetch(`${base}/v1/tools/list_products`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
      body: '{}',
    });
    assert.equal(read.status, 200);
    assert.match(await read.text(), /CLI Muse Coffee/);
    const smoke = await runMuseSmoke({ baseUrl: base, apiKey: KEY });
    assert.equal(smoke.passed, true);
    assert.equal(smoke.previewChecked, !scoped);
    if (scoped) {
      assert.deepEqual(smoke.tools, ['list_products']);
      const denied = await fetch(`${base}/v1/tools/create_product`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: 'Forbidden CLI product' }),
      });
      assert.equal(denied.status, 404);
    }
    assert.equal(smoke.appliedWrites, 0);
    const smokeCli = spawnSync(
      process.execPath,
      [
        new URL('../../deploy/muse/smoke.mjs', import.meta.url).pathname,
        '--base-url',
        base,
        '--api-key-file',
        keysPath,
      ],
      { encoding: 'utf8', timeout: 10000 },
    );
    assert.equal(smokeCli.status, 0, smokeCli.stderr);
    assert.equal(JSON.parse(smokeCli.stdout).passed, true);
    assert.ok(!smokeCli.stdout.includes(KEY));
    assert.ok(!smokeCli.stdout.includes('CLI Muse Coffee'));
  });
}
test('real toolkit catalog, reads and product write previews share one store', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-muse-test-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const tools = ['list_products', 'create_product'];
  const toolkit = createEmbeddedAgentToolkit({
    commerce,
    dbPath,
    capabilities: tools,
    allowApply: false,
    policyStorePath: join(directory, 'policy.json'),
  });
  const connector = createMuseConnector({
    toolkit,
    apiKeys: [KEY],
    publicUrl: 'http://127.0.0.1',
    tools,
  });
  const server = createServer(connector.handler);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => {
    await new Promise((resolve) => server.close(resolve));
    connector.close();
    toolkit.close();
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  await commerce.products.create({ name: 'Muse Coffee' });
  const base = `http://127.0.0.1:${server.address().port}`;
  const headers = { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' };
  const read = await fetch(`${base}/v1/tools/list_products`, {
    method: 'POST',
    headers,
    body: '{}',
  });
  assert.equal(read.status, 200);
  assert.match(await read.text(), /Muse Coffee/);
  const preview = await fetch(`${base}/v1/tools/create_product`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ name: 'Preview Coffee' }),
  });
  assert.equal(preview.status, 200);
  assert.equal((await preview.json()).preview, true);
  assert.equal((await commerce.products.list()).length, 1);
  const unavailable = await fetch(`${base}/v1/tools/list_customers`, {
    method: 'POST',
    headers,
    body: '{}',
  });
  assert.equal(unavailable.status, 404);
});

test('CLI explains setup and refuses startup without credentials or governed apply config', () => {
  const bin = new URL('../../bin/stateset-muse.js', import.meta.url);
  const run = (args, env = {}) =>
    spawnSync(process.execPath, [bin.pathname, ...args], {
      encoding: 'utf8',
      timeout: 10000,
      env: {
        ...process.env,
        STATESET_MUSE_API_KEYS: '',
        STATESET_KERNEL_POLICY: '',
        STATESET_KERNEL_PRINCIPAL: '',
        STATESET_KERNEL_STORE_ID: '',
        ...env,
      },
    });
  const help = run(['--help']);
  assert.equal(help.status, 0);
  assert.match(help.stdout, /preview by default/);
  const noKeys = run([]);
  assert.notEqual(noKeys.status, 0);
  assert.match(noKeys.stderr, /STATESET_MUSE_API_KEYS/);
  const apply = run(['--apply'], { STATESET_MUSE_API_KEYS: KEY });
  assert.notEqual(apply.status, 0);
  assert.match(apply.stderr, /trusted kernel configuration/);
  const mixed = run(['--accounts-file', 'unused.json'], { STATESET_MUSE_API_KEYS: KEY });
  assert.notEqual(mixed.status, 0);
  assert.match(mixed.stderr, /cannot be combined/);
  for (const flag of [
    '--requests-per-minute',
    '--public-requests-per-minute',
    '--max-in-flight',
    '--max-account-in-flight',
    '--max-body-bytes',
  ]) {
    const invalid = run([flag, '0'], { STATESET_MUSE_API_KEYS: KEY });
    assert.notEqual(invalid.status, 0);
    assert.match(invalid.stderr, /positive integer/);
  }
});

test('apply preserves exact money and enforces operator-owned kernel authority', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-muse-apply-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const principal = {
    id: 'agent:muse-test',
    kind: 'agent',
    tenantId: 'tenant:muse-test',
    delegatedBy: 'user:muse-test',
    capabilities: ['products.create'],
  };
  const kernel = {
    strict: true,
    storeId: 'store:muse-test',
    principal,
    policy: {
      version: 'muse-test-1',
      trusted_authority_keys: {},
      commands: {
        'products.create': {
          required_capabilities: ['products.create'],
          requires_tenant: true,
          requires_store: true,
          requires_agent_delegation: true,
          requires_signed_authority: false,
          requires_approval: false,
        },
      },
    },
  };
  const toolkit = createEmbeddedAgentToolkit({
    commerce,
    dbPath,
    kernel,
    capabilities: ['create_product', 'list_products'],
    allowApply: true,
    policyStorePath: join(directory, 'policy.json'),
  });
  const connector = createMuseConnector({
    toolkit,
    accounts: [
      {
        id: 'operator-one',
        apiKeys: [KEY],
        tools: ['create_product', 'list_products'],
        allowApply: true,
      },
      { id: 'operator-two', apiKeys: [OTHER_KEY], tools: ['create_product'], allowApply: true },
    ],
    publicUrl: 'http://127.0.0.1',
    tools: ['create_product', 'list_products'],
    allowApply: true,
  });
  const server = createServer(connector.handler);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => {
    await new Promise((resolve) => server.close(resolve));
    connector.close();
    toolkit.close();
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const url = `http://127.0.0.1:${server.address().port}/v1/tools/create_product`;
  const post = (params, key = 'muse-test-exact-write', apiKey = KEY) =>
    fetch(url, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
        'Idempotency-Key': key,
      },
      body: JSON.stringify(params),
    });
  const response = await post({
    name: 'Exact Muse Offer',
    variants: [{ sku: 'MUSE-EXACT', price: '9007199254740993.25' }],
  });
  assert.equal(response.status, 200);
  const result = await response.json();
  assert.equal(result.success, true, JSON.stringify(result));
  const products = await commerce.products.list();
  assert.equal(products.length, 1);
  const variants = await commerce.products.getVariants(products[0].id);
  assert.equal(variants[0].priceExact, '9007199254740993.25');
  const retry = await post({
    name: 'Exact Muse Offer',
    variants: [{ sku: 'MUSE-EXACT', price: '9007199254740993.25' }],
  });
  assert.equal((await retry.json()).result.receipt.receipt_id, result.result.receipt.receipt_id);
  assert.equal((await commerce.products.list()).length, 1);
  const conflict = await post({ name: 'Conflicting Muse Offer' });
  const conflictResult = await conflict.json();
  assert.equal(conflictResult.result.receipt.error_code, 'kernel.idempotency_conflict');
  assert.equal((await commerce.products.list()).length, 1);
  const smoke = await runMuseSmoke({ baseUrl: new URL(url).origin, apiKey: KEY });
  assert.equal(smoke.previewChecked, false);
  assert.equal(smoke.appliedWrites, 0);
  assert.equal((await commerce.products.list()).length, 1);
  const unsafe = await post({
    name: 'Float Offer',
    variants: [{ sku: 'MUSE-FLOAT', price: 12.34 }],
  });
  assert.equal((await unsafe.json()).success, false);
  const independent = await post(
    { name: 'Second operator offer' },
    'muse-test-exact-write',
    OTHER_KEY,
  );
  const independentResult = await independent.json();
  assert.equal(independentResult.result.success, true, JSON.stringify(independentResult));
  assert.notEqual(independentResult.result.receipt.receipt_id, result.result.receipt.receipt_id);
  assert.equal(
    (await commerce.products.list()).length,
    2,
    'Same caller retry key has independent account scope',
  );
  principal.capabilities = [];
  const denied = await post({ name: 'Denied Muse Offer' }, 'muse-test-denied-write');
  const deniedResult = await denied.json();
  assert.equal(deniedResult.result.success, false, JSON.stringify(deniedResult));
  assert.notEqual(deniedResult.result.receipt.status, 'succeeded');
  assert.equal((await commerce.products.list()).length, 2);
});
