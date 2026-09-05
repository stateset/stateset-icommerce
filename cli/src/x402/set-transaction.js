// Operator-only SetPaymentBatch codec. Not registered as a model-facing tool.
import { Interface, Transaction, TypedDataEncoder, getAddress, verifyTypedData } from 'ethers';

/** @typedef {import('../../../bindings/node/purchase-runtime').SetPaymentIntent} SetIntent */
/** @typedef {{batchId:string, merkleRoot:string, tenantStoreKey:string, sequenceStart:string,
 * sequenceEnd:string, validAfter?:string, signingHash:string, authorization:string}} SetBatch */
/** @typedef {{nonce:string,gasLimit:string,maxFeePerGas:string,maxPriorityFeePerGas:string}} GasPlan */
/** @typedef {{intent:SetIntent,payerNonce:string,plan:{batch:SetBatch,transaction:GasPlan}}} VerificationInput */

const ABI = [
  'function settleBatch(bytes32 batchId,bytes32 merkleRoot,bytes32 tenantStoreKey,uint64 sequenceStart,uint64 sequenceEnd,(bytes32 intentId,address payer,address payee,uint256 amount,address token,uint64 nonce,uint64 validAfter,uint64 validUntil,bytes32 signingHash,bytes authorization)[] payments)',
];
const iface = new Interface(ABI);
const U64 = (1n << 64n) - 1n;
const U256 = (1n << 256n) - 1n;
/** @param {unknown} value @param {bigint} [max] */
function uint(value, max = U256) {
  if (typeof value !== 'string' || !/^(0|[1-9]\d{0,77})$/.test(value) || BigInt(value) > max)
    throw new Error('expected an exact unsigned decimal string');
  return value;
}
/** @param {unknown} value */
function address(value) {
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]{40}$/.test(value) || /^0x0{40}$/.test(value))
    throw new Error('invalid Set address');
  return getAddress(value);
}
/** @param {unknown} value */
function bytes32(value) {
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]{64}$/.test(value))
    throw new Error('invalid bytes32');
  return value.toLowerCase();
}

/** Matches sequencer settlement.rs: UUID bytes FIRST, then 16 zero bytes.
 * @param {unknown} uuid */
export function sequencerUuidToBytes32(uuid) {
  if (
    typeof uuid !== 'string' ||
    !/^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$/.test(uuid) ||
    uuid.replaceAll('-', '') === '0'.repeat(32)
  )
    throw new Error('invalid sequencer UUID');
  return `0x${uuid.replaceAll('-', '').toLowerCase()}${'0'.repeat(32)}`;
}

/** Exact Set contract typed data, not EIP-3009 or the sequencer Ed25519 envelope.
 * @param {{intent:SetIntent,payerNonce:string,validAfter?:string}} input */
export function buildSetPaymentAuthorization({ intent, payerNonce, validAfter = '0' }) {
  uint(intent.chainId);
  uint(intent.amount);
  uint(payerNonce, U64);
  uint(validAfter, U64);
  uint(intent.validUntil, U64);
  if (
    intent.chainId === '0' ||
    intent.amount === '0' ||
    BigInt(validAfter) >= BigInt(intent.validUntil)
  )
    throw new Error('invalid Set payment bounds');
  const domain = {
    name: 'SetPaymentBatch',
    version: '1',
    chainId: intent.chainId,
    verifyingContract: address(intent.settlementContract),
  };
  const types = {
    PaymentAuthorization: [
      { name: 'intentId', type: 'bytes32' },
      { name: 'payer', type: 'address' },
      { name: 'payee', type: 'address' },
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'nonce', type: 'uint64' },
      { name: 'validAfter', type: 'uint64' },
      { name: 'validBefore', type: 'uint64' },
    ],
  };
  const value = {
    intentId: bytes32(intent.intentId),
    payer: address(intent.payer),
    payee: address(intent.payee),
    token: address(intent.token),
    amount: intent.amount,
    nonce: payerNonce,
    validAfter,
    validBefore: intent.validUntil,
  };
  return { domain, types, value, digest: TypedDataEncoder.hash(domain, types, value) };
}

/** Single-payment profile. EOA payer signatures only; ERC-1271 needs chain verification.
 * @param {{intent:SetIntent,payerNonce:string,batch:SetBatch}} input */
