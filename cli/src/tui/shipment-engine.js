import fs from 'node:fs';
import { getCommerceCtor, adaptCommerceApis } from '../commerce.js';
import { createKernelToolExecutor } from '../kernel-tool-execution.js';
import { compareDecimals } from '../utils/exact-decimal.js';
import { shipmentAction } from './shipment-operations.js';
import { readNativeReceipt } from './receipt-engine.js';

export function shipmentCommitment(kernel, lines) {
  const quantity = lines.reduce((total, line) => total + BigInt(line.quantity), 0n).toString();
  const configured = kernel.commitment ?? {};
  if (!configured || typeof configured !== 'object' || Array.isArray(configured))
    throw new Error('Shipment commitment must be operator-owned configuration.');
  if (
    configured.quantity !== null &&
    configured.quantity !== undefined &&
    (typeof configured.quantity !== 'string' ||
      compareDecimals(configured.quantity, quantity) !== 0)
  )
    throw new Error('Operator commitment does not match the reviewed shipment quantity.');
  return { ...configured, quantity };
}

export async function previewShipment(commerce, kernel, payload) {
  const params = shipmentAction(payload.params),
    order = await commerce.orders.get(params.orderId);
  if (
    !order ||
    !Number.isSafeInteger(order.version) ||
    !Array.isArray(order.items) ||
    !order.items.length
  )
    throw new Error('Order, lines or native version are unavailable.');
  const lines = order.items.map((item) => ({
    id: item.id,
    quantity: item.quantity,
    shippedQuantity:
      item.shippedQuantity +
      (params.lines.find((line) => line.orderItemId === item.id)?.quantity ?? 0),
  }));
  const context = {
    lines,
    trackingNumber: params.trackingNumber || order.trackingNumber || '',
    status: lines.every((line) => line.shippedQuantity === line.quantity)
      ? 'shipped'
      : 'partially_shipped',
  };
  const execute = createKernelToolExecutor({ commerce, kernel, allowApply: false });
  const result = await execute('ship_reserved_order', params, {
    expectedVersion: order.version,
    idempotencyKey: payload.idempotencyKey,
    commitment: shipmentCommitment(kernel, params.lines),
  });
  return {
    params,
    context,
    receipt: result.receipt,
    order: {
      id: order.id,
      orderNumber: order.orderNumber,
      status: order.status,
      paymentStatus: order.paymentStatus,
      version: order.version,
      items: order.items.map((item) => ({
        id: item.id,
        sku: item.sku,
        quantity: item.quantity,
        shippedQuantity: item.shippedQuantity,
      })),
    },
  };
}

export async function applyShipment(database, kernel, payload) {
  const params = shipmentAction(payload.params);
  if (!Number.isSafeInteger(payload.expectedVersion))
    throw new Error('A native preview version is required.');
  if (!fs.statSync(database).isFile()) throw new Error('The order store must already exist.');
  const Commerce = getCommerceCtor(),
    commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    const execute = createKernelToolExecutor({ commerce, kernel, allowApply: true });
    const result = await execute('ship_reserved_order', params, {
      expectedVersion: payload.expectedVersion,
      idempotencyKey: payload.idempotencyKey,
      commitment: shipmentCommitment(kernel, params.lines),
    });
    return { receipt: result.receipt };
  } finally {
    await commerce.close();
  }
}
export function reconcileShipment(database, kernel, payload) {
  return readNativeReceipt(database, kernel, payload.idempotencyKey, 'orders.ship_reserved');
}
