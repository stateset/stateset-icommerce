/**
 * Shopify Webhook Event Handlers
 *
 * Processes Shopify webhook payloads and creates/updates records in StateSet.
 * Each handler follows: validate → map → id_map check → commerce write → id_map store.
 */

import {
  mapCustomerToStateSet,
  mapFulfillmentToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
} from './mapper.js';
import { decimal } from '../../tools/providers/money.js';
import { deterministicHash } from '../../tools/providers/runtime.js';

function previousPayload(mapping) {
  if (!mapping?.externalData) return {};
  return typeof mapping.externalData === 'string'
    ? JSON.parse(mapping.externalData)
    : mapping.externalData;
}

function isStale(mapping, payload) {
  const previous = Date.parse(previousPayload(mapping).updated_at);
  const incoming = Date.parse(payload.updated_at);
  return Number.isFinite(previous) && Number.isFinite(incoming) && incoming < previous;
}

function patchFields(payload, mapped, fields) {
  return Object.fromEntries(
    Object.entries(fields)
      .filter(([external]) => Object.hasOwn(payload, external))
      .map(([, internal]) => [internal, mapped[internal]]),
  );
}

function orderEconomics(payload) {
  return {
    currency: payload.currency,
    total_price:
      payload.total_price === undefined ? undefined : decimal(payload.total_price).toFixed(),
    line_items: payload.line_items?.map((item) => ({
      id: String(item.id),
      sku: item.sku,
      quantity: item.quantity,
      price: decimal(item.price).toFixed(),
      discount: decimal(item.total_discount ?? '0').toFixed(),
      taxes: item.tax_lines?.map((tax) => ({
        title: tax.title,
        price: decimal(tax.price).toFixed(),
      })),
    })),
  };
}

function assertUnchangedOrderEconomics(existing, payload) {
  const previous = orderEconomics(previousPayload(existing));
  const incoming = orderEconomics(payload);
  for (const field of ['currency', 'total_price', 'line_items']) {
    if (
      previous[field] !== undefined &&
      incoming[field] !== undefined &&
      deterministicHash(previous[field]) !== deterministicHash(incoming[field])
    ) {
      throw new Error(
        `Shopify order ${field} changed; native order editing and financial reconciliation are required`,
      );
    }
  }
}

/**
 * Create a set of Shopify webhook handlers.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {import('../id-map-store.js').IdMapStore} idMapStore
 * @returns {Object<string, (payload: Object) => Promise<Object>>}
 */
