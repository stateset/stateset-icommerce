import { GovernedOperations } from './governed-operations.js';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function shipmentAction(value) {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).some((key) => !['orderId', 'trackingNumber', 'lines'].includes(key)) ||
    !uuid.test(value.orderId) ||
    typeof value.trackingNumber !== 'string' ||
    value.trackingNumber.length > 200 ||
    /[\p{Cc}\p{Bidi_Control}]/u.test(value.trackingNumber) ||
    !Array.isArray(value.lines) ||
    !value.lines.length ||
    value.lines.length > 100 ||
    value.lines.some(
      (line) =>
        !line ||
        Object.keys(line).some((key) => !['orderItemId', 'quantity'].includes(key)) ||
        !uuid.test(line.orderItemId) ||
        !Number.isInteger(line.quantity) ||
        line.quantity < 1 ||
        line.quantity > 2147483647,
    ) ||
    new Set(value.lines.map((line) => line.orderItemId)).size !== value.lines.length
  )
    throw new Error(
      'Choose an order, unique order lines with positive whole quantities, and tracking up to 200 characters.',
    );
  return {
    orderId: value.orderId,
    trackingNumber: value.trackingNumber.trim(),
    lines: value.lines.map(({ orderItemId, quantity }) => ({ orderItemId, quantity })),
  };
}

export function validShipmentContext(context) {
  return Boolean(
    context &&
    typeof context.trackingNumber === 'string' &&
    context.trackingNumber.length <= 200 &&
    ['shipped', 'partially_shipped'].includes(context.status) &&
    Array.isArray(context.lines) &&
    context.lines.length > 0 &&
    context.lines.length <= 100 &&
    context.lines.every(
      (line) =>
        line &&
        uuid.test(line.id) &&
        Number.isInteger(line.quantity) &&
        line.quantity > 0 &&
        Number.isInteger(line.shippedQuantity) &&
        line.shippedQuantity >= 0 &&
        line.shippedQuantity <= line.quantity,
    ) &&
    new Set(context.lines.map((line) => line.id)).size === context.lines.length,
  );
}

export function shipmentReceiptIssues(receipt, review) {
  if (receipt?.status !== 'succeeded') return [];
  const issues = [],
    result = receipt.result;
  if (receipt.aggregate_id !== review.params.orderId || result?.id !== review.params.orderId)
    issues.push('Native shipment identifies a different or unavailable order.');
  if (
    receipt.version_before !== review.expectedVersion ||
    receipt.version_after !== review.expectedVersion + 1 ||
    result?.version !== receipt.version_after
  )
    issues.push('Native shipment version does not match the reviewed order.');
  if (
    result?.status !== review.context.status ||
    (result?.tracking_number ?? '') !== review.context.trackingNumber
  )
    issues.push('Native shipment status or tracking does not match the review.');
  if (
    !Array.isArray(result?.items) ||
    result.items.length !== review.context.lines.length ||
    review.context.lines.some(
      (line) =>
        !result.items.some(
          (item) =>
            item.id === line.id &&
            item.quantity === line.quantity &&
            item.shipped_quantity === line.shippedQuantity,
        ),
    )
  )
    issues.push('Native shipped line quantities do not match the reviewed dispatch.');
  if (
    receipt.economic_context?.principal?.id !== review.principalId ||
    receipt.economic_context?.store_id !== review.storeId
  )
    issues.push('Native shipment scope does not match the reviewed operator and store.');
  return issues;
}

export class ShipmentOperations extends GovernedOperations {
  constructor(options) {
    super(options, {
      name: 'shipment',
      title: 'Shipment',
      resource: 'order',
      commandType: 'orders.ship_reserved',
      validate: shipmentAction,
      receiptIssues: shipmentReceiptIssues,
      validVersion: Number.isSafeInteger,
      context: (data) => {
        if (data.receipt?.status === 'previewed' && !validShipmentContext(data.context))
          throw new Error(
            'Shipment review context is invalid or exceeds the supported 100-line bound.',
          );
        return data.context;
      },
      validContext: validShipmentContext,
      matchesAggregate: (receipt, params) =>
        !receipt.aggregate_id || receipt.aggregate_id === params.orderId,
      historySummary: (params, context) => ({
        orderId: params.orderId,
        trackingNumber: context.trackingNumber,
        quantity: params.lines
          .reduce((total, line) => total + BigInt(line.quantity), 0n)
          .toString(),
      }),
    });
  }
}
