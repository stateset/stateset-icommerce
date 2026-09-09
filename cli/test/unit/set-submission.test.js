import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { createDurableSetSubmission } from '../../../bindings/node/purchase-runtime.mjs';

const intent = (n = '1') => ({
  chainId: '31337',
  settlementContract: `0x${'4'.repeat(40)}`,
  payer: `0x${'2'.repeat(40)}`,
  payee: `0x${'3'.repeat(40)}`,
  token: `0x${'5'.repeat(40)}`,
  amount: '40000000',
  validUntil: '4070908800',
  intentId: `0x${n.repeat(64)}`,
  idempotencyKey: `pay:${n}`,
});
// Fake signed encoding/hash, deliberately not an Ethereum signature or keccak.
const hash = (raw) => `0x${createHash('sha256').update(raw).digest('hex')}`;
function fixture() {
  const directory = mkdtempSync(join(tmpdir(), 'set-submission-'));
  let db = new Database(join(directory, 'journal.db'));
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = FULL');
  const calls = { prepare: 0, sign: 0, broadcast: [] };
  const controls = { allowed: true, lost: false, revokeAfterSign: false };
  const options = {
    db,
    scope: 'tenant:buyer',
    nonceStart: '0',
    allowSubmit: true,
    authorize: async () => controls.allowed,
    prepare: async (input) => {
      calls.prepare++;
      const row = db
        .prepare('SELECT * FROM _stateset_set_submissions WHERE request_key=?')
        .get(input.intent.idempotencyKey);
      assert.equal(row.payer_nonce, input.payerNonce);
      return { relayerNonce: input.payerNonce, operation: input.intent.intentId };
    },
    sign: async (input) => {
      const row = db
        .prepare('SELECT * FROM _stateset_set_submissions WHERE request_key=?')
        .get(input.intent.idempotencyKey);
      assert.deepEqual(JSON.parse(row.plan), input.plan);
      calls.sign++;
      const rawTransaction = `0x${Buffer.from(JSON.stringify({ input, salt: calls.sign })).toString('hex')}`;
      if (controls.revokeAfterSign) controls.allowed = false;
      return { rawTransaction, transactionHash: hash(rawTransaction) };
    },
    validateSigned: async (input, raw) => {
      assert.deepEqual(JSON.parse(Buffer.from(raw.slice(2), 'hex').toString()).input, input);
      return hash(raw);
    },
    broadcast: async (raw) => {
      const body = JSON.parse(Buffer.from(raw.slice(2), 'hex').toString());
      const row = db
        .prepare('SELECT artifact FROM _stateset_set_submissions WHERE request_key=?')
        .get(body.input.intent.idempotencyKey);
      assert.equal(JSON.parse(row.artifact).rawTransaction, raw);
      calls.broadcast.push(raw);
      if (controls.lost) throw new Error('lost broadcast response');
      return hash(raw);
    },
  };
  const service = () => createDurableSetSubmission({ ...options, db });
  const first = service();
  return {
    first,
    options,
    controls,
    calls,
    service,
    get db() {
      return db;
    },
    reopen() {
      db.close();
      db = new Database(join(directory, 'journal.db'));
      return service();
    },
    close() {
      db.close();
      rmSync(directory, { recursive: true, force: true });
    },
  };
}

test('journal checkpoints intent, nonce, plan and bytes before broadcast; replay returns same hash', async () => {
  const f = fixture();
  try {
    const tx = await f.first.submit(intent());
    assert.equal(await f.first.submit(intent()), tx);
    assert.equal(await f.first.findTransaction(intent()), tx);
    assert.equal(f.calls.prepare, 1);
    assert.equal(f.calls.sign, 1);
    assert.equal(f.calls.broadcast.length, 1);
    assert.equal(await f.first.findTransaction(intent('2')), null);
    assert.deepEqual(await f.first.recover(), []);
  } finally {
    f.close();
  }
});

test('lost broadcast response survives reopening without another signature or transaction', async () => {
  const f = fixture();
  try {
    f.controls.lost = true;
    await assert.rejects(f.first.submit(intent()), /lost broadcast/);
    const tx = await f.first.findTransaction(intent());
    const recovery = f.reopen();
    assert.equal(await recovery.findTransaction(intent()), tx);
    f.controls.lost = false;
    assert.equal((await recovery.recover())[0].transactionHash, tx);
    assert.equal(f.calls.sign, 1);
    assert.equal(f.calls.prepare, 1);
    assert.equal(f.calls.broadcast.length, 2);
    assert.equal(new Set(f.calls.broadcast).size, 1);
  } finally {
    f.close();
  }
});

test('artifact checkpoint failure never broadcasts; recovery preserves nonce and plan', async () => {
  const f = fixture();
  try {
    f.db.exec(`CREATE TRIGGER fail_artifact BEFORE UPDATE OF artifact ON _stateset_set_submissions
      BEGIN SELECT RAISE(ABORT, 'artifact disk failure'); END;`);
    await assert.rejects(f.first.submit(intent()), /artifact disk failure/);
    assert.equal(f.calls.broadcast.length, 0);
    assert.equal(await f.first.findTransaction(intent()), null);
    f.db.exec('DROP TRIGGER fail_artifact');
    await f.reopen().submit(intent());
    assert.equal(f.calls.prepare, 1);
    assert.equal(f.calls.broadcast.length, 1);
    assert.equal(f.db.prepare('SELECT next_nonce FROM _stateset_set_nonces').get().next_nonce, '1');
  } finally {
    f.close();
  }
});

