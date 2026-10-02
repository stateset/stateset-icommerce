import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer, request } from 'node:http';
import { z } from 'zod';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createMuseConnector } from '../../src/connectors/muse.js';
import { loadMuseAccounts } from '../../src/connectors/muse-accounts.js';
import { createMuseRequestControls } from '../../src/connectors/muse-request-controls.js';

const KEY = 'muse-placeholder-key-not-a-secret'; // gitleaks:allow
const OTHER_KEY = 'muse-other-placeholder-key-not-a-secret'; // gitleaks:allow
const ROTATED_KEY = 'muse-rotated-placeholder-key-not-a-secret'; // gitleaks:allow
const definitions = [
  {
    name: 'read_product',
    description: 'Read product',
    permission: 'read',
    inputSchema: { type: 'object', properties: { id: { type: 'string' } }, required: ['id'] },
  },
  {
    name: 'write_product',
    description: 'Write product',
    permission: 'write',
    inputSchema: {
      type: 'object',
      properties: { amount: { type: 'string' } },
      required: ['amount'],
    },
  },
];
const schemas = {
  read_product: { id: z.string().min(1) },
  write_product: { amount: z.string().regex(/^\d+\.\d{2}$/) },
};
function stub(calls) {
  return {
    getTools: () => definitions,
    getRawTool: (name) => ({ inputSchema: schemas[name] }),
    executeTool: async (...args) => {
      calls.push(args);
      return { amount: '19.99' };
    },
  };
}
async function serve(t, options = {}) {
  const calls = [];
  const connector = createMuseConnector({
    toolkit: stub(calls),
    apiKeys: [KEY],
    publicUrl: 'http://127.0.0.1',
    tools: definitions.map(({ name }) => name),
    ...options,
  });
  const server = createServer(connector.handler);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => {
    await new Promise((resolve) => server.close(resolve));
    connector.close();
  });
  const base = `http://127.0.0.1:${server.address().port}`;
  const post = (name, params, headers = {}) =>
    fetch(`${base}/v1/tools/${name}`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json', ...headers },
      body: typeof params === 'string' ? params : JSON.stringify(params),
    });
  return { ...connector, calls, base, post, server };
}

test('OpenAPI exactly describes the allowlist and protected request schemas', async (t) => {
  const { base, calls } = await serve(t);
  const schema = await (await fetch(`${base}/openapi.json`)).json();
  assert.equal(schema.openapi, '3.1.0');
  assert.deepEqual(Object.keys(schema.paths), [
    '/v1/tools/read_product',
    '/v1/tools/write_product',
  ]);
  assert.equal(schema.paths['/v1/tools/write_product'].post['x-stateset-preview-only'], true);
  assert.equal(
    schema.paths['/v1/tools/read_product'].post.requestBody.content['application/json'].schema
      .additionalProperties,
    false,
  );
  assert.deepEqual(schema.security, [{ bearerAuth: [] }]);
  assert.equal(
    schema.paths['/v1/tools/read_product'].post.responses[429].headers['Retry-After'].schema
      .minimum,
    1,
  );
  assert.equal(
    schema.paths['/v1/tools/read_product'].post.responses[503].headers['Retry-After'].schema
      .minimum,
    1,
  );
  assert.equal(calls.length, 0);
  const unauthorized = await fetch(`${base}/v1/tools`);
  assert.equal(unauthorized.status, 401);
  assert.equal(unauthorized.headers.get('www-authenticate'), 'Bearer');
});

test('authentication, host, origin and tool scope fail closed', async (t) => {
  const { post, base, calls } = await serve(t);
  assert.equal(
    (await post('read_product', { id: 'x' }, { Authorization: 'Bearer invalid' })).status,
    401,
  );
  const badHost = await new Promise((resolve, reject) => {
    const req = request(`${base}/health`, { headers: { Host: 'evil.example' } }, (res) => {
      res.resume();
      resolve(res.statusCode);
    });
    req.on('error', reject);
    req.end();
  });
  assert.equal(badHost, 403);
  assert.equal(
    (await post('read_product', { id: 'x' }, { Origin: 'https://evil.example' })).status,
    403,
  );
  assert.equal((await post('unlisted', {})).status, 404);
  assert.equal(
    (await fetch(`${base}/v1/tools/read_product`, { headers: { Authorization: `Bearer ${KEY}` } }))
      .status,
    405,
  );
  assert.equal(calls.length, 0);
});

