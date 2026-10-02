/**
 * Explain Tools Module
 *
 * Read-only tools that let an agent tell a customer or merchant WHY:
 *
 *  - `explain_order` assembles one chronological story of an order from the
 *    read APIs of every domain that touched it (checkout cart, payments,
 *    shipments, returns, fraud, activity log, tax), sums the money exactly,
 *    and flags inconsistencies between them.
 *  - `explain_cart_pricing` shows why a cart costs what it costs: lines,
 *    promotions applied and refused (with reason codes), tax by jurisdiction,
 *    shipping, and a reconciliation of the explained total against the stored
 *    grand total.
 *
 * Nothing here writes: `tax.calculate` and `promotions.apply` evaluate without
 * persisting. Every section degrades on its own -- a domain that is missing or
 * fails is omitted and named in `unavailable`, and a valid order or cart is
 * always explained. Money is exact decimal strings throughout.
 */

import { z } from 'zod';

import {
  addDecimals,
  clampAtZero,
  compareDecimals,
  formatDecimal,
  multiplyDecimals,
  roundDecimal,
  subtractDecimals,
} from '../utils/exact-decimal.js';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Page size and bound for resolving an order number (there is no getByNumber). */
const ORDER_SCAN_PAGE = 200;
const ORDER_SCAN_MAX = 5000;

/** Payment statuses whose amount was captured from the customer. */
const CAPTURED_PAYMENT_STATUSES = new Set([
  'completed',
  'refunded',
  'partially_refunded',
  'disputed',
]);
/** Payment statuses still in flight. */
const PENDING_PAYMENT_STATUSES = new Set(['pending', 'processing', 'requires_action']);
/** Shipment statuses that mean the parcel has left. */
const SHIPPED_SHIPMENT_STATUSES = new Set([
  'shipped',
  'in_transit',
  'out_for_delivery',
  'delivered',
]);

/**
 * Information these tools cannot read because the Node binding has no API for
 * it. Reported on every explanation so the reader knows what is NOT in it.
 */
export const EXPLAIN_ORDER_API_GAPS = Object.freeze([
  {
    topic: 'refunds',
    reason:
      'Refund records could not be read from the available payment binding, so refund amounts ' +
      'are inferred only from a fully refunded payment status when possible.',
    missingApi: 'payments.getRefunds(paymentId) / PaymentOutput.amountRefunded',
  },
  {
    topic: 'promotion_usage',
    reason:
      'Promotion usage rows (which promotion/coupon discounted which order, and by how much) have ' +
      'no read API; only promotions.recordUsage writes them. The discount shown is the one stored ' +
      'on the checkout cart.',
    missingApi: 'promotions.listUsage({ orderId })',
  },
  {
    topic: 'order_tax',
    reason:
      'Orders carry no tax, discount or shipping fields and no per-line tax, and stored ' +
      'tax_calculations rows have no read API. Tax comes from the checkout cart and a fresh ' +
      'recomputation (labelled recomputed).',
    missingApi: 'OrderOutput.taxAmountExact / per-line tax, tax.getCalculation({ orderId })',
  },
  {
    topic: 'kernel_receipts',
    reason:
      'Kernel receipts and kernel outbox events are stored but have no read API, so governed ' +
      'commands (checkout.commit, payments.create_refund, ...) cannot be cited by receipt.',
    missingApi:
      'a kernel receipt / outbox read keyed by aggregate id (e.g. kernelReceipts({ aggregateId }))',
  },
  {
    topic: 'status_history',
    reason:
      'Order, payment, shipment and return status transitions are not recorded with timestamps; ' +
      'only the creation time and the current status (with the row updatedAt) are readable.',
    missingApi: 'status transition history per aggregate',
  },
]);

export const EXPLAIN_CART_API_GAPS = Object.freeze([
  {
    topic: 'promotion_evaluation_at_checkout',
    reason:
      'Promotions are re-evaluated now; the evaluation the cart was priced with is not stored. ' +
      "For a completed cart the customer's history has changed since (e.g. first-order offers " +
      'now refuse), so applied/rejected may differ from checkout time.',
    missingApi: 'stored promotion evaluation per cart',
  },
  {
    topic: 'stored_tax_calculation',
    reason:
      'Tax is recomputed now from the cart address; the stored calculation behind the cart tax ' +
      'has no read API.',
    missingApi: 'tax.getCalculation({ cartId })',
  },
]);

/** Human-readable reason for an unexpected read failure. */
function describeError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Run one section's reads. A failure (or a missing domain) is recorded in
 * `unavailable` and yields `fallback`, so one broken domain never sinks the
 * whole explanation.
 */
