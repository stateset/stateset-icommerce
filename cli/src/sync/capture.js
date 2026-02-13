/**
 * Event Capture for Commerce Operations
 *
 * Wraps Commerce instance to automatically capture events on mutations.
 * Every write operation atomically:
 * 1. Executes the original operation
 * 2. Appends an event to the outbox
 */

import { createOutbox } from './outbox.js';

/**
 * Event type mappings for commerce operations
 */
const EVENT_MAPPINGS = {
  // Orders
  'orders.create': 'order.created',
  'orders.updateStatus': 'order.status_changed',
  'orders.ship': 'order.shipped',
  'orders.cancel': 'order.cancelled',
  'orders.update': 'order.updated',

  // Customers
  'customers.create': 'customer.created',
  'customers.update': 'customer.updated',
  'customers.delete': 'customer.deleted',

  // Products
  'products.create': 'product.created',
  'products.update': 'product.updated',
  'products.delete': 'product.deleted',

  // Inventory
  'inventory.create': 'inventory.created',
  'inventory.adjust': 'inventory.adjusted',
  'inventory.reserve': 'inventory.reserved',
  'inventory.release': 'inventory.released',
  'inventory.confirm': 'inventory.confirmed',

  // Returns
  'returns.create': 'return.requested',
  'returns.approve': 'return.approved',
  'returns.reject': 'return.rejected',
  'returns.complete': 'return.completed',

  // Payments
  'payments.create': 'payment.created',
  'payments.markCompleted': 'payment.completed',
  'payments.markFailed': 'payment.failed',
  'payments.refund': 'payment.refunded',

  // Carts
  'carts.create': 'cart.created',
  'carts.addItem': 'cart.item_added',
  'carts.updateItem': 'cart.item_updated',
  'carts.removeItem': 'cart.item_removed',
  'carts.setShipping': 'cart.shipping_set',
  'carts.setPayment': 'cart.payment_set',
  'carts.applyDiscount': 'cart.discount_applied',
  'carts.checkout': 'cart.checked_out',
  'carts.cancel': 'cart.cancelled',
  'carts.abandon': 'cart.abandoned',

  // A2A Commerce
  'a2a.pay': 'a2a_payment.created',
  'a2a.requestPayment': 'a2a_payment_request.created',
  'a2a.payRequest': 'a2a_payment_request.paid',
  'a2a.requestQuote': 'a2a_quote.requested',
  'a2a.provideQuote': 'a2a_quote.provided',
  'a2a.acceptQuote': 'a2a_quote.accepted',
  'a2a.declineQuote': 'a2a_quote.declined',
  'a2a.fulfillQuote': 'a2a_quote.fulfilled',
  'a2a.counterQuote': 'a2a_quote.countered',
  'a2a.reviseQuote': 'a2a_quote.revised',
  'a2a.createEscrow': 'a2a_escrow.created',
  'a2a.fundEscrow': 'a2a_escrow.funded',
  'a2a.releaseEscrow': 'a2a_escrow.released',
  'a2a.refundEscrow': 'a2a_escrow.refunded',
  'a2a.disputeEscrow': 'a2a_escrow.disputed',
  'a2a.fileDispute': 'a2a_dispute.filed',
  'a2a.submitEvidence': 'a2a_dispute.evidence_submitted',
  'a2a.resolveDispute': 'a2a_dispute.resolved',
  'a2a.escalateDispute': 'a2a_dispute.escalated',
  'a2a.rateAgent': 'a2a_feedback.submitted',
  'a2a.registerService': 'a2a_service.registered',

  // A2A Phase B — Notifications, Subscriptions, Splits, Conditional, Events
  'a2a.sendNotification': 'a2a_notification.sent',
  'a2a.createSubscription': 'a2a_subscription.created',
  'a2a.pauseSubscription': 'a2a_subscription.paused',
  'a2a.resumeSubscription': 'a2a_subscription.resumed',
  'a2a.cancelSubscription': 'a2a_subscription.cancelled',
  'a2a.processBilling': 'a2a_subscription.billed',
  'a2a.createSplitPayment': 'a2a_split_payment.created',
  'a2a.executeSplitPayment': 'a2a_split_payment.executed',
  'a2a.createConditionalPayment': 'a2a_conditional_payment.created',
  'a2a.settleConditionalPayment': 'a2a_conditional_payment.settled',
  'a2a.subscribeEvents': 'a2a_event_subscription.created',
};