test('read results preserve decimals, writes preview, and model identity options are rejected', async (t) => {
  const { post, calls } = await serve(t);
  const read = await post('read_product', { id: 'x' });
  assert.equal(read.status, 200);
  assert.deepEqual(await read.json(), { amount: '19.99' });
  assert.deepEqual(calls, [['read_product', { id: 'x' }]]);
  const preview = await post('write_product', { amount: '19.99' });
  assert.equal((await preview.json()).preview, true);
  assert.equal(calls.length, 1);
  for (const extra of ['allowApply', 'principal', 'kernel', 'dbPath', 'executionOptions']) {
    assert.equal((await post('read_product', { id: 'x', [extra]: true })).status, 400);
  }
  assert.equal(calls.length, 1);
});

test('bad JSON, invalid schema, floats, media types, and large bodies never execute', async (t) => {
  const { post, calls } = await serve(t, { maxBodyBytes: 64 });
  for (const body of ['{bad', 'null', '[]', '{}']) {
    assert.equal((await post('read_product', body)).status, 400);
  }
  assert.equal((await post('write_product', { amount: 19.99 })).status, 400);
  assert.equal(
    (await post('read_product', { id: 'x' }, { 'Content-Type': 'text/plain' })).status,
    415,
  );
  assert.equal((await post('read_product', { id: 'x'.repeat(100) })).status, 413);
  assert.equal(calls.length, 0);
});

test('trusted apply option delegates and internal exceptions are redacted', async (t) => {
  const { post, calls, openapi } = await serve(t, { allowApply: true });
  assert.equal(openapi.paths['/v1/tools/write_product'].post.parameters[0].name, 'Idempotency-Key');
  assert.equal(openapi.paths['/v1/tools/write_product'].post.parameters[0].required, true);
  assert.equal((await post('write_product', { amount: '19.99' })).status, 400);
  for (const key of ['short', 'has spaces', 'x'.repeat(129)]) {
    assert.equal(
      (await post('write_product', { amount: '19.99' }, { 'Idempotency-Key': key })).status,
      400,
    );
  }
  assert.equal(calls.length, 0);
  assert.equal(
    (await post('write_product', { amount: '19.99' }, { 'Idempotency-Key': 'muse-write-1' }))
      .status,
    200,
  );
  assert.deepEqual(calls, [
    ['write_product', { amount: '19.99' }, { idempotencyKey: 'muse-write-1' }],
  ]);
  const broken = stub([]);
  broken.executeTool = () => {
    throw new Error('/private/store.db token=secret');
  };
  const other = await serve(t, { toolkit: broken });
  const response = await other.post('read_product', { id: 'x' });
  assert.equal(response.status, 500);
  assert.deepEqual(await response.json(), { error: 'Commerce tool execution failed' });
});

test('account scopes govern discovery, execution, previews, and isolated retries', async (t) => {
  const accounts = [
    { id: 'reviewer', apiKeys: [KEY], tools: ['read_product', 'write_product'] },
    {
      id: 'operator',
      apiKeys: [OTHER_KEY, ROTATED_KEY],
      tools: ['write_product'],
      allowApply: true,
    },
  ];
  const { base, post, calls } = await serve(t, { apiKeys: undefined, accounts, allowApply: true });
  const catalog = await (
    await fetch(`${base}/v1/tools`, { headers: { Authorization: `Bearer ${KEY}` } })
  ).json();
  assert.equal(catalog.tools.find((tool) => tool.name === 'write_product').previewOnly, true);
  const schema = await (
    await fetch(`${base}/openapi.json`, { headers: { Authorization: `Bearer ${OTHER_KEY}` } })
  ).json();
  assert.deepEqual(Object.keys(schema.paths), ['/v1/tools/write_product']);
  assert.equal(schema.paths['/v1/tools/write_product'].post['x-stateset-preview-only'], false);
  assert.equal(
    (await fetch(`${base}/openapi.json`, { headers: { Authorization: 'Bearer invalid' } })).status,
    401,
  );
  assert.equal(
    (await post('read_product', { id: 'private' }, { Authorization: `Bearer ${OTHER_KEY}` }))
      .status,
    404,
  );
  assert.equal((await (await post('write_product', { amount: '19.99' })).json()).preview, true);
  assert.equal(calls.length, 0);
  for (const key of [OTHER_KEY, ROTATED_KEY]) {
    assert.equal(
      (
        await post(
          'write_product',
          { amount: '19.99' },
          { Authorization: `Bearer ${key}`, 'Idempotency-Key': 'same-retry-key' },
        )
      ).status,
      200,
    );
  }
  assert.equal(
    calls[0][2].idempotencyKey,
    calls[1][2].idempotencyKey,
    'Rotation retains retry scope',
  );
  assert.match(calls[0][2].idempotencyKey, /^muse:[a-f0-9]{64}$/);
  const previewService = await serve(t, { apiKeys: undefined, accounts, allowApply: false });
  assert.equal(
    (
      await (
        await previewService.post(
          'write_product',
          { amount: '19.99' },
          { Authorization: `Bearer ${OTHER_KEY}` },
        )
      ).json()
    ).preview,
    true,
  );
  assert.equal(previewService.calls.length, 0, 'Account cannot enable service writes');
});

