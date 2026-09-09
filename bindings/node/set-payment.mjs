import { createHash } from 'node:crypto';

// keccak256("PaymentSettled(bytes32,bytes32,address,address,uint256,address)")
// ABI: set/contracts/commerce/SetPaymentBatch.sol (not EIP-3009 Transfer).
export const SET_PAYMENT_SETTLED_TOPIC =
  '0x6012c68d92fcfe82072c5af63697de5bc68714d79533d164eeb0c41547d4e803';
const HASH = /^0x[0-9a-fA-F]{64}$/;
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const QUANTITY = /^0x(?:0|[1-9a-fA-F][0-9a-fA-F]*)$/;
function quantity(value) {
  if (typeof value !== 'string' || !QUANTITY.test(value)) throw new Error('invalid RPC quantity');
  return BigInt(value);
}
function address(value) {
  if (typeof value !== 'string' || !ADDRESS.test(value) || /^0x0{40}$/.test(value))
    throw new Error('invalid payment address');
  return value.toLowerCase();
}
function wordAddress(value) {
  if (!/^0{24}[0-9a-fA-F]{40}$/.test(value)) throw new Error('invalid ABI address');
  return address(`0x${value.slice(24)}`);
}

/** SetPaymentBatch USDC adapter. RPC, durable submission and transaction lookup
 * are operator-owned. Never supplies keys, grants allowances or selects a chain.
 * `submit` must persist the intent/key binding BEFORE signing or broadcasting.
 */
