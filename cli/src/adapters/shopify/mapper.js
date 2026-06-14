/**
 * Shopify ↔ StateSet Data Mapper
 *
 * Pure functions that transform between Shopify's data model and StateSet's.
 * No I/O — fully deterministic and trivially testable.
 */

// ---------------------------------------------------------------------------
// HTML stripping
// ---------------------------------------------------------------------------

/**
 * Strip HTML tags from a string (for product descriptions).
 * @param {string} html
 * @returns {string}
 */
export function stripHtml(html) {
  if (!html || typeof html !== 'string') return '';
  return (
    html
      .replace(/<[^>]*>/g, '')
      // Decode `&amp;` LAST so an already-escaped entity such as `&amp;lt;`
      // resolves to the literal text `&lt;` rather than being double-unescaped
      // to `<` (CodeQL js/double-escaping). Output is plain text for a product
      // description (never rendered as raw HTML), so decoded `<`/`>` stay literal.
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'")
      .replace(/&nbsp;/g, ' ')
      .replace(/&amp;/g, '&')
      .replace(/\s+/g, ' ')
      .trim()
  );
}

// ---------------------------------------------------------------------------
// Status mappings
// ---------------------------------------------------------------------------

const CUSTOMER_STATUS_MAP = {
  enabled: 'active',
  disabled: 'inactive',
  invited: 'pending',
  declined: 'inactive',
};

const FINANCIAL_STATUS_MAP = {
  pending: 'pending',
  authorized: 'pending',
  paid: 'paid',
  partially_paid: 'pending',
  partially_refunded: 'partially_refunded',
  refunded: 'refunded',
  voided: 'refunded',
};

const FULFILLMENT_STATUS_MAP = {
  null: 'pending',
  unfulfilled: 'pending',
  partial: 'processing',
  fulfilled: 'shipped',
  restocked: 'cancelled',
};

/**
 * Map Shopify customer state to StateSet status.
 */
export function mapCustomerStatus(shopifyState) {
  return CUSTOMER_STATUS_MAP[shopifyState] || 'active';
}

/**
 * Map Shopify financial_status to StateSet payment status.
 */
export function mapFinancialStatus(shopifyStatus) {
  return FINANCIAL_STATUS_MAP[shopifyStatus] || 'pending';
}

/**
 * Map Shopify fulfillment_status to StateSet fulfillment status.
 */
export function mapFulfillmentStatus(shopifyStatus) {
  return FULFILLMENT_STATUS_MAP[shopifyStatus || 'null'] || 'pending';
}

// ---------------------------------------------------------------------------
// Shopify → StateSet mappers
// ---------------------------------------------------------------------------