async function readSection(unavailable, section, read, fallback = null) {
  try {
    return await read();
  } catch (error) {
    unavailable.push({ topic: section, reason: `read failed: ${describeError(error)}` });
    return fallback;
  }
}

/**
 * Sort key for ISO timestamps of mixed precision and offsets
 * ("...03.041136338+00:00", "...03.299Z"): epoch nanoseconds as a BigInt.
 */
export function timeKey(iso) {
  if (typeof iso !== 'string') return null;
  const match = /^(.*T\d{2}:\d{2}:\d{2})(?:\.(\d+))?(Z|[+-]\d{2}:?\d{2})?$/.exec(iso.trim());
  if (!match) {
    const ms = Date.parse(iso);
    return Number.isNaN(ms) ? null : BigInt(ms) * 1_000_000n;
  }
  const [, base, fraction = '', zone = 'Z'] = match;
  const ms = Date.parse(`${base}${zone}`);
  if (Number.isNaN(ms)) return null;
  return BigInt(ms) * 1_000_000n + BigInt((fraction + '000000000').slice(0, 9));
}

/** Stable chronological sort; entries without a readable time go last. */
function sortTimeline(entries) {
  return entries
    .map((entry, index) => ({ entry, index, key: timeKey(entry.at) }))
    .sort((x, y) => {
      if (x.key === null || y.key === null) {
        if (x.key === y.key) return x.index - y.index;
        return x.key === null ? 1 : -1;
      }
      if (x.key !== y.key) return x.key < y.key ? -1 : 1;
      return x.index - y.index;
    })
    .map(({ entry }) => entry);
}

/** `a` and `b` are both present and differ. */
function differs(a, b) {
  return (
    a !== null && a !== undefined && b !== null && b !== undefined && compareDecimals(a, b) !== 0
  );
}

/** Map a stored cart/order address to the tax engine's address input. */
function taxAddress(address) {
  return {
    line1: address.line1,
    line2: address.line2,
    city: address.city,
    state: address.state,
    postalCode: address.postalCode,
    country: address.country,
  };
}

/**
 * Tax exactly as `calculate_cart_tax` computes it (standard category, each
 * line's own discount, the cart's shipping), without writing it back.
 */
async function recomputeCartTax(commerce, cart, items) {
  const result = await commerce.tax.calculate({
    lineItems: items.map((item) => ({
      id: item.id,
      sku: item.sku,
      productId: item.productId,
      quantity: item.quantity,
      unitPrice: Number(item.unitPriceExact),
      discountAmount: Number(item.discountAmountExact),
      taxCategory: 'standard',
      description: item.name,
    })),
    shippingAddress: taxAddress(cart.shippingAddress),
    customerId: cart.customerId,
    currency: cart.currency,
    shippingAmount: Number(cart.shippingAmountExact),
  });
  return formatTax(result);
}

/** Shape a TaxCalculationOutput for an explanation. */
function formatTax(result) {
  return {
    totalTax: formatDecimal(result.totalTaxExact),
    shippingTax: formatDecimal(result.shippingTaxExact),
    taxableSubtotal: formatDecimal(result.subtotalExact),
    isEstimate: result.isEstimate,
    exemptionsApplied: result.exemptionsApplied,
    exemptionDetails: result.exemptionDetails ?? null,
    breakdown: (result.taxBreakdown ?? []).map((row) => ({
      jurisdictionId: row.jurisdictionId,
      jurisdiction: row.jurisdictionName,
      taxType: row.taxType,
      rateName: row.rateName,
      rate: row.rate,
      taxableAmount: formatDecimal(row.taxableAmountExact),
      taxAmount: formatDecimal(row.taxAmountExact),
      isCompound: row.isCompound,
    })),
    jurisdictions: (result.jurisdictions ?? []).map((j) => ({
      id: j.id,
      name: j.name,
      code: j.code,
      level: j.level,
      totalRate: j.totalRate,
      totalTax: formatDecimal(j.totalTaxExact),
    })),
    lines: (result.lineItemTaxes ?? []).map((line) => ({
      lineItemId: line.lineItemId,
      taxableAmount: formatDecimal(line.taxableAmountExact),
      taxAmount: formatDecimal(line.taxAmountExact),
      effectiveRate: line.effectiveRate,
      isExempt: line.isExempt,
      exemptionReason: line.exemptionReason ?? null,
    })),
  };
}

// ---------------------------------------------------------------------------
// explain_order
// ---------------------------------------------------------------------------

