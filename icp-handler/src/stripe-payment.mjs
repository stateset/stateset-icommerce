// Read-only Stripe direct-charge reconciliation. No confirm/capture/refund API
// calls, client secrets, card details or caller-provided URLs are accepted.
export function createStripePaymentLookup({
  apiKey,
  accountId,
  apiVersion,
  connectedAccount = false,
  livemode = false,
  fetchImpl = globalThis.fetch,
}) {
  if (
    typeof apiKey !== 'string' ||
    !apiKey ||
    !/^acct_[A-Za-z0-9]+$/.test(accountId) ||
    typeof apiVersion !== 'string' ||
    !/^\d{4}-\d{2}-\d{2}(\.[a-z]+)?$/.test(apiVersion) ||
    typeof connectedAccount !== 'boolean' ||
    typeof livemode !== 'boolean' ||
    typeof fetchImpl !== 'function'
  )
    throw new Error('explicit Stripe credentials, account and API version required');
  const identity = `stripe:${accountId}:${livemode ? 'live' : 'test'}:usd-card-v1`;
  async function get(path) {
    const response = await fetchImpl(`https://api.stripe.com/v1/${path}`, {
      method: 'GET',
      redirect: 'error',
      signal: AbortSignal.timeout(15_000),
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Stripe-Version': apiVersion,
        ...(connectedAccount ? { 'Stripe-Account': accountId } : {}),
      },
    });
    // Do not copy provider bodies/errors: they can contain customer data and secrets.
    if (!response.ok) throw new Error('Stripe lookup unavailable');
    return response.json();
  }
  function cents(value) {
    if (typeof value !== 'string' || !/^(0|[1-9]\d{0,5})(\.\d{1,2})?$/.test(value))
      throw new Error('Stripe profile requires positive USD with at most two decimal places');
    const [whole, fraction = ''] = value.split('.');
    const minor = BigInt(whole) * 100n + BigInt(fraction.padEnd(2, '0'));
    if (minor <= 0n || minor > 99999999n) throw new Error('Stripe amount outside profile bounds');
    return minor;
  }
  const sameAmount = (value, expected) => Number.isSafeInteger(value) && BigInt(value) === expected;
  return Object.freeze({
    identity,
    async lookup(reference, terms) {
      if (
        typeof reference !== 'string' ||
        !/^pi_[A-Za-z0-9]+$/.test(reference) ||
        terms.currency !== 'USD' ||
        typeof terms.cartId !== 'string' ||
        !terms.cartId ||
        typeof terms.buyer !== 'string' ||
        !terms.buyer ||
        typeof terms.cartFingerprint !== 'string' ||
        !/^sha256:[0-9a-f]{64}$/.test(terms.cartFingerprint)
      )
        throw new Error('unsupported Stripe payment reference or currency');
      const expected = cents(terms.amount);
      const account = await get('account');
      if (account.object !== 'account' || account.id !== accountId)
        throw new Error('Stripe account mismatch');
      const intent = await get(`payment_intents/${reference}?expand[]=latest_charge`);
      if (
        intent.object !== 'payment_intent' ||
        intent.id !== reference ||
        intent.livemode !== livemode ||
        intent.currency !== 'usd' ||
        !sameAmount(intent.amount, expected) ||
        intent.metadata?.stateset_cart_id !== terms.cartId ||
        intent.metadata?.stateset_cart_fingerprint !== terms.cartFingerprint ||
        intent.metadata?.stateset_buyer !== terms.buyer ||
        intent.transfer_data !== null ||
        intent.on_behalf_of !== null
      )
        throw new Error('Stripe payment does not match merchant and quoted terms');
      if (intent.status !== 'succeeded') {
        if (
          [
            'requires_payment_method',
            'requires_confirmation',
            'requires_action',
            'processing',
            'requires_capture',
          ].includes(intent.status)
        )
          return { status: 'pending' };
        return { status: 'needs_attention' };
      }
      const charge = intent.latest_charge;
      if (
        !sameAmount(intent.amount_received, expected) ||
        intent.amount_capturable !== 0 ||
        !charge ||
        charge.object !== 'charge' ||
        !/^ch_[A-Za-z0-9]+$/.test(charge.id) ||
        charge.payment_intent !== reference ||
        charge.livemode !== livemode ||
        charge.currency !== 'usd' ||
        !sameAmount(charge.amount, expected) ||
        !sameAmount(charge.amount_captured, expected) ||
        charge.amount_refunded !== 0 ||
        charge.refunded !== false ||
        charge.disputed !== false ||
        charge.paid !== true ||
        charge.captured !== true ||
        charge.status !== 'succeeded' ||
        charge.payment_method_details?.type !== 'card' ||
        charge.transfer_data !== null ||
        charge.on_behalf_of !== null
      )
        throw new Error('Stripe capture is incomplete, reversed or outside this profile');
      return {
        status: 'succeeded',
        evidence: {
          provider: identity,
          reference,
          transactionId: charge.id,
          amount: terms.amount,
          currency: 'USD',
          cartId: terms.cartId,
          cartFingerprint: terms.cartFingerprint,
          buyer: terms.buyer,
        },
      };
    },
  });
}