/**
 * Map a Shopify customer to StateSet format.
 * @param {Object} shopifyCustomer
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapCustomerToStateSet(shopifyCustomer) {
  const c = shopifyCustomer;
  return {
    entityType: 'customers',
    externalId: String(c.id),
    data: {
      email: c.email || '',
      firstName: c.first_name || '',
      lastName: c.last_name || '',
      phone: c.phone || null,
      status: mapCustomerStatus(c.state),
      acceptsMarketing: c.accepts_marketing || false,
      metadata: {
        shopifyId: String(c.id),
        shopifyTags: c.tags || '',
        shopifyNote: c.note || '',
      },
    },
    raw: c,
  };
}

/**
 * Map a Shopify product (with variants) to StateSet format.
 * @param {Object} shopifyProduct
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapProductToStateSet(shopifyProduct) {
  const p = shopifyProduct;

  const variants = (p.variants || []).map((v) => ({
    sku: v.sku || '',
    name: v.title || 'Default',
    price: parseFloat(v.price) || 0,
    compareAtPrice: v.compare_at_price ? parseFloat(v.compare_at_price) : null,
    weight: v.weight ? parseFloat(v.weight) : null,
    weightUnit: v.weight_unit || null,
    barcode: v.barcode || null,
    metadata: {
      shopifyVariantId: String(v.id),
      shopifyInventoryItemId: v.inventory_item_id ? String(v.inventory_item_id) : null,
    },
  }));

  return {
    entityType: 'products',
    externalId: String(p.id),
    data: {
      name: p.title || '',
      description: stripHtml(p.body_html),
      slug: p.handle || '',
      status: p.status === 'active' ? 'active' : 'draft',
      productType: p.product_type || null,
      vendor: p.vendor || null,
      tags: p.tags ? p.tags.split(',').map((t) => t.trim()) : [],
      variants,
      metadata: {
        shopifyId: String(p.id),
        shopifyHandle: p.handle || '',
      },
    },
    raw: p,
  };
}

/**
 * Map a Shopify order to StateSet format.
 * @param {Object} shopifyOrder
 * @param {Object} [context] - { idMap, platform } for resolving customer references
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapOrderToStateSet(shopifyOrder, context = {}) {
  const o = shopifyOrder;

  // Resolve customer ID via idMap
  let customerId = null;
  if (o.customer?.id && context.idMap) {
    const mapping = context.idMap.lookup(
      context.platform || 'shopify',
      'customers',
      String(o.customer.id),
    );
    customerId = mapping?.statesetId || null;
  }

  const items = (o.line_items || []).map((li) => ({
    sku: li.sku || '',
    name: li.name || li.title || '',
    quantity: li.quantity || 1,
    unitPrice: parseFloat(li.price) || 0,
    totalPrice: parseFloat(li.price) * (li.quantity || 1),
    metadata: {
      shopifyLineItemId: String(li.id),
      shopifyVariantId: li.variant_id ? String(li.variant_id) : null,
      shopifyProductId: li.product_id ? String(li.product_id) : null,
    },
  }));

  const totalAmount = parseFloat(o.total_price) || items.reduce((s, i) => s + i.totalPrice, 0);

  return {
    entityType: 'orders',
    externalId: String(o.id),
    data: {
      customerId,
      currency: o.currency || 'USD',
      totalAmount,
      paymentStatus: mapFinancialStatus(o.financial_status),
      fulfillmentStatus: mapFulfillmentStatus(o.fulfillment_status),
      items,
      shippingAddress: o.shipping_address
        ? {
            address1: o.shipping_address.address1 || '',
            address2: o.shipping_address.address2 || '',
            city: o.shipping_address.city || '',
            province: o.shipping_address.province || '',
            zip: o.shipping_address.zip || '',
            country: o.shipping_address.country || '',
          }
        : null,
      metadata: {
        shopifyId: String(o.id),
        shopifyOrderNumber: o.order_number ? String(o.order_number) : null,
        shopifyFinancialStatus: o.financial_status || null,
        shopifyFulfillmentStatus: o.fulfillment_status || null,
      },
    },
    raw: o,
  };
}

/**
 * Map a Shopify inventory level to StateSet format.
 * @param {Object} shopifyInventory - { inventory_item_id, sku, available, ... }
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapInventoryToStateSet(shopifyInventory) {
  const inv = shopifyInventory;
  const fallbackSku =
    inv.sku ||
    (inv.inventory_item_id || inv.id ? `SHOPIFY-INV-${inv.inventory_item_id || inv.id}` : '');
  return {
    entityType: 'inventory',
    externalId: String(inv.inventory_item_id || inv.id),
    data: {
      sku: fallbackSku,
      quantity: inv.available !== null && inv.available !== undefined ? inv.available : 0,
      metadata: {
        shopifyInventoryItemId: String(inv.inventory_item_id || inv.id),
        shopifyLocationId: inv.location_id ? String(inv.location_id) : null,
      },
    },
    raw: inv,
  };
}

/**
 * Map a Shopify fulfillment to a StateSet shipment-compatible payload.
 * @param {Object} shopifyFulfillment
 * @param {Object} [context] - { idMap, platform } for resolving order references
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapFulfillmentToStateSet(shopifyFulfillment, context = {}) {
  const fulfillment = shopifyFulfillment || {};
  const trackingNumber =
    fulfillment.tracking_number ||
    (Array.isArray(fulfillment.tracking_numbers) ? fulfillment.tracking_numbers[0] : null) ||
    null;
  const sourceOrderId =
    fulfillment.order_id !== null && fulfillment.order_id !== undefined
      ? String(fulfillment.order_id)
      : null;

  let orderId = null;
  if (sourceOrderId && context.idMap) {
    const orderMapping = context.idMap.lookup(
      context.platform || 'shopify',
      'orders',
      sourceOrderId,
    );
    orderId = orderMapping?.statesetId || null;
  }

  const statusRaw = String(fulfillment.status || '').toLowerCase();
  let status = 'pending';
  if (statusRaw === 'success') status = 'shipped';
  if (statusRaw === 'cancelled') status = 'cancelled';

  return {
    entityType: 'fulfillments',
    externalId: String(fulfillment.id),
    data: {
      orderId,
      carrier:
        fulfillment.tracking_company ||
        fulfillment.shipment_status ||
        fulfillment.service ||
        'shopify',
      trackingNumber,
      status,
      metadata: {
        shopifyFulfillmentId: String(fulfillment.id),
        shopifyOrderId: sourceOrderId,
        shopifyStatus: fulfillment.status || null,
      },
    },
    raw: fulfillment,
  };
}

// ---------------------------------------------------------------------------
// StateSet → Shopify mappers (for export / parity testing)
// ---------------------------------------------------------------------------

/**
 * Map a StateSet customer back to Shopify format.
 */
