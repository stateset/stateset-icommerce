import { RateLimiter } from '../channels/rate-limiter.js';

/** Bounded admission state: one public bucket and one bucket per configured account. */
export function createMuseRequestControls({
  accountIds,
  requestsPerMinute = 60,
  publicRequestsPerMinute = 120,
  maxInFlight = 16,
  maxAccountInFlight = 4,
  windowMs = 60000,
}) {
  for (const [name, value] of Object.entries({
    requestsPerMinute,
    publicRequestsPerMinute,
    maxInFlight,
    maxAccountInFlight,
    windowMs,
  })) {
    if (!Number.isSafeInteger(value) || value < 1)
      throw new Error(`${name} must be a positive integer.`);
  }
  const accounts = new Map([...accountIds].map((id) => [id, 0]));
  const accountRates = new RateLimiter({ maxRequests: requestsPerMinute, windowMs });
  const publicRate = new RateLimiter({ maxRequests: publicRequestsPerMinute, windowMs });
  let inFlight = 0;
  return {
    checkRate(accountId) {
      if (accountId !== undefined && !accounts.has(accountId)) throw new Error('Unknown account.');
      return accountId === undefined ? publicRate.check('public') : accountRates.check(accountId);
    },
    acquire(accountId) {
      if (!accounts.has(accountId)) throw new Error('Unknown account.');
      if (inFlight >= maxInFlight || accounts.get(accountId) >= maxAccountInFlight) return null;
      inFlight++;
      accounts.set(accountId, accounts.get(accountId) + 1);
      let released = false;
      return () => {
        if (released) return;
        released = true;
        inFlight--;
        accounts.set(accountId, accounts.get(accountId) - 1);
      };
    },
    close() {
      accountRates.destroy();
      publicRate.destroy();
    },
  };
}