/** Find an order by UUID or order number. */
async function resolveOrder(commerce, identifier) {
  if (UUID.test(identifier)) {
    const order = await commerce.orders.get(identifier);
    return order ? { order, resolvedBy: 'id' } : null;
  }
  // No orders.getByNumber on the binding: scan pages, bounded.
  for (let offset = 0; offset < ORDER_SCAN_MAX; offset += ORDER_SCAN_PAGE) {
    const page = await commerce.orders.list({ limit: ORDER_SCAN_PAGE, offset });
    const hit = page.find((o) => o.orderNumber === identifier || o.id === identifier);
    if (hit) return { order: hit, resolvedBy: 'order_number_scan' };
    if (page.length < ORDER_SCAN_PAGE) break;
  }
  return null;
}

/** The cart checkout turned into this order, when the order has a customer. */
async function findCheckoutCart(commerce, order) {
  if (!order.customerId) return null;
  const carts = await commerce.carts.forCustomer(order.customerId);
  return carts.find((cart) => cart.orderId === order.id) ?? null;
}

function summarizeOrder(order) {
  return {
    id: order.id,
    orderNumber: order.orderNumber,
    customerId: order.customerId,
    status: order.status,
    paymentStatus: order.paymentStatus,
    fulfillmentStatus: order.fulfillmentStatus,
    currency: order.currency,
    total: formatDecimal(order.totalAmountExact),
    trackingNumber: order.trackingNumber ?? null,
    shippingMethod: order.shippingMethod ?? null,
    shippingAddress: order.shippingAddress ?? null,
    items: (order.items ?? []).map((item) => ({
      id: item.id,
      sku: item.sku,
      name: item.name,
      quantity: item.quantity,
      unitPrice: formatDecimal(item.unitPriceExact),
      total: formatDecimal(item.totalExact),
    })),
    createdAt: order.createdAt,
    updatedAt: order.updatedAt,
  };
}

/** Build the order's money summary and the flags it implies. */
function explainOrderMoney({
  order,
  cart,
  payments,
  refunds = [],
  refundsReadable = false,
  recomputedTax,
}) {
  const currency = order.currency;
  const orderTotal = formatDecimal(order.totalAmountExact);
  const itemsSubtotal = addDecimals((order.items ?? []).map((i) => i.totalExact));
  const captured = payments.filter((p) => CAPTURED_PAYMENT_STATUSES.has(p.status));
  const pending = payments.filter((p) => PENDING_PAYMENT_STATUSES.has(p.status));
  const charged = addDecimals(captured.map((p) => p.amountExact));
  const partiallyRefunded = payments.filter((p) => p.status === 'partially_refunded');
  const fullyRefunded = payments.filter((p) => p.status === 'refunded');
  // Refund records include pending requests as well as completed refunds. A
  // pending request is still money reserved for the customer, so include it
  // in the exact refund total while preserving its status in the timeline.
  const readableRefunds = refunds.filter((refund) => refund.status !== 'failed');
  const refunded = refundsReadable
    ? addDecimals(readableRefunds.map((refund) => refund.amountExact))
    : partiallyRefunded.length > 0
      ? null
      : addDecimals(fullyRefunded.map((p) => p.amountExact));

  const money = {
    currency,
    orderTotal,
    itemsSubtotal,
    discount: cart ? formatDecimal(cart.discountAmountExact) : null,
    tax: cart ? formatDecimal(cart.taxAmountExact) : null,
    shipping: cart ? formatDecimal(cart.shippingAmountExact) : null,
    couponCode: cart?.couponCode ?? null,
    charged,
    pendingCharges: addDecimals(pending.map((p) => p.amountExact)),
    refunded,
    net: refunded === null ? null : subtractDecimals(charged, refunded),
    recomputedTax: recomputedTax?.totalTax ?? null,
    basis: {
      discount: cart ? 'checkout_cart' : 'unavailable',
      tax: cart ? 'checkout_cart' : 'unavailable',
      shipping: cart ? 'checkout_cart' : 'unavailable',
      charged:
        'payments with a captured status (completed, refunded, partially_refunded, disputed)',
      refunded: refundsReadable
        ? 'payment refund records (pending and completed, excluding failed)'
        : refunded === null
          ? 'unavailable: a payment is partially_refunded and refund amounts are not readable'
          : "payment status only: fully 'refunded' payments; pending refunds are not readable",
      recomputedTax: recomputedTax ? 'recomputed_now' : 'unavailable',
    },
  };

  const flags = [];
  const flag = (code, severity, message, detail = undefined) =>
    flags.push(detail ? { code, severity, message, detail } : { code, severity, message });

  if (compareDecimals(charged, orderTotal) > 0) {
    flag(
      'over_captured',
      'error',
      `Captured ${charged} ${currency} exceeds the order total ${orderTotal}.`,
    );
  }
  if (refunded !== null && compareDecimals(refunded, charged) > 0) {
    flag('refunded_exceeds_charged', 'error', `Refunded ${refunded} exceeds charged ${charged}.`);
  }
  if (
    captured.length > 0 &&
    compareDecimals(charged, orderTotal) >= 0 &&
    ['pending', 'authorized', 'partially_paid', 'failed'].includes(order.paymentStatus)
  ) {
    flag(
      'order_payment_status_stale',
      'warning',
      `Payments captured ${charged} ${currency} (the full total) but the order paymentStatus is '${order.paymentStatus}'.`,
    );
  }
  if (cart && differs(cart.grandTotalExact, order.totalAmountExact)) {
    flag(
      'order_total_differs_from_cart',
      'error',
      `The order total ${orderTotal} differs from the checkout cart grand total ${formatDecimal(cart.grandTotalExact)}.`,
    );
  }
  if (cart) {
    const explained = clampAtZero(
      subtractDecimals(
        addDecimals(itemsSubtotal, cart.taxAmountExact, cart.shippingAmountExact),
        cart.discountAmountExact,
      ),
    );
    money.explainedTotal = explained;
    if (compareDecimals(explained, orderTotal) !== 0) {
      flag(
        'order_total_unexplained',
        'warning',
        `Items ${itemsSubtotal} + tax ${money.tax} + shipping ${money.shipping} - discount ${money.discount} = ${explained}, not the order total ${orderTotal}.`,
      );
    }
  }
  if (recomputedTax && cart && differs(recomputedTax.totalTax, cart.taxAmountExact)) {
    flag(
      'tax_recompute_differs',
      'info',
      `Tax recomputed now is ${recomputedTax.totalTax}; the checkout cart stored ${money.tax}. Rates, exemptions or the address may have changed since checkout, or tax was never calculated on the cart.`,
    );
  }
  return { money, flags, flag };
}

