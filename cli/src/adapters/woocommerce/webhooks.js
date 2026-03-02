/**
 * WooCommerce Webhook Event Handlers
 *
 * Processes WooCommerce webhook payloads and creates/updates records in StateSet.
 * Each handler follows: validate -> map -> id_map check -> commerce write -> id_map store.
 *
 * WooCommerce webhooks sign the payload with HMAC-SHA256 (base64-encoded)
 * and deliver the signature in the `X-WC-Webhook-Signature` header.
 */

import crypto from 'crypto';
import { mapCustomerToStateSet, mapProductToStateSet, mapOrderToStateSet } from './mapper.js';

/**
 * Verify a WooCommerce webhook signature.
 *
 * WooCommerce signs the raw JSON body with HMAC-SHA256 using the webhook secret,
 * and sends the base64-encoded result in the X-WC-Webhook-Signature header.
 *
 * @param {string} rawBody - Raw request body string
 * @param {string} signatureHeader - Value of X-WC-Webhook-Signature header
 * @param {string} secret - Webhook secret (configured in WooCommerce)
 * @returns {{ valid: boolean, error?: string }}
 */
export function verifyWooCommerceSignature(rawBody, signatureHeader, secret) {
  if (!rawBody || typeof rawBody !== 'string') {
    return { valid: false, error: 'Missing or invalid request body' };
  }

  if (!signatureHeader || typeof signatureHeader !== 'string') {
    return { valid: false, error: 'Missing X-WC-Webhook-Signature header' };
  }

  if (!secret || typeof secret !== 'string') {
    return { valid: false, error: 'Missing webhook secret' };
  }

  const expected = crypto.createHmac('sha256', secret).update(rawBody, 'utf-8').digest('base64');

  // Timing-safe comparison
  const expectedBuf = Buffer.from(expected, 'utf-8');
  const receivedBuf = Buffer.from(signatureHeader, 'utf-8');

  if (expectedBuf.length !== receivedBuf.length) {
    return { valid: false, error: 'Signature mismatch' };
  }

  if (!crypto.timingSafeEqual(expectedBuf, receivedBuf)) {
    return { valid: false, error: 'Signature mismatch' };
  }

  return { valid: true };
}

/**
 * Create a set of WooCommerce webhook handlers.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {import('../id-map-store.js').IdMapStore} idMapStore
 * @returns {Object<string, (payload: Object) => Promise<Object>>}
 */
export function createWooCommerceWebhookHandlers(commerce, idMapStore) {
  const platform = 'woocommerce';

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
    'order.created': async (payload) => {
      const mapped = mapOrderToStateSet(payload, { idMap: idMapStore, platform });
      if (!commerce.orders?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No orders handler' };
      }
      return createOrSkip('orders', mapped, (data) => commerce.orders.create(data));
    },

    'order.updated': async (payload) => {
      const mapped = mapOrderToStateSet(payload, { idMap: idMapStore, platform });
      if (!commerce.orders?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No orders handler' };
      }
      return upsert('orders', mapped, (data) => commerce.orders.create(data));
    },

    'product.created': async (payload) => {
      const mapped = mapProductToStateSet(payload);
      if (!commerce.products?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No products handler' };
      }
      return createOrSkip('products', mapped, (data) => commerce.products.create(data));
    },

    'product.updated': async (payload) => {
      const mapped = mapProductToStateSet(payload);
      if (!commerce.products?.create) {
        return { action: 'skipped', externalId: mapped.externalId, reason: 'No products handler' };
      }
      return upsert('products', mapped, (data) => commerce.products.create(data));
    },

    'customer.created': async (payload) => {
      const mapped = mapCustomerToStateSet(payload);
      if (!commerce.customers?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'No customers handler',
        };
      }
      return createOrSkip('customers', mapped, (data) => commerce.customers.create(data));
    },

    'customer.updated': async (payload) => {
      const mapped = mapCustomerToStateSet(payload);
      if (!commerce.customers?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'No customers handler',
        };
      }
      return upsert('customers', mapped, (data) => commerce.customers.create(data));
    },
  };
}

/**
 * Get the list of supported WooCommerce webhook topics.
 * @returns {string[]}
 */
export function getSupportedWooCommerceTopics() {
  return [
    'order.created',
    'order.updated',
    'product.created',
    'product.updated',
    'customer.created',
    'customer.updated',
  ];
}
