import test from 'node:test';
import assert from 'node:assert/strict';
import { Wallet, Transaction, Interface, Signature, keccak256 } from 'ethers';
import Database from 'better-sqlite3';
import {
  buildSetPaymentAuthorization,
  buildSetBatchCalldata,
  createSetTransactionVerifier,
  sequencerUuidToBytes32,
} from '../../src/x402/set-transaction.js';
import { createDurableSetSubmission } from '../../../bindings/node/purchase-runtime.mjs';

// Public deterministic test keys. No provider attached; nothing is broadcast.
const payer = new Wallet(`0x${'11'.repeat(32)}`);
const relayer = new Wallet(`0x${'22'.repeat(32)}`);
const address = (n) => `0x${n.repeat(40)}`;
const bytes32 = (n) => `0x${n.repeat(64)}`;
const intent = {
  chainId: '31337',
  settlementContract: address('4'),
  payer: payer.address,
  payee: address('3'),
  token: address('5'),
  amount: '40000000',
  validUntil: '4070908800',
  intentId: sequencerUuidToBytes32('12345678-1234-4234-8234-123456789abc'),
  idempotencyKey: 'purchase:1:pay',
};
const limits = {
  relayer: relayer.address,
  maxGasLimit: '500000',
  maxFeePerGas: '10000000000',
  maxPriorityFeePerGas: '1000000000',
};
async function fixture() {
  const payerNonce = '7';
  const typed = buildSetPaymentAuthorization({ intent, payerNonce, validAfter: '0' });
  const batch = {
    batchId: bytes32('a'),
    merkleRoot: bytes32('b'),
    tenantStoreKey: bytes32('c'),
    sequenceStart: '10',
    sequenceEnd: '10',
    validAfter: '0',
    signingHash: bytes32('d'),
    authorization: await payer.signTypedData(typed.domain, typed.types, typed.value),
  };
  const transaction = {
    nonce: '9',
    gasLimit: '300000',
    maxFeePerGas: '2000000000',
    maxPriorityFeePerGas: '1000000000',
  };
  const input = { intent: structuredClone(intent), payerNonce, plan: { batch, transaction } };
  const unsigned = {
    type: 2,
    chainId: BigInt(intent.chainId),
    to: intent.settlementContract,
    nonce: 9,
    gasLimit: 300000n,
    maxFeePerGas: 2000000000n,
    maxPriorityFeePerGas: 1000000000n,
    value: 0n,
    data: buildSetBatchCalldata({ intent, payerNonce, batch }),
  };
  return { input, unsigned, verify: createSetTransactionVerifier(limits) };
}

test('sequencer UUID encoding matches Rust byte placement and rejects SHA-hash identities', () => {
  assert.equal(
    intent.intentId,
    '0x12345678123442348234123456789abc00000000000000000000000000000000',
  );
  assert.throws(() => sequencerUuidToBytes32(bytes32('a')), /UUID/);
  assert.throws(() => sequencerUuidToBytes32('00000000-0000-0000-0000-000000000000'), /UUID/);
});

test('real EIP-712 authorization and signed EIP-1559 transaction verify against Set ABI', async () => {
  const f = await fixture();
  const raw = await relayer.signTransaction(f.unsigned);
  assert.equal(await f.verify(f.input, raw), keccak256(raw));
  const abi = new Interface([
    'function settleBatch(bytes32,bytes32,bytes32,uint64,uint64,(bytes32,address,address,uint256,address,uint64,uint64,uint64,bytes32,bytes)[])',
  ]);
  const decoded = abi.decodeFunctionData('settleBatch', Transaction.from(raw).data);
  assert.equal(decoded[5].length, 1);
  assert.equal(decoded[5][0][0], intent.intentId);
  assert.equal(decoded[5][0][3], 40000000n);
  assert.equal(decoded[5][0][5], 7n);
});

