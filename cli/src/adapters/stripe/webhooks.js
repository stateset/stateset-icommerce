/**
 * Stripe Webhook Event Handlers
 *
 * Processes Stripe webhook payloads and creates/updates records in StateSet.
 * Each handler follows: validate → map → id_map check → commerce write → id_map store.
 */

import {
  mapPaymentIntentToStateSet,
  mapChargeToStateSet,
  mapCustomerToStateSet,
  mapSubscriptionToStateSet,
  mapRefundToStateSet,
  mapInvoiceToStateSet,
  mapDisputeToStateSet,
} from './mapper.js';

/**
 * Create a set of Stripe webhook handlers.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {import('../id-map-store.js').IdMapStore} idMapStore
 * @returns {Object<string, (payload: Object) => Promise<Object>>}
 */
export function createStripeWebhookHandlers(commerce, idMapStore) {
  const platform = 'stripe';

  /**
   * Helper: create or skip based on id_map.
   */
  async function createOrSkip(entityType, mapped, createFn) {
    const existing = idMapStore.lookup(platform, entityType, mapped.externalId);
    if (existing) {
      return {
        action: 'skipped',
        externalId: mapped.externalId,
        statesetId: existing.statesetId,
      };
    }

    const result = await createFn(mapped.data);
    const statesetId = result?.id || result?.[`${entityType.slice(0, -1)}_id`] || mapped.externalId;

    idMapStore.store(platform, entityType, mapped.externalId, statesetId, mapped.raw);

    return { action: 'created', externalId: mapped.externalId, statesetId };
  }

  /**
   * Helper: update if exists, create otherwise.
   */
  async function upsert(entityType, mapped, createFn) {
    const existing = idMapStore.lookup(platform, entityType, mapped.externalId);
    if (existing) {
      idMapStore.store(platform, entityType, mapped.externalId, existing.statesetId, mapped.raw);
      return {
        action: 'updated',
        externalId: mapped.externalId,
        statesetId: existing.statesetId,
      };
    }
    return createOrSkip(entityType, mapped, createFn);
  }

  return {
    // --- Payment Intents ---

    'payment_intent.succeeded': async (payload) => {
      const intent = payload.data?.object || payload;
      const mapped = mapPaymentIntentToStateSet(intent);
      if (!commerce.payments?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No payments handler' };
      }
      return createOrSkip('payments', mapped, (data) => commerce.payments.create(data));
    },

    'payment_intent.payment_failed': async (payload) => {
      const intent = payload.data?.object || payload;
      const mapped = mapPaymentIntentToStateSet(intent);
      const existing = idMapStore.lookup(platform, 'payments', mapped.externalId);
      if (existing) {
        idMapStore.store(platform, 'payments', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'updated',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      return { action: 'recorded', externalId: mapped.externalId, status: 'failed' };
    },

    'payment_intent.canceled': async (payload) => {
      const intent = payload.data?.object || payload;
      const mapped = mapPaymentIntentToStateSet(intent);
      const existing = idMapStore.lookup(platform, 'payments', mapped.externalId);
      if (existing) {
        idMapStore.store(platform, 'payments', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'cancelled',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      return { action: 'skipped', externalId: mapped.externalId, reason: 'Not found in id_map' };
    },

    // --- Charges ---

    'charge.succeeded': async (payload) => {
      const charge = payload.data?.object || payload;
      const mapped = mapChargeToStateSet(charge);
      if (!commerce.payments?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No payments handler' };
      }
      return createOrSkip('charges', mapped, (data) => commerce.payments.create(data));
    },

    'charge.refunded': async (payload) => {
      const charge = payload.data?.object || payload;
      // Extract the refund from the charge's refunds list
      const latestRefund = charge.refunds?.data?.[0];
      if (!latestRefund) {
        return { action: 'skipped', externalId: charge.id, reason: 'No refund data in charge' };
      }

      const mapped = mapRefundToStateSet(latestRefund);
      if (!commerce.payments?.refund) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No refund handler' };
      }
      return createOrSkip('refunds', mapped, (data) => commerce.payments.refund(data));
    },

    'charge.dispute.created': async (payload) => {
      const dispute = payload.data?.object || payload;
      const mapped = mapDisputeToStateSet(dispute);
      // Disputes are recorded but don't have a direct commerce handler
      idMapStore.store(platform, 'disputes', mapped.externalId, mapped.externalId, mapped.raw);
      return { action: 'recorded', externalId: mapped.externalId, status: mapped.data.status };
    },

    // --- Customers ---

    'customer.created': async (payload) => {
      const customer = payload.data?.object || payload;
      const mapped = mapCustomerToStateSet(customer);
      if (!commerce.customers?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No customers handler' };
      }
      return createOrSkip('customers', mapped, (data) => commerce.customers.create(data));
    },

    'customer.updated': async (payload) => {
      const customer = payload.data?.object || payload;
      const mapped = mapCustomerToStateSet(customer);
      if (!commerce.customers?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No customers handler' };
      }
      return upsert('customers', mapped, (data) => commerce.customers.create(data));
    },

    // --- Subscriptions ---

    'customer.subscription.created': async (payload) => {
      const sub = payload.data?.object || payload;
      const mapped = mapSubscriptionToStateSet(sub);
      if (!commerce.subscriptions?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'No subscriptions handler',
        };
      }
      return createOrSkip('subscriptions', mapped, (data) => commerce.subscriptions.create(data));
    },

    'customer.subscription.updated': async (payload) => {
      const sub = payload.data?.object || payload;
      const mapped = mapSubscriptionToStateSet(sub);
      if (!commerce.subscriptions?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'No subscriptions handler',
        };
      }
      return upsert('subscriptions', mapped, (data) => commerce.subscriptions.create(data));
    },

    'customer.subscription.deleted': async (payload) => {
      const sub = payload.data?.object || payload;
      const externalId = sub.id;
      const existing = idMapStore.lookup(platform, 'subscriptions', externalId);
      if (existing && commerce.subscriptions?.cancel) {
        await commerce.subscriptions.cancel(existing.statesetId);
        return { action: 'cancelled', externalId, statesetId: existing.statesetId };
      }
      return { action: 'skipped', externalId, reason: 'Subscription not found in id_map' };
    },

    // --- Invoices ---

    'invoice.paid': async (payload) => {
      const invoice = payload.data?.object || payload;
      const mapped = mapInvoiceToStateSet(invoice);
      if (!commerce.invoices?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No invoices handler' };
      }
      return upsert('invoices', mapped, (data) => commerce.invoices.create(data));
    },

    'invoice.payment_failed': async (payload) => {
      const invoice = payload.data?.object || payload;
      const mapped = mapInvoiceToStateSet(invoice);
      const existing = idMapStore.lookup(platform, 'invoices', mapped.externalId);
      if (existing) {
        idMapStore.store(platform, 'invoices', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'updated',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
          status: 'failed',
        };
      }
      return { action: 'recorded', externalId: mapped.externalId, status: 'failed' };
    },
  };
}

/**
 * Get the list of supported Stripe webhook event types.
 * @returns {string[]}
 */
export function getSupportedStripeEvents() {
  return [
    'payment_intent.succeeded',
    'payment_intent.payment_failed',
    'payment_intent.canceled',
    'charge.succeeded',
    'charge.refunded',
    'charge.dispute.created',
    'customer.created',
    'customer.updated',
    'customer.subscription.created',
    'customer.subscription.updated',
    'customer.subscription.deleted',
    'invoice.paid',
    'invoice.payment_failed',
  ];
}
