/**
 * Stripe ↔ StateSet Data Mapper
 *
 * Pure functions that transform between Stripe's data model and StateSet's.
 * No I/O — fully deterministic and trivially testable.
 */

// ---------------------------------------------------------------------------
// Status mappings
// ---------------------------------------------------------------------------

export const PAYMENT_INTENT_STATUS_MAP = {
  succeeded: 'completed',
  processing: 'processing',
  requires_payment_method: 'pending',
  requires_confirmation: 'pending',
  requires_action: 'pending',
  canceled: 'cancelled',
  requires_capture: 'authorized',
};

export const CHARGE_STATUS_MAP = {
  succeeded: 'completed',
  pending: 'processing',
  failed: 'failed',
};

export const REFUND_STATUS_MAP = {
  succeeded: 'completed',
  pending: 'pending',
  failed: 'failed',
  canceled: 'cancelled',
  requires_action: 'pending',
};

export const SUBSCRIPTION_STATUS_MAP = {
  active: 'active',
  past_due: 'past_due',
  unpaid: 'past_due',
  canceled: 'cancelled',
  incomplete: 'pending',
  incomplete_expired: 'cancelled',
  trialing: 'active',
  paused: 'paused',
};

export const INVOICE_STATUS_MAP = {
  paid: 'paid',
  open: 'pending',
  draft: 'draft',
  uncollectible: 'failed',
  void: 'cancelled',
};

