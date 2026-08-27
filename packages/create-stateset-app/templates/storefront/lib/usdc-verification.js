const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

export function normalizeAddress(value) {
  const address = String(value || '').toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(address)) throw new Error('Invalid EVM address');
  return address;
}

export function assertTransactionHash(value) {
  const hash = String(value || '').toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(hash)) throw new Error('Invalid transaction hash');
  return hash;
}

function addressTopic(address) {
  return `0x${normalizeAddress(address).slice(2).padStart(64, '0')}`;
}

export function verifyUsdcTransfer({
  receipt,
  currentBlock,
  tokenAddress,
  merchantAddress,
  payerAddress,
  expectedAmountUnits,
  minimumConfirmations = 2,
}) {
  if (
    !receipt ||
    (receipt.status !== 'success' && receipt.status !== '0x1' && receipt.status !== 1)
  ) {
    throw new Error('Transaction did not succeed');
  }
  const receiptBlock = BigInt(receipt.blockNumber);
  const confirmations = BigInt(currentBlock) - receiptBlock + 1n;
  if (confirmations < BigInt(minimumConfirmations)) {
    throw new Error(
      `Transaction needs ${minimumConfirmations} confirmations; found ${confirmations}`,
    );
  }
  const token = normalizeAddress(tokenAddress);
  const fromTopic = addressTopic(payerAddress);
  const toTopic = addressTopic(merchantAddress);
  const amount = BigInt(expectedAmountUnits);
  const match = (receipt.logs || []).find(
    (log) =>
      normalizeAddress(log.address) === token &&
      String(log.topics?.[0] || '').toLowerCase() === TRANSFER_TOPIC &&
      String(log.topics?.[1] || '').toLowerCase() === fromTopic &&
      String(log.topics?.[2] || '').toLowerCase() === toTopic &&
      BigInt(log.data) === amount,
  );
  if (!match) throw new Error('Transaction does not contain the expected USDC transfer');
  return { confirmations: Number(confirmations), logIndex: Number(match.logIndex) };
}