test('account configuration rejects ambiguous credentials and expanded permissions', () => {
  const base = { toolkit: stub([]), publicUrl: 'http://127.0.0.1', tools: ['read_product'] };
  const account = { id: 'reader', apiKeys: [KEY], tools: ['read_product'] };
  for (const accounts of [
    [],
    [null],
    [{ ...account, tools: [] }],
    [{ ...account, tools: ['write_product'] }],
    [{ ...account, allowApply: 'true' }],
    [account, account],
    [account, { ...account, id: 'other' }],
    [{ ...account, apiKeys: [KEY, KEY] }],
  ]) {
    assert.throws(() => createMuseConnector({ ...base, accounts }));
  }
  assert.throws(
    () => createMuseConnector({ ...base, apiKeys: [KEY], accounts: [account] }),
    /not both/,
  );
});

test('operator account files resolve relative key files and reject unknown fields without leaking secrets', (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'muse-accounts-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const path = join(directory, 'accounts.json');
  writeFileSync(join(directory, 'reader.txt'), `${KEY}\n${ROTATED_KEY}\n`, { mode: 0o600 });
  const account = { id: 'reader', apiKeyFile: './reader.txt', tools: ['read_product'] };
  writeFileSync(path, JSON.stringify({ accounts: [account] }));
  assert.deepEqual(loadMuseAccounts(path), [
    { id: 'reader', apiKeys: [KEY, ROTATED_KEY], tools: ['read_product'], allowApply: undefined },
  ]);
  for (const config of [
    { accounts: [account], extra: true },
    { accounts: [{ ...account, apiKeys: [KEY] }] },
    { accounts: [{ ...account, apiKeyFile: './missing.txt' }] },
    { accounts: [] },
  ]) {
    writeFileSync(path, JSON.stringify(config));
    assert.throws(
      () => loadMuseAccounts(path),
      (error) => !error.message.includes(KEY),
    );
  }
  writeFileSync(path, `{invalid secret=${KEY}`);
  assert.throws(() => loadMuseAccounts(path), /configuration JSON/);
});

test('admission windows expire and concurrency permits release once', (t) => {
  let now = 1000;
  t.mock.method(Date, 'now', () => now);
  const controls = createMuseRequestControls({
    accountIds: ['reader'],
    requestsPerMinute: 1,
    publicRequestsPerMinute: 1,
    maxInFlight: 1,
    maxAccountInFlight: 1,
    windowMs: 1000,
  });
  t.after(() => controls.close());
  assert.equal(controls.checkRate('reader').allowed, true);
  assert.equal(controls.checkRate('reader').retryAfterMs, 1000);
  assert.equal(controls.checkRate().allowed, true, 'Public budget is separate');
  now += 1000;
  assert.equal(controls.checkRate('reader').allowed, true);
  const release = controls.acquire('reader');
  assert.equal(controls.acquire('reader'), null);
  release();
  release();
  const next = controls.acquire('reader');
  assert.equal(typeof next, 'function');
  next();
  assert.throws(() => controls.checkRate('unknown'), /Unknown account/);
  assert.throws(() => controls.acquire('unknown'), /Unknown account/);
  for (const option of [
    'requestsPerMinute',
    'publicRequestsPerMinute',
    'maxInFlight',
    'maxAccountInFlight',
    'windowMs',
  ]) {
    for (const value of [0, -1, 1.5, NaN, '2'])
      assert.throws(
        () => createMuseRequestControls({ accountIds: ['reader'], [option]: value }),
        /positive integer/,
      );
  }
});

