/**
 * Rich Messages for StateSet Channel Gateways
 *
 * Provides a platform-agnostic rich message format and builder functions
 * for common commerce data (orders, carts, inventory, analytics).
 *
 * Each channel gateway implements sendRichMessage() to convert to the
 * native format (Embeds, Block Kit, HTML, etc.). Channels without rich
 * support use richMessageToPlainText() as a fallback.
 */

// ============================================================================
// RichMessage type (documented via JSDoc)
// ============================================================================

/**
 * @typedef {Object} RichMessage
 * @property {string}  title
 * @property {string}  [description]
 * @property {string}  [color]         - Hex color e.g. '#FF5733'
 * @property {{ name: string, value: string, inline?: boolean }[]} [fields]
 * @property {string}  [imageUrl]
 * @property {{ label: string, url?: string, action?: string }[]} [buttons]
 * @property {string}  [footer]
 */

// ============================================================================
// Builder functions
// ============================================================================

/**
 * Build an order summary card.
 *
 * @param {Object} order - Order object from Commerce API
 * @returns {RichMessage}
 */
export function createOrderSummary(order) {
  const statusColors = {
    pending: '#FFA500',
    confirmed: '#2196F3',
    processing: '#9C27B0',
    shipped: '#4CAF50',
    delivered: '#00BCD4',
    cancelled: '#F44336',
    refunded: '#795548',
  };

  const color = statusColors[order.status] || '#607D8B';

  const fields = [
    { name: 'Status', value: (order.status || 'unknown').toUpperCase(), inline: true },
    { name: 'Total', value: `$${(order.total || 0).toFixed(2)}`, inline: true },
  ];

  if (order.customerEmail || order.customer_email) {
    fields.push({
      name: 'Customer',
      value: order.customerEmail || order.customer_email,
      inline: true,
    });
  }

  if (order.items && order.items.length > 0) {
    const itemList = order.items
      .slice(0, 5)
      .map(
        (i) =>
          `${i.quantity || 1}x ${i.name || i.sku} ($${(i.unitPrice || i.unit_price || 0).toFixed(2)})`,
      )
      .join('\n');
    fields.push({
      name: 'Items',
      value: itemList + (order.items.length > 5 ? `\n...and ${order.items.length - 5} more` : ''),
    });
  }

  if (order.trackingNumber || order.tracking_number) {
    fields.push({
      name: 'Tracking',
      value: order.trackingNumber || order.tracking_number,
      inline: true,
    });
  }

  const createdAt = order.createdAt || order.created_at;
  if (createdAt) {
    fields.push({ name: 'Date', value: new Date(createdAt).toLocaleDateString(), inline: true });
  }

  return {
    title: `Order ${order.orderNumber || order.order_number || order.id}`,
    color,
    fields,
    footer: 'StateSet Commerce',
  };
}

/**
 * Build a multi-order list card.
 *
 * @param {Object[]} orders
 * @returns {RichMessage}
 */
export function createOrderList(orders) {
  const fields = orders.slice(0, 10).map((o) => ({
    name: o.orderNumber || o.order_number || o.id,
    value: `${(o.status || 'unknown').toUpperCase()} — $${(o.total || 0).toFixed(2)}`,
    inline: true,
  }));

  return {
    title: `Orders (${orders.length})`,
    color: '#2196F3',
    fields,
    footer: orders.length > 10 ? `Showing 10 of ${orders.length}` : 'StateSet Commerce',
  };
}

/**
 * Build an inventory card for a SKU.
 *
 * @param {string} sku
 * @param {Object} stock - Stock object from Commerce API
 * @returns {RichMessage}
 */
