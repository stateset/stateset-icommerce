import {
  claimPaymentForOrder,
  findPaymentByIdempotencyKey,
  finishRecordedCheckout,
} from './checkout-finalization.js';
import { getTaxProvider } from './tax.js';
import { addDecimals } from './money.js';
import { getShippingProvider, validateShippingAddress } from './shipping.js';

export class CheckoutError extends Error {
  constructor(message, status = 400) {
    super(message);
    this.name = 'CheckoutError';
    this.status = status;
  }
}

export function normalizeCheckoutEmail(value) {
  const email = String(value || '')
    .trim()
    .toLowerCase();
  if (!email || email.length > 254) {
    throw new CheckoutError('A valid email is required');
  }

  const at = email.indexOf('@');
  if (at <= 0 || at !== email.lastIndexOf('@')) {
    throw new CheckoutError('A valid email is required');
  }

  const domainStart = at + 1;
  const dot = email.indexOf('.', domainStart + 1);
  if (dot < domainStart + 1 || dot === email.length - 1) {
    throw new CheckoutError('A valid email is required');
  }

  for (const character of email) {
    if (character.trim() === '') {
      throw new CheckoutError('A valid email is required');
    }
  }

  return email;
}

async function resolveCustomer(commerce, email, payerAddress, firstName, lastName) {
  const findOwner = async () => {
    const customers = await commerce.customers.list();
    const walletOwner = customers.find(
      (candidate) => candidate.metadata?.walletAddress?.toLowerCase() === payerAddress,
    );
    if (walletOwner) return walletOwner;
    const emailOwner = customers.find((candidate) => candidate.email?.toLowerCase() === email);
    if (emailOwner) {
      throw new CheckoutError(
        'That email belongs to a different account; sign in to link a new wallet',
        409,
      );
    }
    return null;
  };

  const existing = await findOwner();
  if (existing) return existing;
  try {
    return await commerce.customers.create({
      email,
      firstName,
      lastName,
      metadata: { walletAddress: payerAddress },
    });
  } catch (error) {
    // A concurrent checkout may have created this customer after our read.
    const winner = await findOwner();
    if (winner) return winner;
    throw error;
  }
}

export async function executeCheckout({
  commerce,
  cartId,
  email,
  txHash,
  payerAddress,
  shippingAddress,
  shippingMethodId,
  verifySettlement,
  taxProvider = getTaxProvider(),
  shippingProvider = getShippingProvider(),
}) {
  if (!cartId || !txHash || !payerAddress || !shippingAddress) {
    throw new CheckoutError(
      'cartId, email, txHash, walletAddress, and shippingAddress are required',
    );
  }
  const normalizedEmail = normalizeCheckoutEmail(email);
  let normalizedAddress;
  try {
    normalizedAddress = validateShippingAddress(shippingAddress);
  } catch (error) {
    throw new CheckoutError(
      error instanceof Error ? error.message : 'Invalid shipping address',
      422,
    );
  }
  if (!taxProvider.hasJurisdiction(normalizedAddress.state)) {
    throw new CheckoutError(`Tax is not configured for ${normalizedAddress.state}`, 422);
  }

  const [cart, items] = await Promise.all([
    commerce.carts.get(cartId),
    commerce.carts.getItems(cartId),
  ]);
  if (!cart) throw new CheckoutError('Cart not found', 404);
  if (!items?.length) throw new CheckoutError('Cart is empty');
  const pricedItems = items.map((item) => ({
    productId: item.productId,
    variantId: item.variantId,
    sku: item.sku,
    name: item.name || item.sku,
    quantity: item.quantity,
    unitPrice: item.unitPriceExact || String(item.unitPrice),
  }));
  const itemTotals = taxProvider.calculateCart(pricedItems, normalizedAddress.state);
  let shippingQuote;
  try {
    shippingQuote = shippingProvider.quote(normalizedAddress, shippingMethodId);
  } catch (error) {
    throw new CheckoutError(
      error instanceof Error ? error.message : 'Invalid shipping method',
      422,
    );
  }
  const totals = {
    ...itemTotals,
    shipping: shippingQuote.amount,
    total: addDecimals([itemTotals.total, shippingQuote.amount]),
  };
  const settlement = await verifySettlement({ totals });
  const idempotencyKey = `base:${txHash}:${settlement.logIndex}`;
  const existingPayment = await findPaymentByIdempotencyKey(commerce, idempotencyKey);
  if (existingPayment) {
    if (
      existingPayment.amountExact !== totals.total ||
      String(existingPayment.currency).toUpperCase() !== 'USDC'
    ) {
      throw new CheckoutError('Settlement was already used for another checkout', 409);
    }
    const existingOrder = await finishRecordedCheckout(commerce, existingPayment, cartId);
    return {
      orderId: existingOrder.id,
      orderNumber: existingOrder.orderNumber,
      status: existingOrder.status,
      replayed: true,
      confirmations: settlement.confirmations,
    };
  }
  if (String(cart.status).toLowerCase() !== 'active') {
    throw new CheckoutError('Cart is not active', 409);
  }

  const customer = await resolveCustomer(
    commerce,
    normalizedEmail,
    payerAddress,
    normalizedAddress.firstName,
    normalizedAddress.lastName,
  );
  const order = await commerce.orders.createExact({
    customerId: customer.id,
    cartId,
    currency: 'USDC',
    stockPolicy: 'reject_if_insufficient',
    shippingMethod: shippingQuote.id,
    shippingAddress: {
      line1: normalizedAddress.line1,
      line2: normalizedAddress.line2,
      city: normalizedAddress.city,
      state: normalizedAddress.state,
      postalCode: normalizedAddress.postalCode,
      country: normalizedAddress.country,
    },
    notes: `Verified Base settlement: ${txHash}; Cart: ${cartId}`,
    items: [
      ...totals.lines.map((item) => ({
        productId: item.productId,
        variantId: item.variantId,
        sku: item.sku,
        name: item.name,
        quantity: item.quantity,
        unitPrice: item.unitPrice,
        taxAmount: item.tax,
      })),
      ...(shippingQuote.amount === '0'
        ? []
        : [
            {
              sku: `SHIPPING-${shippingQuote.id.toUpperCase()}`,
              name: shippingQuote.label,
              quantity: 1,
              unitPrice: shippingQuote.amount,
              taxAmount: '0',
            },
          ]),
    ],
  });
  const claim = await claimPaymentForOrder({
    commerce,
    order,
    paymentInput: {
      orderId: order.id,
      customerId: customer.id,
      idempotencyKey,
      amount: totals.total,
      currency: 'USDC',
      paymentMethod: 'crypto',
    },
  });
  if (claim.conflict) {
    throw new CheckoutError('Settlement was already used for another checkout', 409);
  }
  const confirmed = await finishRecordedCheckout(commerce, claim.payment, cartId);
  return {
    orderId: confirmed.id,
    orderNumber: confirmed.orderNumber,
    status: confirmed.status,
    confirmations: settlement.confirmations,
  };
}
