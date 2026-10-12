import { canonicalJson, newId, newNonceHex, signEd25519 } from './codec.mjs';
import { amount, exactMoney } from './quote-money.mjs';
import { NativeMerchantCheckout } from './native-checkout.mjs';
import { NativeMerchantPayment } from './native-payment.mjs';
import { NativeMerchantFulfillment } from './native-fulfillment.mjs';
import * as state from './state.mjs';

let configured = null;
let locked = false;

// Operator configuration only. Call after configureStorage and before importing
// server.mjs. resolveCart must authorize the buyer's access to an existing cart;
// it must not create a cart or trust a caller-supplied cart ID without ownership checks.
export function configureNativeMerchant(options) {
  if (locked || configured) throw new Error('configure native merchant before server startup');
  if (!state.isDurable()) throw new Error('native HTTP checkout requires durable protocol storage');
  configured = new NativeHttpMerchant(options);
}

export function takeNativeMerchant() {
  locked = true;
  return configured;
}

function invalid(code, message) {
  return { ok: false, error: { type: 'icp.error', code, message } };
}

function quantities(items) {
  if (!Array.isArray(items) || !items.length) throw new Error('purchase items are required');
  const totals = new Map();
  for (const item of items) {
    if (
      typeof item.sku !== 'string' ||
      !item.sku ||
      !Number.isSafeInteger(item.quantity) ||
      item.quantity <= 0
    )
      throw new Error('positive integer quantities and SKUs are required');
    const total = (totals.get(item.sku) ?? 0) + item.quantity;
    if (!Number.isSafeInteger(total)) throw new Error('quantity overflow');
    totals.set(item.sku, total);
  }
  return canonicalJson([...totals].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
}

class NativeHttpMerchant {
  constructor({
    commerce,
    principal,
    policy,
    storeId,
    readReceipt,
    resolveCart,
    allowApply = false,
    payments = null,
    fulfillment = null,
  }) {
    if (typeof resolveCart !== 'function') throw new Error('operator cart resolver is required');
    this.commerce = commerce;
    this.resolveCart = resolveCart;
    this.fulfillment = fulfillment
      ? new NativeMerchantFulfillment({
          store: state,
          commerce,
          principal,
          policy,
          storeId,
          readReceipt,
          allowApply,
          resolveDispatch: fulfillment.resolveDispatch,
        })
      : null;
    this.payments = payments
      ? new NativeMerchantPayment({
          store: state,
          commerce,
          principal,
          policy,
          storeId,
          readReceipt,
          allowApply,
          provider: payments.provider,
          resolvePayment: payments.resolvePayment,
        })
      : null;
    this.checkout = new NativeMerchantCheckout({
      store: state,
      commerce,
      principal,
      policy,
      storeId,
      readReceipt,
      allowApply,
      stockPolicy: 'reject_if_insufficient',
      requireCartFingerprint: true,
      resolveQuote: (id) => {
        const record = state.getQuote(id);
        if (!record?.nativeCheckout) throw new Error('native quote not found');
        return record.nativeCheckout;
      },
    });
  }

  async quote(intent, signingKey, merchantAid) {
    if (intent.merchant !== merchantAid)
      return invalid('quote.merchant_mismatch', 'intent names another merchant');
    if (intent.from_proposal_id)
      return invalid('quote.proposal_unsupported', 'native checkout requires a current cart quote');
    await this.checkout.requireFeature('checkout.cart_fingerprint.v1');
    await this.checkout.requireFeature('checkout.stock_policy.v1');
    const binding = await this.resolveCart(structuredClone(intent));
    if (!binding || typeof binding.cartId !== 'string')
      return invalid('quote.cart_unavailable', 'no authorized cart for this intent');
    const { cart, fingerprint } = await this.commerce.checkoutSnapshot(binding.cartId);
    if (cart.id !== binding.cartId || !/^sha256:[0-9a-f]{64}$/.test(fingerprint))
      throw new Error('invalid native cart snapshot');
    try {
      if (quantities(intent.items) !== quantities(cart.items))
        return invalid('quote.items_mismatch', 'cart items do not match the purchase intent');
      if (
        cart.currency !== intent.max_total?.currency ||
        amount(cart.grand_total) > amount(intent.max_total.amount) ||
        amount(cart.grand_total) <= 0n
      )
        return invalid(
          'policy.quote.exceeds_max_total',
          'cart violates the purchase ceiling or currency',
        );
      const cap = intent.principal_binding?.authority?.max_per_intent;
      if (cap && (cap.currency !== cart.currency || amount(cart.grand_total) > amount(cap.amount)))
        return invalid(
          'delegation.amount_exceeded',
          'cart exceeds the delegated amount or currency',
        );
    } catch {
      return invalid('format.invalid_money', 'invalid purchase amounts or quantities');
    }
    if (!['active', 'ready_for_payment', 'payment_pending'].includes(cart.status))
      return invalid('quote.cart_unavailable', 'cart is no longer open');
    const now = Date.now();
    const expiry = Math.min(
      now + 300_000,
      Date.parse(intent.exp),
      cart.expires_at ? Date.parse(cart.expires_at) : Infinity,
      intent.principal_binding ? Date.parse(intent.principal_binding.expiry) : Infinity,
    );
    if (!Number.isFinite(expiry) || expiry <= now)
      return invalid('replay.expired', 'intent or cart has expired');
    const quote = {
      type: 'quote',
      v: 'icp-1.0',
      quote_id: newId('icp_qt'),
      intent_id: intent.intent_id,
      merchant: merchantAid,
      total: { amount: exactMoney(amount(cart.grand_total)), currency: cart.currency },
      lines: cart.items.map((item) => ({
        sku: item.sku,
        quantity: item.quantity,
        unit_price: { amount: exactMoney(amount(item.unit_price)), currency: cart.currency },
        line_total: { amount: exactMoney(amount(item.total)), currency: cart.currency },
      })),
      settler: intent.settler,
      escrow_terms: { settlement: 'unconfigured' },
      cart_fingerprint: fingerprint,
      expiry: new Date(expiry).toISOString(),
      nonce: newNonceHex(),
      iat: new Date(now).toISOString(),
      exp: new Date(expiry).toISOString(),
    };
    return {
      ok: true,
      quote,
      signatureHex: signEd25519(canonicalJson(quote), signingKey),
      nativeCheckout: {
        quoteId: quote.quote_id,
        cartId: cart.id,
        cartFingerprint: fingerprint,
        amount: quote.total.amount,
        currency: cart.currency,
        expiresAt: quote.exp,
        budgetId: binding.budgetId ?? null,
      },
    };
  }

  accept(quoteId) {
    // The immutable quote identity, never a caller's retry key, defines the
    // economic operation. A new acceptance signature cannot allocate twice.
    return this.checkout.accept({ quoteId, idempotencyKey: `icp-http:${quoteId}` });
  }

  reconcilePayment(record, orderId) {
    if (!this.payments) return null;
    const buyer = state.getIntent(record.intentId)?.intent.buyer;
    return this.payments.reconcile({
      quoteId: record.quote.quote_id,
      orderId,
      buyer,
      cartId: record.nativeCheckout.cartId,
      cartFingerprint: record.nativeCheckout.cartFingerprint,
      amount: record.quote.total.amount,
      currency: record.quote.total.currency,
    });
  }

  reconcileFulfillment(record, orderId) {
    if (!this.fulfillment) return null;
    return this.fulfillment.reconcile({
      quoteId: record.quote.quote_id,
      orderId,
      buyer: state.getIntent(record.intentId)?.intent.buyer,
      amount: record.quote.total.amount,
      currency: record.quote.total.currency,
    });
  }
}