export function createInventoryCard(sku, stock) {
  const available = stock.available ?? stock.quantity ?? 0;
  const reserved = stock.reserved ?? 0;
  const reorderPoint = stock.reorderPoint ?? stock.reorder_point ?? 0;

  const color = available <= 0 ? '#F44336' : available <= reorderPoint ? '#FFA500' : '#4CAF50';

  const fields = [
    { name: 'Available', value: String(available), inline: true },
    { name: 'Reserved', value: String(reserved), inline: true },
  ];

  if (reorderPoint > 0) {
    fields.push({ name: 'Reorder Point', value: String(reorderPoint), inline: true });
  }

  if (stock.name) {
    fields.push({ name: 'Name', value: stock.name, inline: true });
  }

  return {
    title: `Inventory: ${sku}`,
    color,
    fields,
    footer: available <= 0 ? 'OUT OF STOCK' : available <= reorderPoint ? 'LOW STOCK' : 'In Stock',
  };
}

/**
 * Build a cart summary card.
 *
 * @param {Object} cart - Cart object from Commerce API
 * @returns {RichMessage}
 */
export function createCartSummary(cart) {
  const fields = [
    { name: 'Status', value: (cart.status || 'active').toUpperCase(), inline: true },
    { name: 'Subtotal', value: `$${(cart.subtotal || 0).toFixed(2)}`, inline: true },
  ];

  if (cart.customerEmail || cart.customer_email) {
    fields.push({
      name: 'Customer',
      value: cart.customerEmail || cart.customer_email,
      inline: true,
    });
  }

  if (cart.items && cart.items.length > 0) {
    const itemList = cart.items
      .slice(0, 5)
      .map(
        (i) =>
          `${i.quantity || 1}x ${i.name || i.sku} ($${(i.unitPrice || i.unit_price || 0).toFixed(2)})`,
      )
      .join('\n');
    fields.push({
      name: 'Items',
      value: itemList + (cart.items.length > 5 ? `\n...and ${cart.items.length - 5} more` : ''),
    });
  }

  return {
    title: `Cart ${cart.cartNumber || cart.cart_number || cart.id}`,
    color: '#9C27B0',
    fields,
    footer: 'StateSet Commerce',
  };
}

/**
 * Build an analytics summary card.
 *
 * @param {Object} summary - Sales summary from Commerce API
 * @returns {RichMessage}
 */
export function createAnalyticsSummary(summary) {
  const fields = [
    { name: 'Revenue', value: `$${(summary.totalRevenue || 0).toFixed(2)}`, inline: true },
    { name: 'Orders', value: String(summary.orderCount || 0), inline: true },
    { name: 'Avg Order', value: `$${(summary.averageOrderValue || 0).toFixed(2)}`, inline: true },
  ];

  if (summary.itemsSold !== undefined) {
    fields.push({ name: 'Items Sold', value: String(summary.itemsSold), inline: true });
  }

  if (summary.uniqueCustomers !== undefined) {
    fields.push({ name: 'Customers', value: String(summary.uniqueCustomers), inline: true });
  }

  return {
    title: 'Sales Summary',
    color: '#4CAF50',
    fields,
    footer: 'StateSet Commerce Analytics',
  };
}

// ============================================================================
// Plain-text fallback
// ============================================================================

/**
 * Convert a RichMessage to plain text for channels without rich support.
 *
 * @param {RichMessage} msg
 * @returns {string}
 */
export function richMessageToPlainText(msg) {
  const lines = [];

  lines.push(`*${msg.title}*`);

  if (msg.description) {
    lines.push(msg.description);
  }

  lines.push('');

  if (msg.fields && msg.fields.length > 0) {
    for (const field of msg.fields) {
      lines.push(`${field.name}: ${field.value}`);
    }
  }

  if (msg.buttons && msg.buttons.length > 0) {
    lines.push('');
    for (const btn of msg.buttons) {
      if (btn.url) {
        lines.push(`${btn.label}: ${btn.url}`);
      } else {
        lines.push(`[${btn.label}]`);
      }
    }
  }

  if (msg.footer) {
    lines.push('');
    lines.push(`— ${msg.footer}`);
  }

  return lines.join('\n');
}
