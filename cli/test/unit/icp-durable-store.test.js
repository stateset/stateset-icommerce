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
import {
  ICPClient,
  generateIdentity,
  generatePrincipalIdentity,
} from '../../../packages/icp-client/src/index.mjs';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const launcher = resolve(root, 'cli/examples/durable-merchant.mjs');

// These lifecycle tests run the durable merchant in its DEFAULT posture:
// enforcing trust. `--demo` describes simulated economic rails, not identity,
// and ICP_TRUST_MODE is never set below. The operator registers this
// principal's public key; the SDK signs each Intent's PrincipalBinding with
// the matching private key. (Until the SDK could sign a real binding, this
// suite had to set ICP_TRUST_MODE=demo to transact at all.)
const principalIdentity = generatePrincipalIdentity();
const PRINCIPALS = ['did:web:demo.example', 'did:web:concurrency.example'];
const principalKeyHex = principalIdentity.ed25519_pubkey.toString('hex');
const allPrincipalKeys = JSON.stringify(
  Object.fromEntries(PRINCIPALS.map((did) => [did, principalKeyHex])),
);
/** Operator registration for the agent AIDs a test intends to serve. */
const agentKeys = (...identities) =>
  JSON.stringify(
    Object.fromEntries(identities.map((id) => [id.aid, id.ed25519_pubkey.toString('hex')])),
  );

async function start(path, keyFile, env = {}) {
  const childEnv = {
    ...process.env,
    PORT: '0',
    ICP_MERCHANT_KEY_FILE: keyFile,
    ICP_MERCHANT_AID: 'aid:v1:zDurableDemoMerchant',
    ICP_PRINCIPAL_KEYS_JSON: allPrincipalKeys,
    ...env,
  };
  // This suite asserts the ENFORCING posture. Inheriting process.env means an
  // ambient ICP_TRUST_MODE=demo in the developer's shell or the CI job would
  // silently downgrade every assertion below to "the handler checked nothing",
  // and the suite would still pass. Never inherit it.
  delete childEnv.ICP_TRUST_MODE;
  const proc = spawn(process.execPath, [launcher, '--apply', '--demo', '--db', path], {
    env: childEnv,
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
    // The agent AID must be registered before the handler starts: an
    // operator-keyed handler admits no caller-minted signers.
    const identity = generateIdentity();
    const env = { ICP_AGENT_KEYS_JSON: agentKeys(identity) };
    const delegated = {
      handlerUrl: '',
      principal: 'did:web:demo.example',
      identity,
      principalIdentity,
    };
    worker = await start(path, keyFile, env);
    let client = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
    const caps = await client.capabilities();
    const { quote } = await client.purchase({
      merchant: caps.merchant_aid,
      settler: 'settler:stateset.usdc.base-sepolia',
      items: [{ sku: 'SKU-100', quantity: 50, unit_price: { amount: '1', currency: 'USDC' } }],
      max_total: { amount: '60', currency: 'USDC' },
    });
    await kill(worker.proc);
    worker = await start(path, keyFile, env);
    client = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
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
    worker = await start(path, keyFile, env);
    client = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
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
    worker = await start(path, keyFile, env);
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
    const identities = [generateIdentity(), generateIdentity()];
    const env = { ICP_AGENT_KEYS_JSON: agentKeys(...identities) };
    workers.push(await start(path, keyFile, env));
    workers.push(await start(path, keyFile, env));
    const clients = await Promise.all(
      workers.map((worker, i) =>
        ICPClient.create({
          handlerUrl: worker.url,
          principal: 'did:web:concurrency.example',
          identity: identities[i],
          principalIdentity,
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
      principalIdentity,
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
    const identity = generateIdentity();
    const delegated = { principal, identity, principalIdentity };
    worker = await start(path, keyFile);
    const caps = await (await fetch(`${worker.url}/icp/v1/.well-known/icp`)).json();
    assert.equal(caps.trust_mode, 'enforce');

    // A self-minted signer AID is not admitted by an operator-keyed handler.
    const client = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
    assert.equal(await codeOf(() => purchase(client)), 'auth.aid_unregistered');

    // Register the agent. An Agent carrying NO delegation is refused outright:
    // the client no longer fabricates a self-signed one to paper over this.
    await kill(worker.proc);
    const agents = agentKeys(identity);
    worker = await start(path, keyFile, { ICP_AGENT_KEYS_JSON: agents });
    const undelegated = await ICPClient.create({ handlerUrl: worker.url, principal, identity });
    assert.equal(await codeOf(() => purchase(undelegated)), 'delegation.required');

    // A properly signed binding still needs a principal the OPERATOR knows.
    const known = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
    assert.equal(await codeOf(() => purchase(known)), 'delegation.principal_unknown');

    // Register that principal under somebody else's key: the binding fails
    // closed on the signature instead of being waved through.
    await kill(worker.proc);
    const impostor = generatePrincipalIdentity();
    worker = await start(path, keyFile, {
      ICP_AGENT_KEYS_JSON: agents,
      ICP_PRINCIPAL_KEYS_JSON: JSON.stringify({
        [principal]: impostor.ed25519_pubkey.toString('hex'),
      }),
    });
    const wrongKey = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
    assert.equal(await codeOf(() => purchase(wrongKey)), 'delegation.signature_invalid');

    // Register the principal's real key: the same agent now transacts with
    // trust ENFORCED and ICP_TRUST_MODE set nowhere.
    await kill(worker.proc);
    worker = await start(path, keyFile, {
      ICP_AGENT_KEYS_JSON: agents,
      ICP_PRINCIPAL_KEYS_JSON: JSON.stringify({ [principal]: principalKeyHex }),
    });
    const authorized = await ICPClient.create({ ...delegated, handlerUrl: worker.url });
    const { quote } = await purchase(authorized);
    assert.ok(quote.quote_id, JSON.stringify(quote));
  } finally {
    if (worker) await kill(worker.proc);
    rmSync(dir, { recursive: true });
  }
});
