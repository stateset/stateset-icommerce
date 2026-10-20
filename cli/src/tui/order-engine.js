import { createRequire } from 'node:module';
import fs from 'node:fs';
import { getCommerceCtor, adaptCommerceApis } from '../commerce.js';
import { createKernelToolExecutor } from '../kernel-tool-execution.js';
import { orderAction } from './order-operations.js';

const require = createRequire(import.meta.url);
export async function previewOrder(commerce, kernel, payload) {
  const params = orderAction(payload.params);
  const order = await commerce.orders.get(params.orderId);
  if (!order || !Number.isSafeInteger(order.version))
    throw new Error('Order or its native version is unavailable.');
  const execute = createKernelToolExecutor({ commerce, kernel, allowApply: false });
  const result = await execute('update_order_status', params, {
    expectedVersion: order.version,
    idempotencyKey: payload.idempotencyKey,
  });
  return {
    order: {
      id: order.id,
      orderNumber: order.orderNumber,
      status: order.status,
      totalAmountExact: order.totalAmountExact,
      currency: order.currency,
      version: order.version,
    },
    params,
    receipt: result.receipt,
  };
}

export async function applyOrder(database, kernel, payload) {
  const params = orderAction(payload.params);
  if (!Number.isSafeInteger(payload.expectedVersion))
    throw new Error('A native preview version is required.');
  // Never create a missing source. This explicit path is the host's --db,
  // not a path accepted from workflow arguments.
  if (!fs.statSync(database).isFile()) throw new Error('The order store must already exist.');
  const Commerce = getCommerceCtor();
  const commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    const execute = createKernelToolExecutor({ commerce, kernel, allowApply: true });
    const result = await execute('update_order_status', params, {
      expectedVersion: payload.expectedVersion,
      idempotencyKey: payload.idempotencyKey,
    });
    return { receipt: result.receipt };
  } finally {
    await commerce.close();
  }
}

export function reconcileOrder(database, kernel, payload) {
  const Database = require('better-sqlite3');
  const db = new Database(database, { readonly: true, fileMustExist: true });
  try {
    const row = db
      .prepare('SELECT receipt FROM kernel_receipts WHERE idempotency_key = ? AND command_type = ?')
      .get(payload.idempotencyKey, 'orders.transition');
    const receipt = row ? JSON.parse(row.receipt) : null;
    if (
      receipt &&
      (receipt.economic_context?.principal?.id !== kernel.principal.id ||
        receipt.economic_context?.store_id !== kernel.storeId)
    )
      throw new Error('Stored receipt does not match the current operator and store.');
    return { receipt };
  } finally {
    db.close();
  }
}