for (const [field, replacement] of Object.entries({
  amount: '40000001',
  payee: address('6'),
  token: address('7'),
  payer: address('8'),
  chainId: '1',
  settlementContract: address('9'),
  validUntil: '4070908801',
  intentId: bytes32('f'),
})) {
  test(`payer authorization rejects substituted ${field}`, async () => {
    const f = await fixture();
    f.input.intent[field] = replacement;
    assert.throws(() => buildSetBatchCalldata({ ...f.input, batch: f.input.plan.batch }));
  });
}

for (const [name, change] of Object.entries({
  recipient: { to: address('9') },
  chain: { chainId: 1n },
  value: { value: 1n },
  nonce: { nonce: 10 },
  gas: { gasLimit: 300001n },
  fee: { maxFeePerGas: 2000000001n },
  tip: { maxPriorityFeePerGas: 1000000001n },
  calldata: { data: '0x' },
  accessList: { accessList: [{ address: address('9'), storageKeys: [] }] },
})) {
  test(`signed transaction rejects unauthorized ${name}`, async () => {
    const f = await fixture();
    const raw = await relayer.signTransaction({ ...f.unsigned, ...change });
    await assert.rejects(f.verify(f.input, raw));
  });
}

test('rejects wrong relayer, excessive gas ceilings and changed payer nonce', async () => {
  const f = await fixture();
  await assert.rejects(f.verify(f.input, await payer.signTransaction(f.unsigned)));
  const raw = await relayer.signTransaction(f.unsigned);
  await assert.rejects(createSetTransactionVerifier({ ...limits, maxGasLimit: '1' })(f.input, raw));
  await assert.rejects(f.verify({ ...f.input, payerNonce: '8' }, raw));
  f.input.plan.batch.validAfter = '1';
  await assert.rejects(f.verify(f.input, raw));
});

test('rejects compact and normalized-v signatures not accepted by the contract EOA bytes path', async () => {
  const f = await fixture();
  const signature = Signature.from(f.input.plan.batch.authorization);
  for (const authorization of [
    signature.compactSerialized,
    `${signature.serialized.slice(0, -2)}${signature.yParity.toString(16).padStart(2, '0')}`,
  ]) {
    assert.throws(
      () =>
        buildSetBatchCalldata({
          intent,
          payerNonce: f.input.payerNonce,
          batch: { ...f.input.plan.batch, authorization },
        }),
      /canonical 65-byte/,
    );
  }
});

test('journal uses real signature verification and recovers identical signed transaction bytes', async () => {
  const db = new Database(':memory:');
  try {
    let signatures = 0;
    const broadcasts = [];
    let fail = true;
    const service = createDurableSetSubmission({
      db,
      scope: 'test',
      nonceStart: '7',
      allowSubmit: true,
      authorize: async () => true,
      prepare: async ({ intent: expected, payerNonce }) => {
        const f = await fixture();
        assert.deepEqual(expected, intent);
        assert.equal(payerNonce, f.input.payerNonce);
        return f.input.plan;
      },
      sign: async ({ intent: expected, payerNonce, plan }) => {
        signatures++;
        const rawTransaction = await relayer.signTransaction({
          type: 2,
          chainId: BigInt(expected.chainId),
          to: expected.settlementContract,
          nonce: Number(plan.transaction.nonce),
          gasLimit: plan.transaction.gasLimit,
          maxFeePerGas: plan.transaction.maxFeePerGas,
          maxPriorityFeePerGas: plan.transaction.maxPriorityFeePerGas,
          value: 0,
          data: buildSetBatchCalldata({ intent: expected, payerNonce, batch: plan.batch }),
        });
        return { rawTransaction, transactionHash: keccak256(rawTransaction) };
      },
      validateSigned: createSetTransactionVerifier(limits),
      broadcast: async (raw) => {
        broadcasts.push(raw);
        if (fail) throw new Error('lost response');
        return keccak256(raw);
      },
    });
    await assert.rejects(service.submit(intent), /lost response/);
    const hash = await service.findTransaction(intent);
    fail = false;
    assert.equal((await service.recover())[0].transactionHash, hash);
    assert.equal(signatures, 1);
    assert.equal(broadcasts.length, 2);
    assert.equal(broadcasts[0], broadcasts[1]);
  } finally {
    db.close();
  }
});