test('concurrent workers broadcast only the persisted winning signed bytes', async () => {
  const f = fixture();
  try {
    const results = await Promise.all(
      Array.from({ length: 6 }, () => f.service().submit(intent())),
    );
    assert.equal(new Set(results).size, 1);
    assert.equal(new Set(f.calls.broadcast).size, 1);
    assert.equal(f.db.prepare('SELECT next_nonce FROM _stateset_set_nonces').get().next_nonce, '1');
  } finally {
    f.close();
  }
});

test('scope isolation and cross-scope payer nonce allocation', async () => {
  const f = fixture();
  try {
    await f.first.submit(intent());
    const other = createDurableSetSubmission({ ...f.options, scope: 'other' });
    assert.equal(await other.findTransaction(intent()), null);
    await other.submit(intent('2'));
    assert.deepEqual(
      f.db.prepare('SELECT payer_nonce FROM _stateset_set_submissions ORDER BY payer_nonce').all(),
      [{ payer_nonce: '0' }, { payer_nonce: '1' }],
    );
    await assert.rejects(other.submit(intent()), /UNIQUE/);
    assert.equal(f.db.prepare('SELECT next_nonce FROM _stateset_set_nonces').get().next_nonce, '2');
  } finally {
    f.close();
  }
});

test('changed intent, invalid signed bytes and modified stored plan fail closed', async () => {
  const f = fixture();
  try {
    await f.first.submit(intent());
    await assert.rejects(f.first.submit({ ...intent(), amount: '1' }), /conflict/);
    await assert.rejects(
      f.first.findTransaction({ ...intent(), payee: `0x${'6'.repeat(40)}` }),
      /conflict/,
    );
    f.db.prepare("UPDATE _stateset_set_submissions SET plan='{}'").run();
    await assert.rejects(f.first.findTransaction(intent()));
    await assert.rejects(f.first.submit(intent()));
    assert.equal(f.calls.broadcast.length, 1);
    const invalid = createDurableSetSubmission({
      ...f.options,
      sign: async () => ({ rawTransaction: '0x01', transactionHash: `0x${'f'.repeat(64)}` }),
    });
    await assert.rejects(invalid.submit(intent('2')));
    assert.equal(f.calls.broadcast.length, 1);
  } finally {
    f.close();
  }
});

test('revocation after signing prevents broadcast but keeps transaction lookup available', async () => {
  const f = fixture();
  try {
    f.controls.revokeAfterSign = true;
    await assert.rejects(f.first.submit(intent()), /not authorized/);
    assert.equal(f.calls.broadcast.length, 0);
    assert.match(await f.first.findTransaction(intent()), /^0x[0-9a-f]{64}$/);
    const readOnly = createDurableSetSubmission({ ...f.options, allowSubmit: false });
    assert.equal(await readOnly.findTransaction(intent()), await f.first.findTransaction(intent()));
    await assert.rejects(readOnly.submit(intent()), /disabled/);
    await assert.rejects(readOnly.recover(), /disabled/);
    assert.equal((await f.first.recover())[0].error, 'submission not authorized');
  } finally {
    f.close();
  }
});

test('recovery cursor advances past denied work and broadcast hash mismatch retains artifact', async () => {
  const f = fixture();
  try {
    f.controls.lost = true;
    await assert.rejects(f.first.submit(intent()));
    await assert.rejects(f.first.submit(intent('2')));
    f.controls.lost = false;
    const service = createDurableSetSubmission({
      ...f.options,
      authorize: async (i) => i.idempotencyKey !== 'pay:1',
    });
    const first = await service.recover({ limit: 1 });
    assert.ok(first[0].error);
    const second = await service.recover({ limit: 1, after: first[0].idempotencyKey });
    assert.ok(second[0].transactionHash);
    const wrong = createDurableSetSubmission({
      ...f.options,
      broadcast: async () => `0x${'f'.repeat(64)}`,
    });
    await assert.rejects(wrong.submit(intent()), /hash mismatch/);
    assert.match(await wrong.findTransaction(intent()), /^0x[0-9a-f]{64}$/);
  } finally {
    f.close();
  }
});

test('nonce exhaustion and changed nonce configuration fail without new effects', async () => {
  const f = fixture();
  try {
    const maximum = ((1n << 64n) - 1n).toString();
    const service = createDurableSetSubmission({ ...f.options, nonceStart: maximum });
    await service.submit(intent());
    await assert.rejects(service.submit(intent('2')), /nonce space exhausted/);
    await assert.rejects(f.first.submit(intent('2')), /nonce configuration conflict/);
    assert.equal(f.calls.broadcast.length, 1);
    assert.equal(
      f.db.prepare('SELECT COUNT(*) AS count FROM _stateset_set_submissions').get().count,
      1,
    );
  } finally {
    f.close();
  }
});

test('disabled, denied and expired submissions do not allocate nonces or invoke signers', async () => {
  const f = fixture();
  try {
    const disabled = createDurableSetSubmission({ ...f.options, allowSubmit: undefined });
    await assert.rejects(disabled.submit(intent()), /disabled/);
    f.controls.allowed = false;
    await assert.rejects(f.first.submit(intent()), /not authorized/);
    f.controls.allowed = true;
    await assert.rejects(f.first.submit({ ...intent(), validUntil: '1' }), /expired/);
    assert.equal(f.db.prepare('SELECT COUNT(*) AS count FROM _stateset_set_nonces').get().count, 0);
    assert.equal(f.calls.prepare, 0);
    assert.equal(f.calls.sign, 0);
    assert.equal(f.calls.broadcast.length, 0);
  } finally {
    f.close();
  }
});
