import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateKeyPairSync } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import Database from 'better-sqlite3';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';
import { ICPClient } from '../../../packages/icp-client/src/index.mjs';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const launcher = resolve(root, 'cli/examples/durable-merchant.mjs');

async function start(path, keyFile, env = {}) {
  const proc = spawn(process.execPath, [launcher, '--apply', '--demo', '--db', path], {
    env: {
      ...process.env,
      PORT: '0',
      ICP_MERCHANT_KEY_FILE: keyFile,
      ICP_MERCHANT_AID: 'aid:v1:zDurableDemoMerchant',
      // The reference ICPClient still self-signs its PrincipalBinding with a
      // placeholder signature (packages/icp-client `_principalBinding`, kid
      // 'self', sig 'deadbeef'), so these lifecycle tests opt out of trust
      // enforcement EXPLICITLY. `--demo` no longer does this implicitly: it
      // describes simulated economic rails, not identity.
      // FOLLOW-UP: teach ICPClient to sign a real binding with a principal key
      // and register it via ICP_PRINCIPAL_KEYS_JSON, then delete this line and
      // the ICP_TRUST_MODE=demo branch in icp-handler/src/server.mjs.
      ICP_TRUST_MODE: 'demo',
      ...env,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  const url = await new Promise((resolveUrl, reject) => {
    const timer = setTimeout(() => {
      proc.kill('SIGKILL');
      reject(new Error('merchant startup timeout'));
    }, 10000);
    proc.once('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
    proc.once('exit', () => {
      clearTimeout(timer);
      reject(new Error(`merchant exited: ${output}`));
    });
    proc.stderr.on('data', (chunk) => {
      output += chunk;
      const match = output.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (match) {
        clearTimeout(timer);
        resolveUrl(match[1]);
      }
    });
    proc.stdout.resume();
  });
  return { proc, url };
}
async function kill(proc) {
  if (proc.exitCode !== null || proc.signalCode !== null) return;
  const exited = once(proc, 'exit');
  proc.kill('SIGKILL');
  await exited;
}

test('durable nonces survive reconnect, enforce capacity and rollback with economic state', () => {
  const dir = mkdtempSync(join(tmpdir(), 'icp-nonce-'));
  const path = join(dir, 'merchant.db');
  let db = new Database(path);
  try {
    let store = new SqliteProtocolStore(db);
    let guard = store.replayGuard({ maxEntries: 1, now: () => 1000 });
    assert.throws(
      () =>
        store.atomic(() => {
          assert.equal(guard.checkAndRecord('buyer', 'nonce'), true);
          store.collection('intents').set('intent', { id: 'intent' });
          throw new Error('signing failed');
        }),
      /signing failed/,
    );
    assert.equal(guard.size(), 0);
    assert.equal(store.collection('intents').size, 0);
    assert.equal(guard.checkAndRecord('buyer', 'nonce'), true);
    db.close();
    db = new Database(path);
    store = new SqliteProtocolStore(db);
    guard = store.replayGuard({ maxEntries: 1, now: () => 2000 });
    assert.equal(guard.checkAndRecord('buyer', 'nonce'), false);
    assert.equal(guard.checkAndRecord('buyer', 'another'), false);
    assert.equal(
      store.replayGuard({ maxEntries: 1, now: () => 86401000 }).checkAndRecord('buyer', 'another'),
      true,
    );
    store.bindIdentity({ aid: 'merchant', publicKey: '11'.repeat(32) });
    assert.throws(
      () => store.bindIdentity({ aid: 'merchant', publicKey: '22'.repeat(32) }),
      /identity/,
    );
    assert.throws(() => store.atomic(async () => {}), /synchronous/);
  } finally {
    db.close();
    rmSync(dir, { recursive: true });
  }
});

test('SIGKILL during acceptance rolls back inventory, escrow and events together', () => {
  const dir = mkdtempSync(join(tmpdir(), 'icp-crash-'));
  const path = join(dir, 'merchant.db');
  try {
    const child = spawnSync(process.execPath, [
      resolve(root, 'cli/test/fixtures/icp-crash-transaction.mjs'),
      path,
    ]);
    assert.equal(child.signal, 'SIGKILL', child.stderr.toString());
    const db = new Database(path);
    try {
      const store = new SqliteProtocolStore(db);
      assert.equal(store.collection('inventory').get('SKU-100'), 100);
      for (const namespace of ['reservations', 'escrows', 'events'])
        assert.equal(store.collection(namespace).size, 0);
    } finally {
      db.close();
    }
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test('HTTP merchant survives quote, acceptance and settlement restarts with stable keys and one reservation', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'icp-http-durable-'));
  const path = join(dir, 'merchant.db');
  const keyFile = join(dir, 'merchant.pem');
  const { privateKey } = generateKeyPairSync('ed25519');
  writeFileSync(keyFile, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  let worker;
  try {
    worker = await start(path, keyFile);
    let client = await ICPClient.create({
      handlerUrl: worker.url,
      principal: 'did:web:demo.example',
    });
    const identity = client.identity;
    const caps = await client.capabilities();
    const { quote } = await client.purchase({
      merchant: caps.merchant_aid,
      settler: 'settler:stateset.usdc.base-sepolia',
      items: [{ sku: 'SKU-100', quantity: 50, unit_price: { amount: '1', currency: 'USDC' } }],
      max_total: { amount: '60', currency: 'USDC' },
    });
    await kill(worker.proc);
    worker = await start(path, keyFile);
    client = await ICPClient.create({
      handlerUrl: worker.url,
      principal: 'did:web:demo.example',
      identity,
    });
    assert.deepEqual(await client.capabilities(), caps);
    const inspection = new Database(path);
    const original = new SqliteProtocolStore(inspection).collection('intents').get(quote.intent_id);
    inspection.close();
    const replay = await fetch(`${worker.url}/icp/v1/intents`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        intent: original.intent,
        signature: { alg: 'ed25519', kid: identity.aid, sig: original.signatureHex },
        _pubkey_hex: identity.ed25519_pubkey.toString('hex'),
        _x_pubkey_hex: identity.x25519_pubkey.toString('hex'),
      }),
    });
    assert.equal((await replay.json()).code, 'replay.nonce_seen');
    // Inject a database failure after inventory/order writes but before the
    // acceptance event can commit. The HTTP response must not claim success.
    const faultDb = new Database(path);
    try {
      faultDb.exec(`CREATE TRIGGER reject_acceptance BEFORE INSERT ON _icp_records
        WHEN NEW.namespace='events' BEGIN SELECT RAISE(ABORT,'injected event failure'); END`);
      await assert.rejects(client.accept(quote.quote_id), /internal.transaction_failed/);
      const failedStore = new SqliteProtocolStore(faultDb);
      assert.equal(failedStore.collection('reservations').size, 0);
      assert.equal(failedStore.collection('escrows').size, 0);
      faultDb.exec('DROP TRIGGER reject_acceptance');
    } finally {
      faultDb.close();
    }
    const accepted = await client.accept(quote.quote_id);
    await kill(worker.proc);
    worker = await start(path, keyFile);
    client = await ICPClient.create({
      handlerUrl: worker.url,
      principal: 'did:web:demo.example',
      identity,
    });
    assert.deepEqual(await client.accept(quote.quote_id), accepted);
    const wrongBuyer = await ICPClient.create({
      handlerUrl: worker.url,
      principal: 'did:web:other.example',
    });
    await assert.rejects(wrongBuyer.accept(quote.quote_id), /auth.acceptance_invalid/);
    const fulfill = async () => {
      const response = await fetch(
        `${worker.url}/icp/v1/escrows/${accepted.funding.escrow_id}/fulfill`,
        {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: '{}',
        },
      );
      assert.equal(response.status, 200);
      return response.json();
    };
    const settled = await fulfill();
    await kill(worker.proc);
    worker = await start(path, keyFile);
    assert.deepEqual(await fulfill(), settled);
    const db = new Database(path);
    try {
      const store = new SqliteProtocolStore(db);
      assert.equal(store.collection('inventory').get('SKU-100'), 50);
      assert.equal(store.collection('reservations').size, 1);
      assert.equal(store.collection('escrows').size, 1);
      assert.equal(store.collection('settlements').size, 1);
      assert.deepEqual(
        store
          .collection('events')
          .get(accepted.funding.escrow_id)
          .map((event) => event.seq),
        [0, 1, 2, 3],
      );
    } finally {
      db.close();
    }
  } finally {
    if (worker) await kill(worker.proc);
    rmSync(dir, { recursive: true });
  }
});

test('two merchant processes serialize competing inventory commitments in one database', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'icp-workers-'));
  const path = join(dir, 'merchant.db');
  const keyFile = join(dir, 'merchant.pem');
  const { privateKey } = generateKeyPairSync('ed25519');
  writeFileSync(keyFile, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  const workers = [];
  try {
    workers.push(await start(path, keyFile));
    workers.push(await start(path, keyFile));
    const clients = await Promise.all(
      workers.map((worker) =>
        ICPClient.create({
          handlerUrl: worker.url,
          principal: 'did:web:concurrency.example',
        }),
      ),
    );
    const quotes = await Promise.all(
      clients.map((client) =>
        client.purchase({
          merchant: 'aid:v1:zDurableDemoMerchant',
          settler: 'settler:stateset.usdc.base-sepolia',
          items: [{ sku: 'SKU-100', quantity: 60, unit_price: { amount: '1', currency: 'USDC' } }],
          max_total: { amount: '70', currency: 'USDC' },
        }),
      ),
    );
    const results = await Promise.allSettled(
      clients.map((client, i) => client.accept(quotes[i].quote.quote_id)),
    );
    assert.equal(results.filter((result) => result.status === 'fulfilled').length, 1);
    assert.equal(
      results.find((result) => result.status === 'rejected').reason.code,
      'inventory.insufficient',
    );
    const winner = results.findIndex((result) => result.status === 'fulfilled');
    const replica = await ICPClient.create({
      handlerUrl: workers[1 - winner].url,
      principal: 'did:web:concurrency.example',
      identity: clients[winner].identity,
    });
    assert.deepEqual(await replica.accept(quotes[winner].quote.quote_id), results[winner].value);
    const db = new Database(path);
    try {
      const store = new SqliteProtocolStore(db);
      assert.equal(store.collection('inventory').get('SKU-100'), 40);
      assert.equal(store.collection('escrows').size, 1);
      assert.equal(store.collection('reservations').size, 1);
    } finally {
      db.close();
    }
  } finally {
    await Promise.all(workers.map((worker) => kill(worker.proc)));
    rmSync(dir, { recursive: true });
  }
});

