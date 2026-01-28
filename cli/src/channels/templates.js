/**
 * Message Templates for StateSet Channel Gateways
 *
 * Pre-built rich message templates for common commerce scenarios.
 * These provide consistent, well-formatted notifications across all channels.
 */

// ============================================================================
// Order templates
// ============================================================================

/**
 * Order confirmation template — sent when an order is placed.
 *
 * @param {Object} order
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function orderConfirmation(order) {
  const orderNum = order.orderNumber || order.order_number || order.id;
  return {
    title: `Order Confirmed: ${orderNum}`,
    description: 'Thank you for your order! We\'re processing it now.',
    color: '#4CAF50',
    fields: [
      { name: 'Order', value: orderNum, inline: true },
      { name: 'Total', value: `$${(order.total || 0).toFixed(2)}`, inline: true },
      { name: 'Items', value: String(order.items?.length || 0), inline: true },
    ],
    footer: 'StateSet Commerce',
  };
}

/**
 * Shipping update template — sent when an order ships.
 *
 * @param {Object} order
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function shippingUpdate(order) {
  const orderNum = order.orderNumber || order.order_number || order.id;
  const tracking = order.trackingNumber || order.tracking_number;

  const fields = [
    { name: 'Order', value: orderNum, inline: true },
    { name: 'Status', value: 'SHIPPED', inline: true },
  ];

  if (tracking) {
    fields.push({ name: 'Tracking', value: tracking, inline: true });
  }

  const buttons = [];
  if (tracking) {
    buttons.push({ label: 'Track Package', action: `track:${order.id}` });
  }

  return {
    title: `Your order ${orderNum} has shipped!`,
    color: '#2196F3',
    fields,
    buttons,
    footer: 'StateSet Commerce',
  };
}

/**
 * Delivery confirmation template.
 *
 * @param {Object} order
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function deliveryConfirmation(order) {
  const orderNum = order.orderNumber || order.order_number || order.id;
  return {
    title: `Order ${orderNum} Delivered`,
    description: 'Your order has been delivered. We hope you enjoy your purchase!',
    color: '#00BCD4',
    fields: [
      { name: 'Order', value: orderNum, inline: true },
      { name: 'Status', value: 'DELIVERED', inline: true },
    ],
    footer: 'StateSet Commerce',
  };
}

// ============================================================================
// Return templates
// ============================================================================

/**
 * Return request received template.
 *
 * @param {Object} returnReq
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function returnReceived(returnReq) {
  return {
    title: `Return Request: ${returnReq.id}`,
    description: 'We\'ve received your return request and will review it shortly.',
    color: '#FF9800',
    fields: [
      { name: 'Status', value: 'PENDING REVIEW', inline: true },
      { name: 'Reason', value: returnReq.reason || 'Not specified', inline: true },
    ],
    footer: 'StateSet Commerce',
  };
}

/**
 * Return approved template.
 *
 * @param {Object} returnReq
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function returnApproved(returnReq) {
  return {
    title: `Return Approved: ${returnReq.id}`,
    description: 'Your return has been approved. Please ship the item back using the provided label.',
    color: '#4CAF50',
    fields: [
      { name: 'Status', value: 'APPROVED', inline: true },
      { name: 'Refund', value: returnReq.refundAmount ? `$${returnReq.refundAmount.toFixed(2)}` : 'Pending', inline: true },
    ],
    footer: 'StateSet Commerce',
  };
}

// ============================================================================
// Cart templates
// ============================================================================

/**
 * Abandoned cart reminder template.
 *
 * @param {Object} cart
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function abandonedCartReminder(cart) {
  const cartNum = cart.cartNumber || cart.cart_number || cart.id;
  const itemCount = cart.items?.length || 0;
  const itemList = (cart.items || []).slice(0, 3)
    .map((i) => `${i.quantity || 1}x ${i.name || i.sku}`)
    .join(', ');

  return {
    title: 'You left something in your cart!',
    description: itemList ? `Your items: ${itemList}${itemCount > 3 ? ` and ${itemCount - 3} more` : ''}` : 'You have items waiting in your cart.',
    color: '#9C27B0',
    fields: [
      { name: 'Cart', value: cartNum, inline: true },
      { name: 'Items', value: String(itemCount), inline: true },
      { name: 'Subtotal', value: `$${(cart.subtotal || 0).toFixed(2)}`, inline: true },
    ],
    buttons: [
      { label: 'View Cart', action: `view_cart:${cart.id}` },
    ],
    footer: 'StateSet Commerce',
  };
}

// ============================================================================
// Inventory templates
// ============================================================================

/**
 * Low stock alert template (for ops channels).
 *
 * @param {Object} data - { sku, name, available, reorderPoint }
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function lowStockAlert(data) {
  return {
    title: `Low Stock: ${data.sku}`,
    description: data.name ? `${data.name} is running low.` : undefined,
    color: '#FF5722',
    fields: [
      { name: 'Available', value: String(data.available ?? 0), inline: true },
      { name: 'Reorder Point', value: String(data.reorderPoint ?? 0), inline: true },
    ],
    footer: 'StateSet Commerce Inventory',
  };
}

/**
 * Back in stock notification template (for customers).
 *
 * @param {Object} data - { sku, name, available }
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function backInStock(data) {
  return {
    title: `Back in Stock: ${data.name || data.sku}`,
    description: `Good news! ${data.name || data.sku} is available again.`,
    color: '#4CAF50',
    fields: [
      { name: 'SKU', value: data.sku, inline: true },
      { name: 'Available', value: String(data.available ?? 0), inline: true },
    ],
    footer: 'StateSet Commerce',
  };
}

// ============================================================================
// Welcome / onboarding
// ============================================================================

/**
 * Welcome message for new users.
 *
 * @param {string} [name] - Customer name
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function welcomeMessage(name) {
  return {
    title: `Welcome${name ? `, ${name}` : ''}!`,
    description: 'I\'m your StateSet Commerce assistant. I can help with orders, inventory, carts, returns, and more.',
    color: '#673AB7',
    fields: [
      { name: 'Get Started', value: 'Just ask me anything, or try /help for commands.' },
    ],
    buttons: [
      { label: 'Help', action: '/help' },
    ],
    footer: 'StateSet Commerce Agent',
  };
}

// ============================================================================
// Approval templates
// ============================================================================

/**
 * Approval request notification template.
 *
 * @param {Object} request - { title, amount, requester, domain }
 * @returns {import('./rich-messages.js').RichMessage}
 */
export function approvalRequest(request) {
  const fields = [
    { name: 'Type', value: request.domain || 'general', inline: true },
  ];

  if (request.amount !== undefined) {
    fields.push({ name: 'Amount', value: `$${Number(request.amount).toFixed(2)}`, inline: true });
  }

  if (request.requester) {
    fields.push({ name: 'Requested By', value: request.requester, inline: true });
  }

  return {
    title: `Approval Required: ${request.title || 'Untitled'}`,
    color: '#FF9800',
    fields,
    footer: 'StateSet Commerce Approvals',
  };
}
