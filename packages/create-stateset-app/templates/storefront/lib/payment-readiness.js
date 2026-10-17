import { normalizeAddress } from './usdc-verification.js';

export async function assertCheckoutAvailable({ merchantAddress, chainId, fetchImpl = fetch }) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetchImpl('/api/store/status', {
      cache: 'no-store',
      signal: controller.signal,
    });
    if (!response.ok) throw new Error();
    const status = await response.json();
    const merchant = normalizeAddress(merchantAddress);
    if (
      status.checkoutAvailable !== true ||
      status.chainId !== chainId ||
      /^0x0{40}$/.test(merchant) ||
      normalizeAddress(status.merchantAddress) !== merchant
    )
      throw new Error();
  } catch {
    throw new Error(
      'Checkout is temporarily unavailable. Please try again later or contact the store.',
    );
  } finally {
    clearTimeout(timer);
  }
}
