/**
 * WooCommerce <-> StateSet Data Mapper
 *
 * Pure functions that transform between WooCommerce's data model and StateSet's.
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
  return html
    .replace(/<[^>]*>/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

// ---------------------------------------------------------------------------
// Status mappings
// ---------------------------------------------------------------------------

const ORDER_STATUS_MAP = {
  pending: 'pending',
  processing: 'processing',
  'on-hold': 'pending',
  completed: 'shipped',
  cancelled: 'cancelled',
  refunded: 'refunded',
  failed: 'failed',
};

const PRODUCT_STATUS_MAP = {
  publish: 'active',
  draft: 'draft',
  pending: 'pending',
  private: 'active',
};

/**
 * Map WooCommerce order status to StateSet status.
 * @param {string} wooStatus
 * @returns {string}
 */
export function mapOrderStatus(wooStatus) {
  return ORDER_STATUS_MAP[wooStatus] || 'pending';
}

/**
 * Map WooCommerce product status to StateSet status.
 * @param {string} wooStatus
 * @returns {string}
 */
export function mapProductStatus(wooStatus) {
  return PRODUCT_STATUS_MAP[wooStatus] || 'draft';
}

/**
 * Derive payment status from WooCommerce order status.
 * WooCommerce does not expose a separate payment status field,
 * so we infer it from the order status + set_paid flag.
 * @param {Object} wooOrder
 * @returns {string}
 */
export function derivePaymentStatus(wooOrder) {
  const status = wooOrder.status;
  if (status === 'refunded') return 'refunded';
  if (status === 'failed') return 'failed';
  if (status === 'pending') return 'pending';
  if (wooOrder.set_paid || status === 'processing' || status === 'completed') return 'paid';
  return 'pending';
}

// ---------------------------------------------------------------------------
// WooCommerce -> StateSet mappers
// ---------------------------------------------------------------------------

/**
 * Map a WooCommerce customer to StateSet format.
 * @param {Object} wooCustomer
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapCustomerToStateSet(wooCustomer) {
  const c = wooCustomer;
  return {
    entityType: 'customers',
    externalId: String(c.id),
    data: {
      email: c.email || '',
      firstName: c.first_name || '',
      lastName: c.last_name || '',
      phone: c.billing?.phone || null,
      status: 'active',
      billingAddress: c.billing
        ? {
            address1: c.billing.address_1 || '',
            address2: c.billing.address_2 || '',
            city: c.billing.city || '',
            province: c.billing.state || '',
            zip: c.billing.postcode || '',
            country: c.billing.country || '',
            company: c.billing.company || '',
          }
        : null,
      shippingAddress: c.shipping
        ? {
            address1: c.shipping.address_1 || '',
            address2: c.shipping.address_2 || '',
            city: c.shipping.city || '',
            province: c.shipping.state || '',
            zip: c.shipping.postcode || '',
            country: c.shipping.country || '',
            company: c.shipping.company || '',
          }
        : null,
      metadata: {
        woocommerceId: String(c.id),
        woocommerceUsername: c.username || '',
        woocommerceRole: c.role || '',
      },
    },
    raw: c,
  };
}

/**
 * Map a WooCommerce product to StateSet format.
 * @param {Object} wooProduct
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapProductToStateSet(wooProduct) {
  const p = wooProduct;

  const images = (p.images || []).map((img) => ({
    src: img.src || '',
    alt: img.alt || img.name || '',
  }));

  const categories = (p.categories || []).map((cat) => cat.name || cat.slug || '');

  return {
    entityType: 'products',
    externalId: String(p.id),
    data: {
      name: p.name || '',
      description: stripHtml(p.description),
      sku: p.sku || '',
      price: parseFloat(p.price) || 0,
      regularPrice: p.regular_price ? parseFloat(p.regular_price) : null,
      salePrice: p.sale_price ? parseFloat(p.sale_price) : null,
      status: mapProductStatus(p.status),
      images,
      categories,
      metadata: {
        woocommerceId: String(p.id),
        woocommerceSlug: p.slug || '',
        woocommerceType: p.type || 'simple',
      },
    },
    raw: p,
  };
}

/**
 * Map a WooCommerce order to StateSet format.
 * @param {Object} wooOrder
 * @param {Object} [context] - { idMap, platform } for resolving customer references
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapOrderToStateSet(wooOrder, context = {}) {
  const o = wooOrder;

  // Resolve customer ID via idMap
  let customerId = null;
  if (o.customer_id && context.idMap) {
    const mapping = context.idMap.lookup(
      context.platform || 'woocommerce',
      'customers',
      String(o.customer_id),
    );
    customerId = mapping?.statesetId || null;
  }

  const items = (o.line_items || []).map((li) => ({
    sku: li.sku || '',
    name: li.name || '',
    quantity: li.quantity || 1,
    unitPrice: typeof li.price === 'number' ? li.price : parseFloat(li.price) || 0,
    totalPrice: parseFloat(li.total) || 0,
    metadata: {
      woocommerceLineItemId: String(li.id),
      woocommerceProductId: li.product_id ? String(li.product_id) : null,
      woocommerceVariationId: li.variation_id ? String(li.variation_id) : null,
    },
  }));

  const totalAmount = parseFloat(o.total) || items.reduce((s, i) => s + i.totalPrice, 0);

  return {
    entityType: 'orders',
    externalId: String(o.id),
    data: {
      customerId,
      currency: o.currency || 'USD',
      totalAmount,
      orderStatus: mapOrderStatus(o.status),
      paymentStatus: derivePaymentStatus(o),
      items,
      shippingAddress: o.shipping
        ? {
            address1: o.shipping.address_1 || '',
            address2: o.shipping.address_2 || '',
            city: o.shipping.city || '',
            province: o.shipping.state || '',
            zip: o.shipping.postcode || '',
            country: o.shipping.country || '',
          }
        : null,
      billingAddress: o.billing
        ? {
            address1: o.billing.address_1 || '',
            address2: o.billing.address_2 || '',
            city: o.billing.city || '',
            province: o.billing.state || '',
            zip: o.billing.postcode || '',
            country: o.billing.country || '',
          }
        : null,
      metadata: {
        woocommerceId: String(o.id),
        woocommerceOrderNumber: o.number ? String(o.number) : null,
        woocommerceStatus: o.status || null,
        woocommercePaymentMethod: o.payment_method || null,
      },
    },
    raw: o,
  };
}

/**
 * Map a WooCommerce product (inventory view) to StateSet inventory format.
 * @param {Object} wooProduct - WooCommerce product with stock fields
 * @returns {import('../base-adapter.js').MappedRecord}
 */