export function mapCustomerFromStateSet(statesetCustomer) {
  const c = statesetCustomer;
  return {
    email: c.email,
    first_name: c.firstName || c.first_name || '',
    last_name: c.lastName || c.last_name || '',
    phone: c.phone || null,
    state: c.status === 'active' ? 'enabled' : 'disabled',
    accepts_marketing: c.acceptsMarketing || c.accepts_marketing || false,
  };
}

/**
 * Map a StateSet product back to Shopify format.
 */
export function mapProductFromStateSet(statesetProduct) {
  const p = statesetProduct;
  return {
    title: p.name,
    body_html: p.description || '',
    handle: p.slug || '',
    status: p.status === 'active' ? 'active' : 'draft',
    product_type: p.productType || p.product_type || '',
    vendor: p.vendor || '',
    variants: (p.variants || []).map((v) => ({
      sku: v.sku || '',
      title: v.name || 'Default',
      price: String(v.price || 0),
      compare_at_price: v.compareAtPrice ? String(v.compareAtPrice) : null,
    })),
  };
}

/**
 * Map a StateSet shipment back to Shopify-like fulfillment format.
 */
export function mapFulfillmentFromStateSet(statesetShipment) {
  const shipment = statesetShipment || {};
  return {
    tracking_number: shipment.trackingNumber || shipment.tracking_number || null,
    tracking_company: shipment.carrier || shipment.tracking_company || null,
    status: shipment.status || 'pending',
  };
}

// ---------------------------------------------------------------------------
// Dispatch by entity type
// ---------------------------------------------------------------------------

/**
 * Map any entity type to StateSet format.
 */
export function mapToStateSet(entityType, record, context = {}) {
  switch (entityType) {
    case 'customers':
      return mapCustomerToStateSet(record);
    case 'products':
      return mapProductToStateSet(record);
    case 'orders':
      return mapOrderToStateSet(record, context);
    case 'inventory':
      return mapInventoryToStateSet(record);
    case 'fulfillments':
      return mapFulfillmentToStateSet(record, context);
    default:
      throw new Error(`Unknown entity type: ${entityType}`);
  }
}

/**
 * Map any entity type from StateSet format back to Shopify.
 */
export function mapFromStateSet(entityType, record) {
  switch (entityType) {
    case 'customers':
      return mapCustomerFromStateSet(record);
    case 'products':
      return mapProductFromStateSet(record);
    case 'fulfillments':
      return mapFulfillmentFromStateSet(record);
    default:
      return record;
  }
}