export const DISPUTE_STATUS_MAP = {
  warning_needs_response: 'open',
  warning_under_review: 'under_review',
  warning_closed: 'closed',
  needs_response: 'open',
  under_review: 'under_review',
  charge_refunded: 'refunded',
  won: 'won',
  lost: 'lost',
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert Stripe amount (cents) to decimal string.
 * @param {number} amountInCents
 * @returns {string}
 */
export function centsToDecimal(amountInCents) {
  if (amountInCents === null || amountInCents === undefined || typeof amountInCents !== 'number') {
    return '0.00';
  }
  return (amountInCents / 100).toFixed(2);
}

/**
 * Convert Stripe timestamp (Unix seconds) to ISO 8601.
 * @param {number} timestamp
 * @returns {string|null}
 */
export function timestampToIso(timestamp) {
  if (!timestamp || typeof timestamp !== 'number') return null;
  return new Date(timestamp * 1000).toISOString();
}

/**
 * Extract name from Stripe object.
 * @param {Object} obj
 * @returns {{ firstName: string, lastName: string }}
 */
function extractName(obj) {
  const name = obj?.name || obj?.billing_details?.name || '';
  const parts = name.split(' ');
  return {
    firstName: parts[0] || '',
    lastName: parts.slice(1).join(' ') || '',
  };
}

// ---------------------------------------------------------------------------
// Payment Intent mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe PaymentIntent to StateSet payment format.
 * @param {Object} intent - Stripe PaymentIntent object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapPaymentIntentToStateSet(intent) {
  if (!intent || !intent.id) {
    throw new Error('Invalid Stripe PaymentIntent: missing id');
  }

  return {
    externalId: intent.id,
    data: {
      amount: centsToDecimal(intent.amount),
      currency: (intent.currency || 'usd').toUpperCase(),
      status: PAYMENT_INTENT_STATUS_MAP[intent.status] || 'pending',
      method: intent.payment_method_types?.[0] || 'card',
      externalId: intent.id,
      customerId: intent.customer || null,
      orderId: intent.metadata?.order_id || null,
      description: intent.description || null,
      createdAt: timestampToIso(intent.created),
    },
    raw: intent,
  };
}

// ---------------------------------------------------------------------------
// Charge mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Charge to StateSet payment format.
 * @param {Object} charge - Stripe Charge object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapChargeToStateSet(charge) {
  if (!charge || !charge.id) {
    throw new Error('Invalid Stripe Charge: missing id');
  }

  return {
    externalId: charge.id,
    data: {
      amount: centsToDecimal(charge.amount),
      currency: (charge.currency || 'usd').toUpperCase(),
      status: CHARGE_STATUS_MAP[charge.status] || 'pending',
      method: charge.payment_method_details?.type || 'card',
      externalId: charge.id,
      paymentIntentId: charge.payment_intent || null,
      customerId: charge.customer || null,
      description: charge.description || null,
      receiptEmail: charge.receipt_email || null,
      createdAt: timestampToIso(charge.created),
    },
    raw: charge,
  };
}

// ---------------------------------------------------------------------------
// Customer mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Customer to StateSet customer format.
 * @param {Object} customer - Stripe Customer object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapCustomerToStateSet(customer) {
  if (!customer || !customer.id) {
    throw new Error('Invalid Stripe Customer: missing id');
  }

  const { firstName, lastName } = extractName(customer);

  return {
    externalId: customer.id,
    data: {
      email: customer.email || null,
      firstName,
      lastName,
      phone: customer.phone || null,
      externalId: customer.id,
      metadata: customer.metadata || {},
      createdAt: timestampToIso(customer.created),
    },
    raw: customer,
  };
}

// ---------------------------------------------------------------------------
// Subscription mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Subscription to StateSet subscription format.
 * @param {Object} sub - Stripe Subscription object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapSubscriptionToStateSet(sub) {
  if (!sub || !sub.id) {
    throw new Error('Invalid Stripe Subscription: missing id');
  }

  const firstItem = sub.items?.data?.[0];

  return {
    externalId: sub.id,
    data: {
      status: SUBSCRIPTION_STATUS_MAP[sub.status] || 'pending',
      customerId: sub.customer || null,
      planId: firstItem?.price?.id || firstItem?.plan?.id || null,
      planName: firstItem?.price?.nickname || firstItem?.plan?.nickname || null,
      amount: firstItem?.price?.unit_amount ? centsToDecimal(firstItem.price.unit_amount) : null,
      currency: (sub.currency || 'usd').toUpperCase(),
      interval: firstItem?.price?.recurring?.interval || firstItem?.plan?.interval || null,
      intervalCount:
        firstItem?.price?.recurring?.interval_count || firstItem?.plan?.interval_count || 1,
      currentPeriodStart: timestampToIso(sub.current_period_start),
      currentPeriodEnd: timestampToIso(sub.current_period_end),
      cancelAtPeriodEnd: sub.cancel_at_period_end || false,
      trialEnd: timestampToIso(sub.trial_end),
      externalId: sub.id,
      createdAt: timestampToIso(sub.created),
    },
    raw: sub,
  };
}

// ---------------------------------------------------------------------------
// Refund mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Refund to StateSet refund format.
 * @param {Object} refund - Stripe Refund object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapRefundToStateSet(refund) {
  if (!refund || !refund.id) {
    throw new Error('Invalid Stripe Refund: missing id');
  }

  return {
    externalId: refund.id,
    data: {
      amount: centsToDecimal(refund.amount),
      currency: (refund.currency || 'usd').toUpperCase(),
      status: REFUND_STATUS_MAP[refund.status] || 'pending',
      reason: refund.reason || null,
      paymentIntentId: refund.payment_intent || null,
      chargeId: refund.charge || null,
      externalId: refund.id,
      createdAt: timestampToIso(refund.created),
    },
    raw: refund,
  };
}

// ---------------------------------------------------------------------------
// Invoice mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Invoice to StateSet invoice format.
 * @param {Object} invoice - Stripe Invoice object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapInvoiceToStateSet(invoice) {
  if (!invoice || !invoice.id) {
    throw new Error('Invalid Stripe Invoice: missing id');
  }

  return {
    externalId: invoice.id,
    data: {
      amount: centsToDecimal(invoice.amount_due),
      amountPaid: centsToDecimal(invoice.amount_paid),
      currency: (invoice.currency || 'usd').toUpperCase(),
      status: INVOICE_STATUS_MAP[invoice.status] || 'pending',
      customerId: invoice.customer || null,
      subscriptionId: invoice.subscription || null,
      number: invoice.number || null,
      dueDate: timestampToIso(invoice.due_date),
      paidAt: timestampToIso(invoice.status_transitions?.paid_at),
      externalId: invoice.id,
      createdAt: timestampToIso(invoice.created),
    },
    raw: invoice,
  };
}

// ---------------------------------------------------------------------------
// Dispute mapper
// ---------------------------------------------------------------------------

/**
 * Map a Stripe Dispute to StateSet dispute format.
 * @param {Object} dispute - Stripe Dispute object
 * @returns {{ externalId: string, data: Object, raw: Object }}
 */
export function mapDisputeToStateSet(dispute) {
  if (!dispute || !dispute.id) {
    throw new Error('Invalid Stripe Dispute: missing id');
  }

  return {
    externalId: dispute.id,
    data: {
      amount: centsToDecimal(dispute.amount),
      currency: (dispute.currency || 'usd').toUpperCase(),
      status: DISPUTE_STATUS_MAP[dispute.status] || 'open',
      reason: dispute.reason || null,
      chargeId: dispute.charge || null,
      paymentIntentId: dispute.payment_intent || null,
      evidenceDueBy: timestampToIso(dispute.evidence_details?.due_by),
      externalId: dispute.id,
      createdAt: timestampToIso(dispute.created),
    },
    raw: dispute,
  };
}

// ---------------------------------------------------------------------------
// Dispatch mapper
// ---------------------------------------------------------------------------

/**
 * Map any Stripe entity to StateSet format by type.
 * @param {string} entityType
 * @param {Object} record
 * @returns {Object}
 */
export function mapToStateSet(entityType, record) {
  switch (entityType) {
    case 'payment_intents':
      return mapPaymentIntentToStateSet(record);
    case 'charges':
      return mapChargeToStateSet(record);
    case 'customers':
      return mapCustomerToStateSet(record);
    case 'subscriptions':
      return mapSubscriptionToStateSet(record);
    case 'refunds':
      return mapRefundToStateSet(record);
    case 'invoices':
      return mapInvoiceToStateSet(record);
    case 'disputes':
      return mapDisputeToStateSet(record);
    default:
      throw new Error(`Unsupported Stripe entity type: ${entityType}`);
  }
}