export function mapInventoryToStateSet(wooProduct) {
  const p = wooProduct;
  const sku = p.sku || (p.id ? `WOO-INV-${p.id}` : '');
  return {
    entityType: 'inventory',
    externalId: String(p.id),
    data: {
      sku,
      quantity: p.stock_quantity !== null && p.stock_quantity !== undefined ? p.stock_quantity : 0,
      stockStatus: p.stock_status || 'instock',
      manageStock: p.manage_stock || false,
      metadata: {
        woocommerceProductId: String(p.id),
        woocommerceStockStatus: p.stock_status || null,
      },
    },
    raw: p,
  };
}

// ---------------------------------------------------------------------------
// StateSet -> WooCommerce mappers (for export / reverse mapping)
// ---------------------------------------------------------------------------

/**
 * Map a StateSet customer back to WooCommerce format.
 * @param {Object} statesetCustomer
 * @returns {Object}
 */
export function mapCustomerFromStateSet(statesetCustomer) {
  const c = statesetCustomer;
  return {
    email: c.email || '',
    first_name: c.firstName || c.first_name || '',
    last_name: c.lastName || c.last_name || '',
    billing: {
      first_name: c.firstName || c.first_name || '',
      last_name: c.lastName || c.last_name || '',
      email: c.email || '',
      phone: c.phone || '',
      address_1: c.billingAddress?.address1 || '',
      address_2: c.billingAddress?.address2 || '',
      city: c.billingAddress?.city || '',
      state: c.billingAddress?.province || '',
      postcode: c.billingAddress?.zip || '',
      country: c.billingAddress?.country || '',
    },
    shipping: {
      first_name: c.firstName || c.first_name || '',
      last_name: c.lastName || c.last_name || '',
      address_1: c.shippingAddress?.address1 || '',
      address_2: c.shippingAddress?.address2 || '',
      city: c.shippingAddress?.city || '',
      state: c.shippingAddress?.province || '',
      postcode: c.shippingAddress?.zip || '',
      country: c.shippingAddress?.country || '',
    },
  };
}

/**
 * Map a StateSet product back to WooCommerce format.
 * @param {Object} statesetProduct
 * @returns {Object}
 */
export function mapProductFromStateSet(statesetProduct) {
  const p = statesetProduct;
  const statusMap = { active: 'publish', draft: 'draft', pending: 'pending' };
  return {
    name: p.name || '',
    description: p.description || '',
    sku: p.sku || '',
    regular_price:
      p.regularPrice !== null && p.regularPrice !== undefined
        ? String(p.regularPrice)
        : String(p.price || 0),
    sale_price: p.salePrice !== null && p.salePrice !== undefined ? String(p.salePrice) : '',
    status: statusMap[p.status] || 'draft',
    images: (p.images || []).map((img) => ({
      src: img.src || '',
      alt: img.alt || '',
    })),
  };
}

// ---------------------------------------------------------------------------
// Dispatch by entity type
// ---------------------------------------------------------------------------

/**
 * Map any entity type to StateSet format.
 * @param {string} entityType
 * @param {Object} record
 * @param {Object} [context]
 * @returns {import('../base-adapter.js').MappedRecord}
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
    default:
      throw new Error(`Unknown entity type: ${entityType}`);
  }
}

/**
 * Map any entity type from StateSet format back to WooCommerce.
 * @param {string} entityType
 * @param {Object} record
 * @returns {Object}
 */
export function mapFromStateSet(entityType, record) {
  switch (entityType) {
    case 'customers':
      return mapCustomerFromStateSet(record);
    case 'products':
      return mapProductFromStateSet(record);
    default:
      return record;
  }
}