async function explainOrder({ commerce, params }) {
  const identifier = params.orderId;
  const unavailable = [];
  let resolved;
  try {
    resolved = await resolveOrder(commerce, identifier);
  } catch (error) {
    return { success: false, error: `Could not read order ${identifier}: ${describeError(error)}` };
  }
  if (!resolved) return { success: false, error: `Order not found: ${identifier}` };
  const { order } = resolved;
  if (resolved.resolvedBy === 'order_number_scan') {
    unavailable.push({
      topic: 'order_by_number',
      reason:
        'No orders.getByNumber on the binding; the order number was resolved by scanning orders.',
      missingApi: 'orders.getByNumber(orderNumber)',
    });
  }

  const timeline = [];
  const push = (at, kind, event, summary, ref) => timeline.push({ at, kind, event, summary, ref });

  // Checkout cart: discount, tax, shipping, coupon, and when the cart opened.
  const cart = await readSection(unavailable, 'checkout_cart', () =>
    findCheckoutCart(commerce, order),
  );
  let cartItems = null;
  if (cart) {
    cartItems = await readSection(unavailable, 'checkout_cart_items', () =>
      commerce.carts.getItems(cart.id),
    );
    push(cart.createdAt, 'cart', 'created', `Cart ${cart.cartNumber} opened.`, {
      cartId: cart.id,
    });
  } else if (!unavailable.some((u) => u.topic === 'checkout_cart')) {
    unavailable.push({
      topic: 'checkout_cart',
      reason: order.customerId
        ? "No cart of this customer's links to the order (created directly, not by checkout)."
        : 'The order has no customer, so its checkout cart cannot be looked up.',
    });
  }
  const orderTotal = formatDecimal(order.totalAmountExact);
  const discountNote =
    cart && compareDecimals(cart.discountAmountExact, '0') > 0
      ? `, discount ${formatDecimal(cart.discountAmountExact)}${cart.couponCode ? ` (coupon ${cart.couponCode})` : ''}`
      : '';
  push(
    order.createdAt,
    'checkout',
    'order_created',
    cart
      ? `Checkout: cart ${cart.cartNumber} became order ${order.orderNumber} for ${orderTotal} ${order.currency}${discountNote}.`
      : `Order ${order.orderNumber} created for ${orderTotal} ${order.currency}.`,
    { orderId: order.id, cartId: cart?.id ?? null },
  );

  const payments =
    (await readSection(unavailable, 'payments', () =>
      commerce.payments.list({ orderId: order.id }),
    )) ?? [];
  const refundsReadable = typeof commerce.payments?.getRefunds === 'function';
  let refundsReadComplete = refundsReadable;
  const refunds = [];
  if (refundsReadable) {
    for (const payment of payments) {
      const records = await readSection(unavailable, 'refunds', () =>
        commerce.payments.getRefunds(payment.id),
      );
      if (Array.isArray(records)) refunds.push(...records);
      else refundsReadComplete = false;
    }
  }
  if (
    refundsReadable &&
    !refundsReadComplete &&
    !unavailable.some((entry) => entry.topic === 'refunds')
  ) {
    unavailable.push({
      topic: 'refunds',
      reason: 'The refund read returned no records for this payment.',
    });
  }
  for (const p of payments) {
    const amount = `${formatDecimal(p.amountExact)} ${p.currency}`;
    push(p.createdAt, 'payment', 'created', `Payment ${p.paymentNumber} of ${amount} created.`, {
      paymentId: p.id,
    });
    if (!refundsReadComplete && (p.status === 'refunded' || p.status === 'partially_refunded')) {
      push(
        p.updatedAt,
        'refund',
        p.status,
        p.status === 'refunded'
          ? `Payment ${p.paymentNumber} fully refunded (${amount}).`
          : `Payment ${p.paymentNumber} partially refunded (amount not readable).`,
        { paymentId: p.id },
      );
    } else if (p.status !== 'pending') {
      push(p.updatedAt, 'payment', p.status, `Payment ${p.paymentNumber} is ${p.status}.`, {
        paymentId: p.id,
      });
    }
  }
  for (const refund of refunds) {
    const amount = `${formatDecimal(refund.amountExact)} ${refund.currency}`;
    push(
      refund.refundedAt ?? refund.createdAt,
      'refund',
      refund.status,
      `Refund ${refund.refundNumber} for ${amount} is ${refund.status}.`,
      { paymentId: refund.paymentId, refundId: refund.id },
    );
  }

  const shipments =
    (await readSection(unavailable, 'shipments', () =>
      commerce.shipments.list({ orderId: order.id }),
    )) ?? [];
  for (const s of shipments) {
    push(
      s.createdAt,
      'shipment',
      'created',
      `Shipment ${s.shipmentNumber} created (${s.carrier} ${s.shippingMethod}).`,
      { shipmentId: s.id },
    );
    if (s.status !== 'pending' && s.updatedAt !== s.createdAt) {
      push(
        s.updatedAt,
        'shipment',
        s.status,
        `Shipment ${s.shipmentNumber} is ${s.status}${s.trackingNumber ? ` (tracking ${s.trackingNumber})` : ''}.`,
        { shipmentId: s.id },
      );
    }
  }

  const returns =
    (await readSection(unavailable, 'returns', () => commerce.returns.listForOrder(order.id))) ??
    [];
  for (const r of returns) {
    push(r.createdAt, 'return', 'created', `Return requested (${r.reason}); now ${r.status}.`, {
      returnId: r.id,
    });
  }

  const fraud = await readSection(unavailable, 'fraud', async () => {
    if (!(await commerce.fraud.isSupported())) {
      unavailable.push({ topic: 'fraud', reason: 'Fraud is not supported by this backend.' });
      return null;
    }
    const assessment = await commerce.fraud.getAssessment(order.id);
    if (!assessment) return { assessed: false };
    push(
      assessment.createdAt,
      'fraud',
      'assessed',
      `Fraud assessment: ${assessment.decision} (risk ${assessment.riskScore}, ${assessment.signals.length} signal(s)).`,
      { orderId: order.id },
    );
    if (assessment.reviewedBy) {
      push(
        assessment.updatedAt,
        'fraud',
        'reviewed',
        `Fraud review by ${assessment.reviewedBy}: ${assessment.decision}${assessment.reviewNotes ? ` (${assessment.reviewNotes})` : ''}.`,
        { orderId: order.id },
      );
    }
    return {
      assessed: true,
      decision: assessment.decision,
      riskScore: assessment.riskScore,
      needsReview: assessment.needsReview,
      reviewedBy: assessment.reviewedBy ?? null,
      reviewNotes: assessment.reviewNotes ?? null,
      signals: assessment.signals.map((s) => ({
        type: s.signalType,
        score: s.score,
        details: s.details,
        detectedAt: s.detectedAt,
      })),
    };
  });

  const activity = await readSection(unavailable, 'activity_log', async () => {
    if (!(await commerce.activityLogs.isSupported())) {
      unavailable.push({
        topic: 'activity_log',
        reason: 'Activity logs are not supported by this backend.',
      });
      return null;
    }
    return commerce.activityLogs.historyForSubject('order', order.id);
  });
  for (const entry of activity ?? []) {
    push(
      entry.createdAt,
      'activity',
      entry.action,
      `${entry.summary}${entry.actor ? ` (by ${entry.actorKind} ${entry.actor})` : ''}`,
      { activityId: entry.id },
    );
  }

  if (order.updatedAt && order.updatedAt !== order.createdAt) {
    push(
      order.updatedAt,
      'order_status',
      order.status,
      `Order is now ${order.status}${order.trackingNumber ? ` (tracking ${order.trackingNumber})` : ''}; payment ${order.paymentStatus}, fulfillment ${order.fulfillmentStatus}.`,
      { orderId: order.id },
    );
  }

  // Tax: recompute from the checkout cart when there is one (exactly as
  // calculate_cart_tax did), else from the order's items and address.
  const recomputedTax = await readSection(unavailable, 'tax', async () => {
    if (cart?.shippingAddress && cartItems) return recomputeCartTax(commerce, cart, cartItems);
    if (!order.shippingAddress) {
      unavailable.push({ topic: 'tax', reason: 'The order has no shipping address to tax.' });
      return null;
    }
    const result = await commerce.tax.calculate({
      lineItems: (order.items ?? []).map((item) => ({
        id: item.id,
        sku: item.sku,
        quantity: item.quantity,
        unitPrice: Number(item.unitPriceExact),
        taxCategory: 'standard',
        description: item.name,
      })),
      shippingAddress: taxAddress(order.shippingAddress),
      customerId: order.customerId,
      currency: order.currency,
    });
    return formatTax(result);
  });

  const refundsAvailable =
    refundsReadable &&
    refundsReadComplete &&
    !unavailable.some((entry) => entry.topic === 'refunds' || entry.topic === 'payments');
  const { money, flags, flag } = explainOrderMoney({
    order,
    cart,
    payments,
    refunds,
    refundsReadable: refundsAvailable,
    recomputedTax,
  });

  const hasShippedShipment = shipments.some((s) => SHIPPED_SHIPMENT_STATUSES.has(s.status));
  const orderShipped = ['shipped', 'delivered'].includes(order.status);
  if ((orderShipped || hasShippedShipment) && order.fulfillmentStatus === 'unfulfilled') {
    flag(
      'order_fulfillment_status_stale',
      'warning',
      `The order is ${order.status}${hasShippedShipment ? ' and a shipment has left' : ''}, but its fulfillmentStatus is 'unfulfilled'.`,
    );
  }
  if (orderShipped && shipments.length > 0 && !hasShippedShipment) {
    flag(
      'shipment_status_behind_order',
      'warning',
      `The order is ${order.status} but no shipment record is shipped (${shipments.map((s) => `${s.shipmentNumber}: ${s.status}`).join(', ')}).`,
    );
  }
  if (orderShipped && shipments.length === 0) {
    flag(
      'shipped_without_shipment',
      'info',
      `The order is ${order.status} with no shipment record.`,
    );
  }
  if (orderShipped && compareDecimals(money.charged, money.orderTotal) < 0) {
    flag(
      'shipped_before_paid',
      'warning',
      `The order is ${order.status} but only ${money.charged} of ${money.orderTotal} ${order.currency} is captured.`,
    );
  }
  const completedReturns = returns.filter((r) => ['received', 'completed'].includes(r.status));
  if (
    completedReturns.length > 0 &&
    (money.refunded === null || compareDecimals(money.refunded, '0') === 0)
  ) {
    flag(
      'return_refund_unverifiable',
      'info',
      `${completedReturns.length} return(s) are received or completed, but no refund is readable.`,
    );
  }
  if (
    fraud?.assessed &&
    (fraud.decision === 'reject' || (fraud.needsReview && !fraud.reviewedBy)) &&
    (orderShipped || compareDecimals(money.charged, '0') > 0)
  ) {
    flag(
      'fraud_hold_not_honoured',
      'error',
      `Fraud decision is '${fraud.decision}'${fraud.needsReview ? ' pending review' : ''}, yet the order was ${orderShipped ? order.status : 'charged'}.`,
    );
  }

  return {
    success: true,
    order: summarizeOrder(order),
    timeline: sortTimeline(timeline),
    money,
    tax: recomputedTax ? { basis: 'recomputed_now', ...recomputedTax } : null,
    promotions: cart
      ? {
          basis: 'checkout_cart',
          couponCode: cart.couponCode ?? null,
          discount: formatDecimal(cart.discountAmountExact),
        }
      : null,
    payments: payments.map((p) => ({
      id: p.id,
      paymentNumber: p.paymentNumber,
      status: p.status,
      amount: formatDecimal(p.amountExact),
      currency: p.currency,
      createdAt: p.createdAt,
      updatedAt: p.updatedAt,
    })),
    shipments: shipments.map((s) => ({
      id: s.id,
      shipmentNumber: s.shipmentNumber,
      status: s.status,
      carrier: s.carrier,
      shippingMethod: s.shippingMethod,
      trackingNumber: s.trackingNumber ?? null,
    })),
    returns: returns.map((r) => ({ id: r.id, status: r.status, reason: r.reason })),
    fraud,
    flags,
    unavailable: [
      ...unavailable,
      ...EXPLAIN_ORDER_API_GAPS.filter((gap) => gap.topic !== 'refunds' || !refundsAvailable),
    ],
  };
}