test('a durable merchant enforces trust unless the operator opts out', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'icp-trust-'));
  const path = join(dir, 'merchant.db');
  const keyFile = join(dir, 'merchant.pem');
  const { privateKey } = generateKeyPairSync('ed25519');
  writeFileSync(keyFile, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  const principal = 'did:web:unregistered.example';
  const purchase = (client) =>
    client.purchase({
      merchant: 'aid:v1:zDurableDemoMerchant',
      settler: 'settler:stateset.usdc.base-sepolia',
      items: [{ sku: 'SKU-100', quantity: 1, unit_price: { amount: '1', currency: 'USDC' } }],
      max_total: { amount: '10', currency: 'USDC' },
    });
  const codeOf = async (fn) => {
    try {
      await fn();
      return null;
    } catch (error) {
      return error.code ?? error.message;
    }
  };
  let worker;
  try {
    // `--demo` is passed by the launcher (simulated economic rails) and no
    // ICP_TRUST_MODE is set, so the handler must still enforce.
    worker = await start(path, keyFile, { ICP_TRUST_MODE: undefined });
    const caps = await (await fetch(`${worker.url}/icp/v1/.well-known/icp`)).json();
    assert.equal(caps.trust_mode, 'enforce');

    const client = await ICPClient.create({ handlerUrl: worker.url, principal });
    const identity = client.identity;
    // A self-minted signer AID is not admitted by an operator-keyed handler.
    assert.equal(await codeOf(() => purchase(client)), 'auth.aid_unregistered');

    // Register the agent: the intent now reaches the delegation check, and the
    // principal it names has no operator-configured key.
    await kill(worker.proc);
    const agents = JSON.stringify({ [identity.aid]: identity.ed25519_pubkey.toString('hex') });
    worker = await start(path, keyFile, {
      ICP_TRUST_MODE: undefined,
      ICP_AGENT_KEYS_JSON: agents,
    });
    const known = await ICPClient.create({ handlerUrl: worker.url, principal, identity });
    assert.equal(await codeOf(() => purchase(known)), 'delegation.principal_unknown');

    // Register the principal too: the reference client still self-signs its
    // binding with a placeholder signature, so it fails closed on the
    // signature rather than being waved through. See the FOLLOW-UP in start().
    await kill(worker.proc);
    worker = await start(path, keyFile, {
      ICP_TRUST_MODE: undefined,
      ICP_AGENT_KEYS_JSON: agents,
      ICP_PRINCIPAL_KEYS_JSON: JSON.stringify({ [principal]: '11'.repeat(32) }),
    });
    const delegated = await ICPClient.create({ handlerUrl: worker.url, principal, identity });
    assert.equal(await codeOf(() => purchase(delegated)), 'delegation.signature_invalid');
  } finally {
    if (worker) await kill(worker.proc);
    rmSync(dir, { recursive: true });
  }
});
