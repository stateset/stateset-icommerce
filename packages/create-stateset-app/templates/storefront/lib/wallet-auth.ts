import { verifyMessage } from 'viem';

export const WALLET_AUTH_TTL_MS = 5 * 60 * 1000;

export function walletAuthMessage(address: string, timestamp: string) {
  return `{{STORE_NAME}} account access\nAddress: ${address.toLowerCase()}\nTimestamp: ${timestamp}`;
}

export async function verifyWalletRequest(request: Request, address: string) {
  const signature = request.headers.get('x-wallet-signature');
  const timestamp = request.headers.get('x-wallet-timestamp');
  if (!signature || !timestamp || !/^\d+$/.test(timestamp)) return false;
  if (Math.abs(Date.now() - Number(timestamp)) > WALLET_AUTH_TTL_MS) return false;
  try {
    return await verifyMessage({
      address: address as `0x${string}`,
      message: walletAuthMessage(address, timestamp),
      signature: signature as `0x${string}`,
    });
  } catch {
    return false;
  }
}