/**
 * Entity type extraction from resource name
 */
const ENTITY_TYPES = {
  orders: 'order',
  customers: 'customer',
  products: 'product',
  inventory: 'inventory',
  returns: 'return',
  payments: 'payment',
  carts: 'cart',
  subscriptions: 'subscription',
  promotions: 'promotion',
  a2a: 'a2a',
};

/**
 * Event capture wrapper class
 */
export class EventCapture {
  /**
   * @param {import('better-sqlite3').Database} db - SQLite database
   * @param {Object} config - Sync configuration
   */
  constructor(db, config) {
    this.outbox = createOutbox(db);
    this.config = config;
    this.enabled = true;
  }

  /**
   * Enable/disable event capture
   * @param {boolean} enabled
   */
  setEnabled(enabled) {
    this.enabled = enabled;
  }

  /**
   * Capture an event
   * @param {string} resourceMethod - Resource.method (e.g., 'orders.create')
   * @param {string} entityId - Entity identifier
   * @param {Object} payload - Event payload
   * @param {Object} [options]
   * @param {string} [options.commandId] - Idempotency key
   * @param {number} [options.baseVersion] - OCC version
   */
  capture(resourceMethod, entityId, payload, options = {}) {
    if (!this.enabled) return;

    const eventType = EVENT_MAPPINGS[resourceMethod];
    if (!eventType) {
      console.warn(
        `[EventCapture] Unmapped operation '${resourceMethod}' — events will not be captured. Add it to EVENT_MAPPINGS.`,
      );
      return;
    }

    const [resource] = resourceMethod.split('.');
    const entityType = ENTITY_TYPES[resource] || resource;

    this.outbox.append({
      tenantId: this.config.identity.tenantId,
      storeId: this.config.identity.storeId,
      entityType,
      entityId: String(entityId),
      eventType,
      payload,
      sourceAgent: this.config.identity.agentId,
      commandId: options.commandId,
      baseVersion: options.baseVersion,
    });
  }

  /**
   * Get the outbox for direct access
   * @returns {import('./outbox.js').Outbox}
   */
  getOutbox() {
    return this.outbox;
  }
}

/**
 * Wrap a single resource with event capture
 * @param {Object} resource - Commerce resource (e.g., commerce.orders)
 * @param {string} resourceName - Resource name (e.g., 'orders')
 * @param {EventCapture} capture - Event capture instance
 * @returns {Object} - Wrapped resource
 */
function wrapResource(resource, resourceName, capture) {
  const wrapped = {};

  for (const [methodName, method] of Object.entries(resource)) {
    if (typeof method !== 'function') {
      wrapped[methodName] = method;
      continue;
    }

    // Check if this method should be captured
    const mappingKey = `${resourceName}.${methodName}`;
    const shouldCapture = EVENT_MAPPINGS[mappingKey] !== undefined;

    if (!shouldCapture) {
      // Pass through read-only methods
      wrapped[methodName] = method.bind(resource);
      continue;
    }

    // Wrap write methods with event capture
    wrapped[methodName] = function (...args) {
      // Call original method
      const result = method.apply(resource, args);

      // Capture event
      try {
        const entityId = extractEntityId(resourceName, methodName, args, result);
        const payload = buildPayload(resourceName, methodName, args, result);

        capture.capture(mappingKey, entityId, payload);
      } catch (error) {
        // Log but don't fail the operation
        console.error(`Event capture failed for ${mappingKey}:`, error.message);
      }

      return result;
    };
  }

  return wrapped;
}