export function buildSetBatchCalldata({ intent, payerNonce, batch }) {
  // SignatureChecker's EOA bytes-signature path requires canonical 65-byte
  // signatures (v=27/28); ethers also accepts compact/normalized alternatives.
  if (
    typeof batch.authorization !== 'string' ||
    !/^0x[0-9a-fA-F]{130}$/.test(batch.authorization) ||
    !['1b', '1c'].includes(batch.authorization.slice(-2).toLowerCase())
  )
    throw new Error('canonical 65-byte Set payer authorization required');
  const auth = buildSetPaymentAuthorization({ intent, payerNonce, validAfter: batch.validAfter });
  const signer = verifyTypedData(auth.domain, auth.types, auth.value, batch.authorization);
  if (signer !== auth.value.payer) throw new Error('Set payer authorization mismatch');
  /** @type {Array<'batchId'|'merkleRoot'|'tenantStoreKey'>} */
  const commitments = ['batchId', 'merkleRoot', 'tenantStoreKey'];
  for (const key of commitments) {
    bytes32(batch[key]);
    if (/^0x0{64}$/.test(batch[key])) throw new Error('empty batch commitment');
  }
  uint(batch.sequenceStart, U64);
  uint(batch.sequenceEnd, U64);
  if (batch.sequenceStart !== batch.sequenceEnd)
    throw new Error('single-payment sequence required');
  return iface.encodeFunctionData('settleBatch', [
    batch.batchId,
    batch.merkleRoot,
    batch.tenantStoreKey,
    batch.sequenceStart,
    batch.sequenceEnd,
    [
      [
        auth.value.intentId,
        auth.value.payer,
        auth.value.payee,
        auth.value.amount,
        auth.value.token,
        payerNonce,
        auth.value.validAfter,
        intent.validUntil,
        bytes32(batch.signingHash),
        batch.authorization,
      ],
    ],
  ]);
}

/** Concrete journal validateSigned callback. Enforces one Set call, exact fees /
 * nonce from the persisted operator plan, and independent operator gas ceilings.
 * @param {{relayer:string,maxGasLimit:string,maxFeePerGas:string,maxPriorityFeePerGas:string}} options
 * @returns {(input:import('../../../bindings/node/purchase-runtime').SetSubmissionPlan, rawTransaction:string) => Promise<string>}
 */
export function createSetTransactionVerifier({
  relayer,
  maxGasLimit,
  maxFeePerGas,
  maxPriorityFeePerGas,
}) {
  const sender = address(relayer);
  const limits = Object.fromEntries(
    Object.entries({ gasLimit: maxGasLimit, maxFeePerGas, maxPriorityFeePerGas }).map(
      ([key, value]) => [key, BigInt(uint(value))],
    ),
  );
  return async ({ intent, payerNonce, plan }, rawTransaction) => {
    if (
      typeof rawTransaction !== 'string' ||
      !/^0x(?:[0-9a-fA-F]{2})+$/.test(rawTransaction) ||
      rawTransaction.length > 2_000_002
    )
      throw new Error('raw signed transaction bytes required');
    if (!plan || typeof plan !== 'object')
      throw new Error('missing persisted Set transaction plan');
    const expectedPlan = /** @type {VerificationInput['plan']} */ (plan);
    if (!expectedPlan.batch || !expectedPlan.transaction)
      throw new Error('invalid persisted Set transaction plan');
    const data = buildSetBatchCalldata({ intent, payerNonce, batch: expectedPlan.batch });
    const tx = Transaction.from(rawTransaction);
    if (
      tx.type !== 2 ||
      !tx.isSigned() ||
      tx.from !== sender ||
      tx.to !== address(intent.settlementContract) ||
      tx.chainId !== BigInt(uint(intent.chainId)) ||
      tx.value !== 0n ||
      tx.data.toLowerCase() !== data.toLowerCase() ||
      tx.accessList?.length
    )
      throw new Error('signed transaction does not match the authorized Set call');
    if (BigInt(tx.nonce) !== BigInt(uint(expectedPlan.transaction.nonce)))
      throw new Error('relayer nonce mismatch');
    /** @type {Array<'gasLimit'|'maxFeePerGas'|'maxPriorityFeePerGas'>} */
    const gasFields = ['gasLimit', 'maxFeePerGas', 'maxPriorityFeePerGas'];
    for (const key of gasFields) {
      const expected = BigInt(uint(expectedPlan.transaction[key]));
      if (tx[key] !== expected || expected > limits[key])
        throw new Error(`unauthorized transaction ${key}`);
    }
    if (
      tx.gasLimit === 0n ||
      tx.maxPriorityFeePerGas === null ||
      tx.maxFeePerGas === null ||
      tx.maxPriorityFeePerGas > tx.maxFeePerGas
    )
      throw new Error('invalid transaction gas bounds');
    if (!tx.hash) throw new Error('missing signed transaction hash');
    return tx.hash;
  };
}