// ---------------------------------------------------------------------------
// explain_cart_pricing
// ---------------------------------------------------------------------------

async function explainCartPricing({ commerce, params }) {
  const { cartId } = params;
  const unavailable = [];
  let cart;
  try {
    cart = await commerce.carts.get(cartId);
  } catch (error) {
    return { success: false, error: `Could not read cart ${cartId}: ${describeError(error)}` };
  }
  if (!cart) return { success: false, error: `Cart not found: ${cartId}` };

  const items =
    (await readSection(unavailable, 'cart_items', () => commerce.carts.getItems(cart.id))) ?? [];
  const lines = items.map((item) => {
    const gross = multiplyDecimals(item.unitPriceExact, item.quantity);
    return {
      id: item.id,
      sku: item.sku,
      name: item.name,
      quantity: item.quantity,
      unitPrice: formatDecimal(item.unitPriceExact),
      originalPrice: item.originalPriceExact ? formatDecimal(item.originalPriceExact) : null,
      gross,
      lineDiscount: formatDecimal(item.discountAmountExact),
      net: subtractDecimals(gross, item.discountAmountExact),
      storedTotal: formatDecimal(item.totalExact),
      storedTax: formatDecimal(item.taxAmountExact),
    };
  });
  const linesSubtotal = addDecimals(lines.map((line) => line.net));

  const stored = {
    subtotal: formatDecimal(cart.subtotalExact),
    discount: formatDecimal(cart.discountAmountExact),
    tax: formatDecimal(cart.taxAmountExact),
    shipping: formatDecimal(cart.shippingAmountExact),
    grandTotal: formatDecimal(cart.grandTotalExact),
  };

  // Promotions: re-evaluate the cart the way applyToCart builds its request
  // (its lines, its coupon, every automatic promotion) -- without writing.
  // Extra codes the caller asks about are a separate what-if evaluation, so
  // they cannot change the explanation of the cart's own price.
  const cartCodes = cart.couponCode ? [cart.couponCode] : [];
  const evaluate = async (couponCodes, basis) => {
    const result = await commerce.promotions.apply({
      cartId: cart.id,
      customerId: cart.customerId,
      couponCodes,
      lineItems: items.map((item) => ({
        id: item.id,
        productId: item.productId,
        variantId: item.variantId,
        sku: item.sku,
        quantity: item.quantity,
        unitPrice: Number(item.unitPriceExact),
        lineTotal: Number(item.totalExact),
      })),
      subtotal: Number(cart.subtotalExact),
      shippingAmount: Number(cart.shippingAmountExact),
      shippingCountry: cart.shippingAddress?.country,
      shippingState: cart.shippingAddress?.state,
      currency: cart.currency,
    });
    return {
      basis,
      couponCodesEvaluated: couponCodes,
      totalDiscount: formatDecimal(result.totalDiscountExact),
      shippingDiscount: formatDecimal(result.shippingDiscountExact),
      applied: result.appliedPromotions.map((p) => ({
        promotionId: p.promotionId,
        name: p.promotionName,
        type: p.discountType,
        couponCode: p.couponCode ?? null,
        discount: formatDecimal(p.discountAmountExact),
      })),
      rejected: (result.rejectedPromotions ?? []).map((p) => ({
        promotionId: p.promotionId ?? null,
        couponCode: p.couponCode ?? null,
        reasonCode: p.reasonCode,
        reason: p.reason,
      })),
    };
  };
  const promotions = await readSection(unavailable, 'promotions', () =>
    evaluate(cartCodes, 'reevaluated_now'),
  );
  const extraCodes = [...new Set(params.couponCodes ?? [])].filter((c) => !cartCodes.includes(c));
  const couponCheck =
    extraCodes.length > 0
      ? await readSection(unavailable, 'coupon_check', () =>
          evaluate([...cartCodes, ...extraCodes], 'what_if_with_extra_codes'),
        )
      : null;

  const tax = await readSection(unavailable, 'tax', async () => {
    if (!cart.shippingAddress) {
      unavailable.push({
        topic: 'tax',
        reason: 'The cart has no shipping address, so tax cannot be computed.',
      });
      return null;
    }
    return { basis: 'recomputed_now', ...(await recomputeCartTax(commerce, cart, items)) };
  });

  const shipping = {
    amount: stored.shipping,
    method: cart.shippingMethod ?? null,
    carrier: cart.shippingCarrier ?? null,
    fulfillmentType: cart.fulfillmentType ?? null,
  };

  // The engine's own formula: max(0, round2(subtotal + tax + shipping - discount)).
  const explainedTotal = clampAtZero(
    roundDecimal(
      subtractDecimals(addDecimals(stored.subtotal, stored.tax, stored.shipping), stored.discount),
      2,
    ),
  );
  const reconciliation = {
    formula: 'max(0, round2(subtotal + tax + shipping - discount))',
    explainedTotal,
    storedGrandTotal: stored.grandTotal,
    matches: compareDecimals(explainedTotal, stored.grandTotal) === 0,
    difference: subtractDecimals(stored.grandTotal, explainedTotal),
  };

  const flags = [];
  const flag = (code, severity, message) => flags.push({ code, severity, message });
  if (!reconciliation.matches) {
    flag(
      'grand_total_unexplained',
      'error',
      `Stored grand total ${stored.grandTotal} differs from ${explainedTotal} (subtotal ${stored.subtotal} + tax ${stored.tax} + shipping ${stored.shipping} - discount ${stored.discount}).`,
    );
  }
  if (items.length > 0 && compareDecimals(linesSubtotal, stored.subtotal) !== 0) {
    flag(
      'subtotal_differs_from_lines',
      'warning',
      `Lines sum to ${linesSubtotal} but the stored subtotal is ${stored.subtotal}.`,
    );
  }
  if (tax && compareDecimals(tax.totalTax, stored.tax) !== 0) {
    flag(
      'tax_not_current',
      'warning',
      `Tax recomputed now is ${tax.totalTax}; the cart holds ${stored.tax}. Run calculate_cart_tax (or recalculate) before checkout.`,
    );
  }
  if (promotions) {
    const onCart = promotions.totalDiscount;
    if (compareDecimals(onCart, stored.discount) !== 0) {
      flag(
        'discount_not_current',
        cart.status === 'completed' ? 'info' : 'warning',
        cart.status === 'completed'
          ? `Promotions re-evaluated now give ${onCart}; the cart was checked out with ${stored.discount}. The customer's history has changed since checkout.`
          : `Promotions evaluated now give ${onCart}; the cart holds ${stored.discount}. Re-apply the discount before checkout.`,
      );
    }
    for (const rejected of promotions.rejected) {
      if (rejected.couponCode && rejected.couponCode === cart.couponCode) {
        flag(
          'cart_coupon_refused_now',
          cart.status === 'completed' ? 'info' : 'warning',
          `The cart's coupon ${cart.couponCode} is refused now: ${rejected.reasonCode} (${rejected.reason}).`,
        );
      }
    }
  }

  return {
    success: true,
    cart: {
      id: cart.id,
      cartNumber: cart.cartNumber,
      status: cart.status,
      customerId: cart.customerId ?? null,
      currency: cart.currency,
      couponCode: cart.couponCode ?? null,
      orderId: cart.orderId ?? null,
      shippingAddress: cart.shippingAddress ?? null,
    },
    lines,
    subtotal: { stored: stored.subtotal, fromLines: linesSubtotal },
    promotions,
    couponCheck,
    tax,
    shipping,
    discount: stored.discount,
    stored,
    total: reconciliation,
    flags,
    unavailable: [...unavailable, ...EXPLAIN_CART_API_GAPS],
  };
}

export const explainTools = [
  {
    name: 'explain_order',
    description:
      'Explain an order to a customer or merchant: one chronological timeline (checkout, payments, ' +
      'shipments, returns, refunds, fraud, activity), the money charged/refunded/net as exact ' +
      'strings, tax recomputed from the address, and flags for inconsistencies (e.g. paid but ' +
      'paymentStatus pending, refunded more than charged). Read-only; lists what it cannot see.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID (UUID) or order number'),
    },
    permission: 'read',
    handler: explainOrder,
  },
  {
    name: 'explain_cart_pricing',
    description:
      'Explain why a cart costs what it costs: lines, subtotal, promotions applied and REFUSED ' +
      'with reason codes, tax by jurisdiction, shipping, and a check that the explained total ' +
      'equals the stored grand total. Pass couponCodes to ask why a code does or does not apply. ' +
      'Read-only: nothing is written to the cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID'),
      couponCodes: z
        .array(z.string().min(1).max(100))
        .max(20)
        .optional()
        .describe("Extra coupon codes to evaluate alongside the cart's own coupon"),
    },
    permission: 'read',
    handler: explainCartPricing,
  },
];

export default explainTools;
