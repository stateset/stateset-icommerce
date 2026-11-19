// Read on the server only. Return browser configuration through publicCardAccount.
export function cardAccounts(env) {
  const input = JSON.parse(env.STATESET_CARD_ACCOUNTS_JSON || '[]');
  const seen = new Set();
  if (!Array.isArray(input) || !input.length)
    throw new Error('Configure at least one card processor account');
  return input.map((entry) => {
    const {
      account,
      provider,
      secretEnv,
      publicKey,
      location,
      sandbox = true,
      currencies,
      methods = ['credit_card'],
      available = true,
    } = entry;
    if (
      !/^[a-z0-9][a-z0-9_-]{0,47}$/.test(account || '') ||
      seen.has(account) ||
      typeof secretEnv !== 'string' ||
      !/^[A-Z][A-Z0-9_]+$/.test(secretEnv) ||
      !env[secretEnv] ||
      !Array.isArray(currencies) ||
      !currencies.length ||
      !currencies.every((code) => /^[A-Z]{3}$/.test(code)) ||
      !Array.isArray(methods) ||
      !methods.length ||
      !methods.every((method) => ['credit_card', 'debit_card'].includes(method)) ||
      typeof available !== 'boolean' ||
      typeof sandbox !== 'boolean'
    )
      throw new Error('Invalid card account configuration');
    if (provider === 'stripe') {
      if (
        !/^pk_(test|live)_[a-zA-Z0-9]+$/.test(publicKey) ||
        !/^sk_(test|live)_[a-zA-Z0-9]+$/.test(env[secretEnv]) ||
        publicKey.split('_')[1] !== env[secretEnv].split('_')[1]
      )
        throw new Error('Configure matching Stripe account keys');
    } else if (
      provider !== 'square' ||
      !/^(sandbox-)?sq0idp-[a-zA-Z0-9_-]+$/.test(publicKey || '') ||
      !/^[a-zA-Z0-9_-]+$/.test(location || '') ||
      publicKey.startsWith('sandbox-') !== sandbox
    ) {
      throw new Error('Configure the Square application ID, location and environment');
    }
    seen.add(account);
    return {
      account,
      provider,
      publicKey,
      secret: env[secretEnv],
      location,
      sandbox,
      currencies,
      methods,
      available,
    };
  });
}

export function publicCardAccount({ account, provider, publicKey, location, sandbox }) {
  return { account, provider, publicKey, ...(provider === 'square' && { location, sandbox }) };
}

// Provider account lookups authenticate configuration before a separate test sale.
export async function verifyCardAccount(account, fetcher = fetch) {
  if (!['stripe', 'square'].includes(account.provider)) throw new Error('Unknown card processor');
  const square = account.provider === 'square';
  const origin = account.sandbox
    ? 'https://connect.squareupsandbox.com'
    : 'https://connect.squareup.com';
  const url = square
    ? origin + '/v2/locations/' + encodeURIComponent(account.location)
    : 'https://api.stripe.com/v1/account';
  const headers = {
    Authorization: 'Bearer ' + account.secret,
    ...(square && { 'Square-Version': '2025-10-16' }),
  };
  const response = await fetcher(url, {
    headers,
    redirect: 'error',
    cache: 'no-store',
    signal: AbortSignal.timeout(12000),
  });
  if (!response.ok)
    throw new Error(account.provider + ' account lookup returned HTTP ' + response.status);
  const body = await response.json();
  const mode = square
    ? account.sandbox
      ? 'sandbox'
      : 'live'
    : account.secret.startsWith('sk_test_')
      ? 'sandbox'
      : 'live';
  const common = {
    account: account.account,
    provider: account.provider,
    mode,
    evidence: 'provider',
    settlementVerified: false,
  };
  if (!square) {
    if (!/^acct_[A-Za-z0-9]+$/.test(body.id || '') || typeof body.charges_enabled !== 'boolean')
      throw new Error('Stripe returned an incomplete account');
    return { ...common, identity: body.id, chargesEnabled: body.charges_enabled };
  }
  const location = body.location;
  if (!location || location.id !== account.location || !location.merchant_id || !location.currency)
    throw new Error('Square returned a different or incomplete merchant location');
  return {
    ...common,
    identity: location.merchant_id + '/' + location.id,
    currency: location.currency,
    chargesEnabled:
      location.status === 'ACTIVE' &&
      Array.isArray(location.capabilities) &&
      location.capabilities.includes('CREDIT_CARD_PROCESSING'),
  };
}
