import { z } from 'zod';

const requestedItem = z
  .object({
    orderItemId: z.string().min(1).optional(),
    sku: z.string().min(1).optional(),
    quantity: z.number().int().positive().max(2147483647),
  })
  .strict()
  .refine((item) => item.orderItemId || item.sku, 'Provide orderItemId or sku');

export const partialShipmentPlanSchema = z
  .object({
    orderId: z.string().min(1),
    shipmentId: z.string().min(1).optional(),
    remainingItems: z.array(requestedItem).min(1).optional(),
  })
  .strict();

/** Read-only observation. These quantities are not reservations or permission to ship. */
export async function planPartialShipment(commerce, input) {
  const params = partialShipmentPlanSchema.parse(input);
  const order = await commerce.orders.get(params.orderId);
  if (!order) throw new Error('Order not found');
  if (order.id !== params.orderId) throw new Error('Order identity mismatch');
  if (['cancelled', 'canceled', 'refunded'].includes(order.status)) {
    throw new Error(`Cannot plan fulfillment for ${order.status} order`);
  }
  if (!Array.isArray(order.items) || !Number.isInteger(order.version)) {
    throw new Error('Order snapshot lacks items or version; update the commerce binding');
  }

  const lines = new Map();
  for (const item of order.items) {
    if (
      !item.id ||
      lines.has(item.id) ||
      !Number.isSafeInteger(item.quantity) ||
      item.quantity <= 0 ||
      !Number.isSafeInteger(item.shippedQuantity) ||
      item.shippedQuantity < 0 ||
      item.shippedQuantity > item.quantity
    ) {
      throw new Error(
        'Order snapshot lacks valid per-line fulfillment quantities; reconcile first',
      );
    }
    lines.set(item.id, {
      orderItemId: item.id,
      sku: item.sku,
      name: item.name,
      orderedQuantity: item.quantity,
      shippedQuantity: item.shippedQuantity,
      remainingQuantity: item.quantity - item.shippedQuantity,
    });
  }

  let shipment = null;
  if (params.shipmentId) {
    shipment = await commerce.shipments.get(params.shipmentId);
    if (!shipment) throw new Error('Shipment not found');
    if (shipment.id !== params.shipmentId || shipment.orderId !== order.id) {
      throw new Error('Shipment does not belong to this order');
    }
  }

  const quantities = new Map();
  if (params.remainingItems) {
    for (const requested of params.remainingItems) {
      const matches = requested.orderItemId
        ? [lines.get(requested.orderItemId)].filter(Boolean)
        : [...lines.values()].filter((line) => line.sku === requested.sku);
      if (matches.length !== 1) {
        throw new Error('Requested item is missing or SKU is ambiguous; use an orderItemId');
      }
      const line = matches[0];
      if (requested.sku !== undefined && requested.sku !== line.sku) {
        throw new Error('Requested SKU does not match the order item');
      }
      const total = (quantities.get(line.orderItemId) || 0) + requested.quantity;
      if (!Number.isSafeInteger(total) || total > line.remainingQuantity) {
        throw new Error(`Requested quantity exceeds unfulfilled units for ${line.orderItemId}`);
      }
      quantities.set(line.orderItemId, total);
    }
  } else {
    for (const line of lines.values()) {
      if (line.remainingQuantity > 0) quantities.set(line.orderItemId, line.remainingQuantity);
    }
  }

  return {
    orderId: order.id,
    orderVersion: order.version,
    orderStatus: order.status,
    parentShipment: shipment
      ? { id: shipment.id, version: shipment.version, status: shipment.status }
      : null,
    items: [...quantities].map(([id, quantity]) => ({ ...lines.get(id), quantity })),
    status: quantities.size ? 'reconciliation_required' : 'nothing_to_fulfill',
    executable: false,
    reason:
      'Order fulfillment quantities and shipment tracking are separate. Reconcile existing shipments and inventory reservations before creating a follow-up shipment. Automatic recovery requires an atomic command with durable idempotency.',
    snapshotOnly: true,
  };
}