export function createShopifyWebhookHandlers(commerce, idMapStore) {
  const platform = 'shopify';

  async function updateMapped(entityType, mapped, existing, update) {
    if (isStale(existing, mapped.raw)) {
      return {
        action: 'skipped',
        reason: 'stale_event',
        externalId: mapped.externalId,
        statesetId: existing.statesetId,
      };
    }
    const updatedFields = await update(existing.statesetId);
    // Never advance the external snapshot when a native write failed. Replays
    // can repeat completed field assignments after a later operation fails.
    idMapStore.store(platform, entityType, mapped.externalId, existing.statesetId, {
      ...previousPayload(existing),
      ...mapped.raw,
    });
    return {
      action: updatedFields.length ? 'updated' : 'unchanged',
      externalId: mapped.externalId,
      statesetId: existing.statesetId,
      updatedFields,
    };
  }

  async function syncVariants(productId, mapped) {
    if (!Object.hasOwn(mapped.raw, 'variants') || mapped.data.variants.length === 0) return [];
    const current = await commerce.products.getVariants(productId);
    const previous = previousPayload(idMapStore.lookup(platform, 'products', mapped.externalId));
    for (const variant of mapped.data.variants) {
      const externalId = variant.metadata.shopifyVariantId;
      if (externalId === 'undefined' || !variant.sku)
        throw new Error('Shopify variants require an ID and SKU');
      const known = idMapStore.lookup(platform, 'variants', externalId);
      const oldSku = previous.variants?.find((entry) => String(entry.id) === externalId)?.sku;
      const match = known
        ? current.find((entry) => entry.id === known.statesetId)
        : current.find((entry) => entry.sku === (oldSku || variant.sku));
      if (known && !match)
        throw new Error(`Shopify variant ${externalId} does not belong to product ${productId}`);
      const result = match
        ? await commerce.products.updateVariant(match.id, variant)
        : await commerce.products.addVariant(productId, variant);
      idMapStore.store(platform, 'variants', externalId, result.id, variant);
    }
    return ['variants'];
  }

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
        return updateMapped('customers', mapped, existing, async (id) => {
          const patch = patchFields(payload, mapped.data, {
            email: 'email',
            first_name: 'firstName',
            last_name: 'lastName',
            phone: 'phone',
            state: 'status',
            accepts_marketing: 'acceptsMarketing',
          });
          if (Object.hasOwn(patch, 'phone') && payload.phone === null) {
            const current = await commerce.customers.get(id);
            if (!current) throw new Error(`Customer ${id} not found`);
            if (current.phone)
              throw new Error('Clearing a customer phone requires a native nullable-field update');
            delete patch.phone;
          }
          if (Object.keys(patch).length) await commerce.customers.update(id, patch);
          return Object.keys(patch);
        });
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
        return updateMapped('products', mapped, existing, async (id) => {
          const patch = patchFields(payload, mapped.data, {
            title: 'name',
            handle: 'slug',
            body_html: 'description',
            status: 'status',
          });
          if (Object.keys(patch).length) await commerce.products.update(id, patch);
          return [...Object.keys(patch), ...(await syncVariants(id, mapped))];
        });
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
        return updateMapped('orders', mapped, existing, async (id) => {
          assertUnchangedOrderEconomics(existing, payload);
          const patch = {};
          if (Object.hasOwn(payload, 'financial_status'))
            patch.paymentStatus = mapped.data.paymentStatus;
          if (Object.hasOwn(payload, 'fulfillment_status')) {
            const statuses = {
              unfulfilled: 'unfulfilled',
              partial: 'partially_fulfilled',
              fulfilled: 'fulfilled',
              restocked: 'unfulfilled',
            };
            patch.fulfillmentStatus =
              payload.fulfillment_status === null
                ? 'unfulfilled'
                : statuses[payload.fulfillment_status];
            if (!patch.fulfillmentStatus)
              throw new Error(
                `Unsupported Shopify fulfillment status: ${payload.fulfillment_status}`,
              );
          }
          if (payload.cancelled_at) patch.status = 'cancelled';
          if (Object.hasOwn(payload, 'note')) patch.notes = payload.note || '';
          if (payload.shipping_address) {
            const address = payload.shipping_address;
            patch.shippingAddress = {
              line1: address.address1 || '',
              line2: address.address2 || undefined,
              city: address.city || '',
              state: address.province_code || address.province || undefined,
              postalCode: address.zip || '',
              country: address.country_code || address.country || '',
            };
          }
          if (Object.keys(patch).length) await commerce.orders.update(id, patch);
          return Object.keys(patch);
        });
      }
      return createOrSkip('orders', mapped, (data) => commerce.orders.create(data));
    },

    'fulfillments/create': async (payload) => {
      const mapped = mapFulfillmentToStateSet(payload, { idMap: idMapStore, platform });
      if (!commerce.shipments?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'Shipments create handler is unavailable',
        };
      }
      return createOrSkip('fulfillments', mapped, (data) => commerce.shipments.create(data));
    },

    'fulfillments/update': async (payload) => {
      const mapped = mapFulfillmentToStateSet(payload, { idMap: idMapStore, platform });
      const existing = idMapStore.lookup(platform, 'fulfillments', mapped.externalId);
      if (existing) {
        return updateMapped('fulfillments', mapped, existing, async (id) => {
          const patch = {};
          if (Object.hasOwn(payload, 'status')) {
            const statuses = {
              pending: 'pending',
              open: 'pending',
              success: 'shipped',
              cancelled: 'cancelled',
            };
            patch.status = statuses[payload.status];
            if (!patch.status)
              throw new Error(`Unsupported Shopify fulfillment status: ${payload.status}`);
          }
          if (payload.shipment_status === 'delivered') patch.status = 'delivered';
          if (mapped.data.trackingNumber) patch.trackingNumber = mapped.data.trackingNumber;
          if (payload.tracking_company) {
            const carrier = payload.tracking_company.toLowerCase();
            patch.carrier = ['ups', 'fedex', 'usps', 'dhl'].includes(carrier) ? carrier : 'other';
          }
          if (Object.keys(patch).length) await commerce.shipments.update(id, patch);
          return Object.keys(patch);
        });
      }
      if (!commerce.shipments?.create) {
        return {
          action: 'skipped',
          externalId: mapped.externalId,
          reason: 'Shipments create handler is unavailable',
        };
      }
      return createOrSkip('fulfillments', mapped, (data) => commerce.shipments.create(data));
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
    'fulfillments/create',
    'fulfillments/update',
    'orders/cancelled',
    'inventory_levels/update',
  ];
}
