export function cardCheckoutEnabled(env) {
  return env.STATESET_CHECKOUT_PROVIDER === 'stripe';
}

export function getStripeConfiguration(env) {
  if (!cardCheckoutEnabled(env) && env.STATESET_CHECKOUT_PROVIDER !== 'routed')
    throw new Error('Card checkout is disabled');
  if (!/^whsec_[A-Za-z0-9]+$/.test(env.STRIPE_WEBHOOK_SECRET || ''))
    throw new Error('Configure the Stripe secret key and checkout webhook signing secret');
  return { ...getStripePlatformConfiguration(env), webhookSecret: env.STRIPE_WEBHOOK_SECRET };
}

export function getStripePlatformConfiguration(env) {
  if (!/^sk_(test|live)_[A-Za-z0-9]+$/.test(env.STRIPE_SECRET_KEY || ''))
    throw new Error('Configure the Stripe platform secret key');
  const origin = new URL(env.NEXT_PUBLIC_BASE_URL || 'http://localhost:3000');
  const local = ['localhost', '127.0.0.1', '[::1]'].includes(origin.hostname);
  if (
    origin.username ||
    origin.password ||
    origin.pathname !== '/' ||
    origin.search ||
    origin.hash ||
    (origin.protocol !== 'https:' && !(origin.protocol === 'http:' && local))
  ) {
    throw new Error('Configure a public HTTPS storefront origin or localhost for testing');
  }
  return {
    key: env.STRIPE_SECRET_KEY,
    origin: origin.origin,
  };
}