export function createSetPaymentAdapter({
  chainId,
  settlementContract,
  token,
  payer,
  rpc,
  submit,
  findTransaction,
  allowSubmit = false,
  intentEncoding = 'sha256-v1',
  getSequencerCapabilities,
}) {
  if (!['sha256-v1', 'sequencer-uuid-v1'].includes(intentEncoding))
    throw new Error('unsupported payment intent encoding');
  if (intentEncoding === 'sequencer-uuid-v1' && typeof getSequencerCapabilities !== 'function')
    throw new Error('sequencer profile requires operator capability lookup');
  if (typeof chainId !== 'string' || !/^[1-9]\d{0,19}$/.test(chainId))
    throw new Error('chainId must be an explicit decimal string');
  const contract = address(settlementContract);
  const tokenAddress = address(token);
  const payerAddress = address(payer);
  const asset = `eip155:${chainId}/erc20:${tokenAddress}`;
  for (const fn of [rpc, submit, findTransaction])
    if (typeof fn !== 'function') throw new Error('operator RPC, submit and lookup are required');

  function intent(context) {
    const quote = context?.operation?.quote;
    if (
      typeof context?.idempotencyKey !== 'string' ||
      !context.idempotencyKey ||
      quote?.asset !== asset ||
      typeof quote.amount !== 'string' ||
      !/^(0|[1-9]\d{0,19})(\.\d{1,18})?$/.test(quote.amount)
    )
      throw new Error('payment requires exact configured USDC asset and six-decimal amount');
    const [whole, fraction = ''] = quote.amount.split('.');
    if (/[1-9]/.test(fraction.slice(6))) throw new Error('USDC precision exceeds six decimals');
    const amount = BigInt(whole) * 1000000n + BigInt(fraction.slice(0, 6).padEnd(6, '0'));
    const expiry = Date.parse(quote.expiresAt);
    if (amount <= 0n || !Number.isFinite(expiry) || expiry < 0)
      throw new Error('invalid payment amount or expiry');
    const terms = {
      chainId,
      settlementContract: contract,
      payer: payerAddress,
      payee: address(quote.payee),
      token: tokenAddress,
      amount: amount.toString(),
      validUntil: Math.floor(expiry / 1000).toString(),
    };
    const digest = createHash('sha256')
      .update(JSON.stringify(['stateset.set-payment.v1', context.idempotencyKey, terms]))
      .digest();
    // Explicit opt-in only: changing this profile changes the signed identity.
    // UUID v8 (application-defined), RFC variant, then the sequencer's zero pad.
    if (intentEncoding === 'sequencer-uuid-v1') {
      digest[6] = (digest[6] & 0x0f) | 0x80;
      digest[8] = (digest[8] & 0x3f) | 0x80;
    }
    const intentId = `0x${
      intentEncoding === 'sequencer-uuid-v1'
        ? digest.subarray(0, 16).toString('hex') + '0'.repeat(32)
        : digest.toString('hex')
    }`;
    return { ...terms, intentId, idempotencyKey: context.idempotencyKey };
  }
  async function checkChain() {
    if (quantity(await rpc('eth_chainId', [])) !== BigInt(chainId))
      throw new Error('settlement RPC chain mismatch');
  }
  async function reconcile(context, expected, transactionHash) {
    await checkChain();
    if (transactionHash === null) return { status: 'unknown' };
    if (typeof transactionHash !== 'string' || !HASH.test(transactionHash))
      throw new Error('invalid settlement transaction reference');
    const receipt = await rpc('eth_getTransactionReceipt', [transactionHash]);
    if (receipt === null) return { status: 'pending' };
    if (
      !receipt ||
      receipt.transactionHash?.toLowerCase() !== transactionHash.toLowerCase() ||
      !HASH.test(receipt.blockHash) ||
      !Array.isArray(receipt.logs)
    )
      throw new Error('invalid settlement receipt');
    // A reverted/missing-event batch does not establish that the signed intent
    // cannot settle in another batch. Never release the hold on this evidence.
    if (receipt.status !== '0x1') return { status: 'unknown' };
    const height = quantity(receipt.blockNumber);
    const finalized = await rpc('eth_getBlockByNumber', ['finalized', false]);
    if (!finalized || quantity(finalized.number) < height) return { status: 'pending' };
    if (!HASH.test(finalized.hash)) throw new Error('invalid finalized block');
    if (
      quantity(finalized.number) === height &&
      finalized.hash.toLowerCase() !== receipt.blockHash.toLowerCase()
    )
      return { status: 'unknown' };
    const canonical = await rpc('eth_getBlockByNumber', [receipt.blockNumber, false]);
    if (
      !canonical ||
      quantity(canonical.number) !== height ||
      canonical.hash?.toLowerCase() !== receipt.blockHash.toLowerCase()
    )
      return { status: 'unknown' };
    const logs = receipt.logs.filter(
      (log) =>
        log.address?.toLowerCase() === contract &&
        log.topics?.[0]?.toLowerCase() === SET_PAYMENT_SETTLED_TOPIC &&
        log.topics?.[2]?.toLowerCase() === expected.intentId,
    );
    if (logs.length !== 1) return { status: 'unknown' };
    const log = logs[0];
    if (
      log.removed !== false ||
      log.topics.length !== 4 ||
      !HASH.test(log.topics[1]) ||
      !HASH.test(log.topics[3]) ||
      !/^0x[0-9a-fA-F]{192}$/.test(log.data) ||
      log.transactionHash?.toLowerCase() !== transactionHash.toLowerCase() ||
      log.blockHash?.toLowerCase() !== receipt.blockHash.toLowerCase() ||
      quantity(log.blockNumber) !== height
    )
      throw new Error('invalid payment settlement log');
    const words = log.data.slice(2).match(/.{64}/g);
    if (
      wordAddress(log.topics[3].slice(2)) !== expected.payer ||
      wordAddress(words[0]) !== expected.payee ||
      wordAddress(words[2]) !== expected.token ||
      BigInt(`0x${words[1]}`).toString() !== expected.amount
    )
      throw new Error('settled payment does not match authorized terms');
    quantity(log.logIndex);
    return {
      status: 'succeeded',
      evidence: {
        transaction_id: transactionHash.toLowerCase(),
        amount: context.operation.quote.amount,
        asset,
        payer: expected.payer,
        payee: expected.payee,
        intent_id: expected.intentId,
        settlement_contract: contract,
        batch_id: log.topics[1].toLowerCase(),
        block_hash: receipt.blockHash.toLowerCase(),
        block_number: height.toString(),
        log_index: log.logIndex,
        finality: 'rpc_finalized',
      },
    };
  }
  return Object.freeze({
    async execute(context) {
      if (allowSubmit !== true) throw new Error('settlement submission is disabled');
      context = structuredClone(context);
      const expected = intent(context);
      if (BigInt(expected.validUntil) <= BigInt(Math.floor(Date.now() / 1000)))
        throw new Error('payment quote has expired');
      await checkChain();
      if (intentEncoding === 'sequencer-uuid-v1') {
        const capabilities = await getSequencerCapabilities();
        if (
          !Array.isArray(capabilities?.features) ||
          !capabilities.features.includes('x402.client_intent_id.v1') ||
          capabilities.intent_id_encoding !== 'uuid-prefix-zero-pad-bytes32'
        )
          throw new Error('sequencer does not support pre-signed client intent identity');
      }
      const transactionHash = await submit(structuredClone(expected));
      return reconcile(context, expected, transactionHash);
    },
    async lookup(context) {
      context = structuredClone(context);
      const expected = intent(context);
      return reconcile(context, expected, await findTransaction(structuredClone(expected)));
    },
  });
}
