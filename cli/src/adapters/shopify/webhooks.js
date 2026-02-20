/**
 * Shopify Webhook Event Handlers
 *
 * Processes Shopify webhook payloads and creates/updates records in StateSet.
 * Each handler follows: validate → map → id_map check → commerce write → id_map store.
 */

import {
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
} from './mapper.js';

/**
 * Create a set of Shopify webhook handlers.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {import('../id-map-store.js').IdMapStore} idMapStore
 * @returns {Object<string, (payload: Object) => Promise<Object>>}
 */
export function createShopifyWebhookHandlers(commerce, idMapStore) {
  const platform = 'shopify';

  /**
   * Helper: create or skip based on id_map.
   */
  async function createOrSkip(entityType, mapped, createFn) {
    const existing = idMapStore.lookup(platform, entityType, mapped.externalId);
    if (existing) {
      return { action: 'skipped', externalId: mapped.externalId, statesetId: existing.statesetId };
    }

    const result = await createFn(mapped.data);
    const statesetId = result.id || result[`${entityType.slice(0, -1)}_id`] || mapped.externalId;

    idMapStore.store(platform, entityType, mapped.externalId, statesetId, mapped.raw);

    return { action: 'created', externalId: mapped.externalId, statesetId };
  }

  return {
    'customers/create': async (payload) => {
      const mapped = mapCustomerToStateSet(payload);
      return createOrSkip('customers', mapped, (data) => commerce.customers.create(data));
    },

    'customers/update': async (payload) => {
      const mapped = mapCustomerToStateSet(payload);
      const existing = idMapStore.lookup(platform, 'customers', mapped.externalId);
      if (existing) {
        // Update existing — store new external data
        idMapStore.store(platform, 'customers', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'updated',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      // Create if not exists
      return createOrSkip('customers', mapped, (data) => commerce.customers.create(data));
    },

    'products/create': async (payload) => {
      const mapped = mapProductToStateSet(payload);
      return createOrSkip('products', mapped, (data) => commerce.products.create(data));
    },

    'products/update': async (payload) => {
      const mapped = mapProductToStateSet(payload);
      const existing = idMapStore.lookup(platform, 'products', mapped.externalId);
      if (existing) {
        idMapStore.store(platform, 'products', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'updated',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      return createOrSkip('products', mapped, (data) => commerce.products.create(data));
    },

    'orders/create': async (payload) => {
      const mapped = mapOrderToStateSet(payload, { idMap: idMapStore, platform });
      return createOrSkip('orders', mapped, (data) => commerce.orders.create(data));
    },

    'orders/updated': async (payload) => {
      const mapped = mapOrderToStateSet(payload, { idMap: idMapStore, platform });
      const existing = idMapStore.lookup(platform, 'orders', mapped.externalId);
      if (existing) {
        idMapStore.store(platform, 'orders', mapped.externalId, existing.statesetId, mapped.raw);
        return {
          action: 'updated',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      return createOrSkip('orders', mapped, (data) => commerce.orders.create(data));
    },

    'orders/cancelled': async (payload) => {
      const externalId = String(payload.id);
      const existing = idMapStore.lookup(platform, 'orders', externalId);
      if (existing && commerce.orders.cancel) {
        await commerce.orders.cancel(existing.statesetId);
        return { action: 'cancelled', externalId, statesetId: existing.statesetId };
      }
      return { action: 'skipped', externalId, reason: 'Order not found in id_map' };
    },

    'inventory_levels/update': async (payload) => {
      const mapped = mapInventoryToStateSet(payload);
      const existing = idMapStore.lookup(platform, 'inventory', mapped.externalId);
      if (existing && commerce.inventory.adjust) {
        await commerce.inventory.adjust({
          sku: mapped.data.sku,
          quantity: mapped.data.quantity,
        });
        return {
          action: 'adjusted',
          externalId: mapped.externalId,
          statesetId: existing.statesetId,
        };
      }
      // Create inventory item if not exists
      if (!existing && commerce.inventory.create) {
        const result = await commerce.inventory.create(mapped.data);
        const statesetId = result.id || mapped.data.sku;
        idMapStore.store(platform, 'inventory', mapped.externalId, statesetId, mapped.raw);
        return { action: 'created', externalId: mapped.externalId, statesetId };
      }
      return { action: 'skipped', externalId: mapped.externalId, reason: 'No inventory handler' };
    },
  };
}

/**
 * Get the list of supported Shopify webhook topics.
 */
export function getSupportedTopics() {
  return [
    'customers/create',
    'customers/update',
    'products/create',
    'products/update',
    'orders/create',
    'orders/updated',
    'orders/cancelled',
    'inventory_levels/update',
  ];
}
