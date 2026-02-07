/**
 * Order Commands Module
 *
 * Handles all order-related CLI operations for stateset-direct
 */

/**
 * Execute order commands
 * @param {string} action - The action to perform
 * @param {Array} args - Command arguments
 * @param {Object} options - Command options
 * @returns {Promise<any>} Command result
 */
export async function execute(action, args, { commerce, output, jsonOutput, resolveId }) {
  switch (action) {
    case 'list': {
      const orders = await commerce.orders.list();
      return formatOrderList(orders, { output, jsonOutput });
    }

    case 'get': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: orders get <id>\n\nProvide an order ID or order number.');
      }

      const id = await resolveId(idArg, 'orders');
      const order = await commerce.orders.get(id);

      if (!order) {
        throw new Error(
          `Order not found: ${idArg}\n\nTry 'stateset-direct orders list' to see all orders.`,
        );
      }

      return formatOrderDetail(order, { output, jsonOutput });
    }

    case 'ship': {
      const [orderIdArg, trackingNumber] = args;
      if (!orderIdArg) {
        throw new Error(
          'Usage: orders ship <id> [tracking]\n\nExample: stateset-direct orders ship abc123 FEDEX12345',
        );
      }

      const orderId = await resolveId(orderIdArg, 'orders');
      const order = await commerce.orders.ship(orderId, trackingNumber);

      return formatOrderShipped(order, trackingNumber, { output, jsonOutput });
    }

    case 'cancel': {
      const orderIdArg = args[0];
      if (!orderIdArg) {
        throw new Error('Usage: orders cancel <id>\n\nCancel a pending or confirmed order.');
      }

      const orderId = await resolveId(orderIdArg, 'orders');
      const order = await commerce.orders.cancel(orderId);

      return formatOrderCancelled(order, { output, jsonOutput });
    }

    case 'count': {
      const count = await commerce.orders.count();
      return { count, formatted: `Order count: ${count}` };
    }

    case 'status': {
      const [orderIdArg, newStatus] = args;
      if (!orderIdArg || !newStatus) {
        throw new Error(
          'Usage: orders status <id> <status>\n\n' +
            'Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded',
        );
      }

      const validStatuses = [
        'pending',
        'confirmed',
        'processing',
        'shipped',
        'delivered',
        'cancelled',
        'refunded',
      ];
      if (!validStatuses.includes(newStatus)) {
        throw new Error(
          `Invalid status: ${newStatus}\n\nValid statuses: ${validStatuses.join(', ')}`,
        );
      }

      const orderId = await resolveId(orderIdArg, 'orders');
      const order = await commerce.orders.updateStatus(orderId, newStatus);

      return {
        order,
        formatted: `Order ${order.orderNumber} status updated to: ${newStatus}`,
      };
    }

    case 'pending': {
      const orders = await commerce.orders.list();
      const pending = orders.filter((o) => o.status === 'pending' || o.status === 'confirmed');
      return formatOrderList(pending, { output, jsonOutput });
    }

    case 'recent': {
      const limit = parseInt(args[0], 10) || 10;
      const orders = await commerce.orders.list();
      const recent = orders
        .sort((a, b) => new Date(b.createdAt) - new Date(a.createdAt))
        .slice(0, limit);
      return formatOrderList(recent, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: orders ${action}\n\n` +
          'Available actions:\n' +
          '  list              List all orders\n' +
          '  get <id>          Get order details\n' +
          '  ship <id> [tracking]  Ship an order\n' +
          '  cancel <id>       Cancel an order\n' +
          '  status <id> <status>  Update order status\n' +
          '  count             Count orders\n' +
          '  pending           List pending orders\n' +
          '  recent [n]        List n most recent orders',
      );
  }
}

/**
 * Format order list for output
 */
function formatOrderList(orders, { output, jsonOutput }) {
  if (jsonOutput) {
    return orders;
  }

  if (orders.length === 0) {
    return { formatted: 'No orders found.' };
  }

  const formatted = output.table(
    orders.map((o) => ({
      id: o.id.slice(0, 8) + '...',
      number: o.orderNumber,
      status: o.status,
      total: `${o.currency} ${o.totalAmount.toFixed(2)}`,
      items: o.items?.length || 0,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'number', header: 'Order #' },
      { key: 'status', header: 'Status' },
      { key: 'total', header: 'Total', align: 'right' },
      { key: 'items', header: 'Items', align: 'right' },
    ],
  );

  return { orders, formatted };
}

/**
 * Format single order detail
 */
function formatOrderDetail(order, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return order;
  }

  const itemLines =
    order.items
      ?.map(
        (i) =>
          `  - ${i.name} (${i.sku}) x${i.quantity} @ ${order.currency} ${i.unitPrice.toFixed(2)}`,
      )
      .join('\n') || '  (no items)';

  const formatted = `
Order: ${order.orderNumber}
${'-'.repeat(40)}
ID:          ${order.id}
Status:      ${order.status}
Total:       ${order.currency} ${order.totalAmount.toFixed(2)}
Payment:     ${order.paymentStatus}
Fulfillment: ${order.fulfillmentStatus}
Tracking:    ${order.trackingNumber || 'N/A'}
Created:     ${order.createdAt}

Items:
${itemLines}
`;

  return { order, formatted };
}

/**
 * Format order shipped response
 */
function formatOrderShipped(order, trackingNumber, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, order };
  }

  const trackingInfo = trackingNumber ? ` with tracking: ${trackingNumber}` : '';
  return {
    order,
    formatted: `Order ${order.orderNumber} shipped${trackingInfo}`,
  };
}

/**
 * Format order cancelled response
 */
function formatOrderCancelled(order, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, order };
  }

  return {
    order,
    formatted: `Order ${order.orderNumber} cancelled`,
  };
}

/**
 * Command metadata for help/completion
 */
export const metadata = {
  name: 'orders',
  aliases: ['o', 'ord'],
  description: 'Order management commands',
  actions: {
    list: { description: 'List all orders', args: [] },
    get: { description: 'Get order by ID', args: ['<id>'] },
    ship: { description: 'Ship an order', args: ['<id>', '[tracking]'] },
    cancel: { description: 'Cancel an order', args: ['<id>'] },
    status: { description: 'Update order status', args: ['<id>', '<status>'] },
    count: { description: 'Count orders', args: [] },
    pending: { description: 'List pending orders', args: [] },
    recent: { description: 'List recent orders', args: ['[count]'] },
  },
};

export default { execute, metadata };