/**
 * Extract entity ID from method call
 * @param {string} resourceName
 * @param {string} methodName
 * @param {Array} args
 * @param {*} result
 * @returns {string}
 */
function extractEntityId(resourceName, methodName, args, result) {
  // For create operations, ID is in result
  if (methodName === 'create') {
    if (result && typeof result === 'object') {
      return (
        result.id ||
        result.order_id ||
        result.customer_id ||
        result.product_id ||
        result.cart_id ||
        result.return_id ||
        result.payment_id ||
        'unknown'
      );
    }
    return 'unknown';
  }

  // For other operations, ID is typically first argument
  if (args[0]) {
    if (typeof args[0] === 'string') {
      return args[0];
    }
    if (typeof args[0] === 'object') {
      return args[0].id || args[0].orderId || args[0].entityId || 'unknown';
    }
  }

  return 'unknown';
}

/**
 * Build event payload from method call
 * @param {string} resourceName
 * @param {string} methodName
 * @param {Array} args
 * @param {*} result
 * @returns {Object}
 */
function buildPayload(resourceName, methodName, args, result) {
  const payload = {
    method: `${resourceName}.${methodName}`,
    timestamp: new Date().toISOString(),
  };

  // Include arguments (sanitized)
  if (args.length > 0) {
    payload.args = args.map((arg) => {
      if (typeof arg === 'object' && arg !== null) {
        // Clone and remove sensitive fields
        const sanitized = { ...arg };
        delete sanitized.password;
        delete sanitized.apiKey;
        delete sanitized.api_key;
        delete sanitized.token;
        delete sanitized.creditCard;
        delete sanitized.cardNumber;
        delete sanitized.secret;
        delete sanitized.authorization;
        delete sanitized.credential;
        delete sanitized.ssn;
        return sanitized;
      }
      return arg;
    });
  }

  // Include result if it's an object
  if (result && typeof result === 'object') {
    payload.result = { ...result };
    delete payload.result.password;
    delete payload.result.apiKey;
  }

  return payload;
}

/**
 * Wrap a Commerce instance with event capture
 * @param {Object} commerce - Commerce instance from @stateset/embedded
 * @param {Object} config - Sync configuration
 * @returns {Object} - Wrapped commerce instance with _outbox and _capture
 */
export function wrapCommerceWithEvents(commerce, config) {
  // Guard: commerce.db may be undefined if using N-API binding
  // In this case, skip sync and return commerce as-is
  if (!commerce.db) {
    console.warn('[Sync] commerce.db not available, event sync disabled');
    return commerce;
  }

  const capture = new EventCapture(commerce.db, config);

  const wrapped = {
    // Pass through non-resource properties
    db: commerce.db,
    close: commerce.close?.bind(commerce),

    // Wrap each resource
    orders: commerce.orders ? wrapResource(commerce.orders, 'orders', capture) : undefined,
    customers: commerce.customers
      ? wrapResource(commerce.customers, 'customers', capture)
      : undefined,
    products: commerce.products ? wrapResource(commerce.products, 'products', capture) : undefined,
    inventory: commerce.inventory
      ? wrapResource(commerce.inventory, 'inventory', capture)
      : undefined,
    returns: commerce.returns ? wrapResource(commerce.returns, 'returns', capture) : undefined,
    payments: commerce.payments ? wrapResource(commerce.payments, 'payments', capture) : undefined,
    carts: commerce.carts ? wrapResource(commerce.carts, 'carts', capture) : undefined,

    // Expose outbox for sync operations
    _outbox: capture.getOutbox(),
    _capture: capture,
  };

  // Copy any other properties
  for (const key of Object.keys(commerce)) {
    if (!(key in wrapped)) {
      wrapped[key] = commerce[key];
    }
  }

  return wrapped;
}

/**
 * Create a standalone event capture instance
 * @param {import('better-sqlite3').Database} db
 * @param {Object} config
 * @returns {EventCapture}
 */
export function createEventCapture(db, config) {
  return new EventCapture(db, config);
}