test('HTTP budgets are shared across rotated keys and public clients, while accounts and health are isolated', async (t) => {
  const accounts = [
    { id: 'reader', apiKeys: [KEY, OTHER_KEY], tools: ['read_product'] },
    { id: 'other', apiKeys: [ROTATED_KEY], tools: ['read_product'] },
  ];
  const { base, post, calls } = await serve(t, {
    apiKeys: undefined,
    accounts,
    requestsPerMinute: 1,
    publicRequestsPerMinute: 1,
  });
  assert.equal((await post('read_product', { id: 'first' })).status, 200);
  const limited = await post(
    'read_product',
    { id: 'second' },
    { Authorization: `Bearer ${OTHER_KEY}` },
  );
  assert.equal(limited.status, 429);
  assert.ok(Number(limited.headers.get('retry-after')) > 0);
  assert.equal(
    (await post('read_product', { id: 'third' }, { Authorization: `Bearer ${ROTATED_KEY}` }))
      .status,
    200,
  );
  assert.equal((await fetch(`${base}/v1/tools`)).status, 401);
  assert.equal(
    (await fetch(`${base}/v1/tools`, { headers: { 'X-Forwarded-For': '198.51.100.1' } })).status,
    429,
  );
  assert.equal((await fetch(`${base}/openapi.json`)).status, 429);
  assert.equal((await fetch(`${base}/health`)).status, 200);
  assert.equal(calls.length, 2);
});

test('per-account and service concurrency reject overload and recover after errors', async (t) => {
  const waiting = new Map();
  const entered = new Map();
  const started = (id) => new Promise((resolve) => entered.set(id, resolve));
  t.after(() => {
    for (const { resolve } of waiting.values()) resolve({});
  });
  const toolkit = stub([]);
  toolkit.executeTool = async (name, { id }) => {
    if (!id.startsWith('slow')) return {};
    return new Promise((resolve, reject) => {
      waiting.set(id, { resolve, reject });
      entered.get(id)();
    });
  };
  const accounts = [
    { id: 'a', apiKeys: [KEY], tools: ['read_product'] },
    { id: 'b', apiKeys: [OTHER_KEY], tools: ['read_product'] },
    { id: 'c', apiKeys: [ROTATED_KEY], tools: ['read_product'] },
  ];
  const { post, base } = await serve(t, {
    apiKeys: undefined,
    accounts,
    toolkit,
    maxInFlight: 2,
    maxAccountInFlight: 1,
  });
  const enterA = started('slow-a');
  const requestA = post('read_product', { id: 'slow-a' });
  await enterA;
  const overloadA = await post('read_product', { id: 'another-a' });
  assert.equal(overloadA.status, 503);
  assert.equal(overloadA.headers.get('retry-after'), '1');
  const enterB = started('slow-b');
  const requestB = post('read_product', { id: 'slow-b' }, { Authorization: `Bearer ${OTHER_KEY}` });
  await enterB;
  assert.equal(
    (await post('read_product', { id: 'blocked-c' }, { Authorization: `Bearer ${ROTATED_KEY}` }))
      .status,
    503,
  );
  assert.equal(
    (await fetch(`${base}/v1/tools`, { headers: { Authorization: `Bearer ${KEY}` } })).status,
    200,
    'Discovery remains available during tool overload',
  );
  waiting.get('slow-b').resolve({});
  assert.equal((await requestB).status, 200);
  assert.equal(
    (await post('read_product', { id: 'recovered-c' }, { Authorization: `Bearer ${ROTATED_KEY}` }))
      .status,
    200,
  );
  waiting.get('slow-a').reject(new Error('private execution error'));
  assert.equal((await requestA).status, 500);
  assert.equal((await post('read_product', { id: 'recovered-a' })).status, 200);
});

