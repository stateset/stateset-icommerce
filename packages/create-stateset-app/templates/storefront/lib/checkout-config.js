import { normalizeAddress } from './usdc-verification.js';

export function getCheckoutConfiguration(environment) {
  let rpc;
  try {
    rpc = new URL(environment.STATESET_BASE_RPC_URL);
  } catch {
    throw new Error('STATESET_BASE_RPC_URL must be an HTTP(S) URL');
  }
  if (!['http:', 'https:'].includes(rpc.protocol)) {
    throw new Error('STATESET_BASE_RPC_URL must be an HTTP(S) URL');
  }
  let merchantAddress;
  try {
    merchantAddress = normalizeAddress(environment.NEXT_PUBLIC_STORE_WALLET_ADDRESS);
    if (/^0x0{40}$/.test(merchantAddress)) throw new Error();
  } catch {
    throw new Error('NEXT_PUBLIC_STORE_WALLET_ADDRESS must be a non-zero EVM address');
  }
  const rawConfirmations = String(environment.STATESET_USDC_CONFIRMATIONS ?? '2');
  const minimumConfirmations = Number(rawConfirmations);
  if (!/^[1-9]\d*$/.test(rawConfirmations) || !Number.isSafeInteger(minimumConfirmations)) {
    throw new Error('STATESET_USDC_CONFIRMATIONS must be a positive safe integer');
  }
  return { rpcUrl: rpc.href, merchantAddress, minimumConfirmations };
}