test('a disconnected caller retains its permit until the commerce operation settles', async (t) => {
  let resolveExecution;
  let signalStart;
  let signalFinish;
  const started = new Promise((resolve) => {
    signalStart = resolve;
  });
  const finished = new Promise((resolve) => {
    signalFinish = resolve;
  });
  t.after(() => resolveExecution?.({}));
  const toolkit = stub([]);
  toolkit.executeTool = async () =>
    new Promise((resolve) => {
      resolveExecution = resolve;
      signalStart();
    });
  const { base, post, server } = await serve(t, {
    toolkit,
    maxInFlight: 1,
    maxAccountInFlight: 1,
    onAudit: (event) => {
      if (event.outcome === 'tool_completed') signalFinish(event);
    },
  });
  const disconnected = new Promise((resolve) =>
    server.once('request', (req, res) => res.once('close', resolve)),
  );
  const pending = request(`${base}/v1/tools/read_product`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
  });
  pending.on('error', () => {});
  pending.end(JSON.stringify({ id: 'pending' }));
  await started;
  pending.destroy();
  await disconnected;
  assert.equal((await post('read_product', { id: 'blocked' })).status, 503);
  resolveExecution({});
  const event = await finished;
  assert.equal(event.outcome, 'tool_completed');
  assert.equal(event.disconnected, true);
  assert.equal(event.statusCode, null);
  toolkit.executeTool = async () => ({});
  assert.equal((await post('read_product', { id: 'recovered' })).status, 200);
});

test('request telemetry correlates outcomes and excludes secrets, arguments, records, and caller request IDs', async (t) => {
  const events = [];
  const { post, base } = await serve(t, { onAudit: (event) => events.push(event) });
  const response = await post(
    'read_product',
    { id: 'private-customer-reference' },
    { 'X-Request-Id': 'caller-controlled-private-id' },
  );
  assert.equal(response.status, 200);
  assert.match(response.headers.get('x-request-id'), /^[a-f0-9-]{36}$/);
  assert.equal(events[0].requestId, response.headers.get('x-request-id'));
  assert.equal(events[0].outcome, 'tool_completed');
  assert.equal(events[0].accountId, 'muse-store');
  assert.equal(events[0].mode, 'read');
  assert.equal(events[0].statusCode, 200);
  assert.ok(events[0].durationMs >= 0);
  assert.ok(Object.isFrozen(events[0]));
  await post('write_product', { amount: '19.99' });
  assert.equal(events.at(-1).outcome, 'preview');
  await fetch(`${base}/v1/tools?token=private-url-secret`);
  assert.equal(events.at(-1).accountId, null);
  assert.equal(events.at(-1).statusCode, 401);
  for (const sensitive of [
    KEY,
    'private-customer-reference',
    'caller-controlled-private-id',
    '19.99',
    'private-url-secret',
  ])
    assert.ok(!JSON.stringify(events).includes(sensitive));
  const deniedToolkit = stub([]);
  deniedToolkit.executeTool = async () => ({ success: true, result: { success: false } });
  const denied = await serve(t, {
    toolkit: deniedToolkit,
    allowApply: true,
    onAudit: (event) => events.push(event),
  });
  assert.equal(
    (
      await denied.post(
        'write_product',
        { amount: '19.99' },
        { 'Idempotency-Key': 'denied-write-key' },
      )
    ).status,
    200,
  );
  assert.equal(events.at(-1).outcome, 'commerce_denied');
  assert.equal(events.at(-1).mode, 'apply');
  for (const onAudit of [
    () => {
      throw new Error('sink failed');
    },
    async () => {
      throw new Error('async sink failed');
    },
  ]) {
    const other = await serve(t, { onAudit, allowApply: true });
    assert.equal(
      (
        await other.post(
          'write_product',
          { amount: '19.99' },
          { 'Idempotency-Key': 'audit-write-key' },
        )
      ).status,
      200,
    );
    assert.equal(other.calls.length, 1, 'Sink failure cannot replay an applied write');
  }
});

test('configuration rejects missing credentials, unscoped tools, and insecure public URLs', () => {
  const config = {
    toolkit: stub([]),
    apiKeys: [KEY],
    publicUrl: 'https://commerce.example',
    tools: ['read_product'],
  };
  assert.throws(() => createMuseConnector({ ...config, apiKeys: [] }), /API keys/);
  assert.throws(() => createMuseConnector({ ...config, tools: [] }), /allowlist/);
  assert.throws(() => createMuseConnector({ ...config, tools: ['not_available'] }), /unavailable/);
  assert.throws(
    () => createMuseConnector({ ...config, publicUrl: 'http://commerce.example' }),
    /HTTPS/,
  );
  assert.throws(
    () => createMuseConnector({ ...config, publicUrl: 'https://user:password@commerce.example' }),
    /HTTPS/,
  );
});
