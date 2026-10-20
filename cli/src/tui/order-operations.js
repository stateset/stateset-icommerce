import { GovernedOperations } from './governed-operations.js';

export const ORDER_ACTIONS = Object.freeze([
  { status: 'confirmed', label: 'Confirm order' },
  { status: 'processing', label: 'Start processing' },
  { status: 'cancelled', label: 'Cancel order' },
]);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function orderAction(value) {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).some((key) => !['orderId', 'status'].includes(key)) ||
    typeof value.orderId !== 'string' ||
    !uuid.test(value.orderId) ||
    !ORDER_ACTIONS.some((action) => action.status === value.status)
  )
    throw new Error('Choose a supported order status and a valid order ID.');
  return { orderId: value.orderId, status: value.status };
}

// Keep a returned native outcome, including a commit, but never present an
// inconsistent success as a normal completion or invite an automatic retry.
export function orderReceiptIssues(receipt, review) {
  if (receipt?.status !== 'succeeded') return [];
  const issues = [];
  if (receipt.version_before !== review.expectedVersion)
    issues.push(
      `Reviewed version ${review.expectedVersion}; native receipt started from ${receipt.version_before ?? 'unavailable'}.`,
    );
  if (
    receipt.version_after !== review.expectedVersion + 1 ||
    receipt.result?.version !== receipt.version_after
  )
    issues.push('Native result version does not match the reviewed transition.');
  if (
    receipt.aggregate_id !== review.params.orderId ||
    receipt.result?.id !== review.params.orderId
  )
    issues.push('Native result identifies a different or unavailable order.');
  if (receipt.result?.status !== review.params.status)
    issues.push('Native result status does not match the reviewed action.');
  if (
    receipt.economic_context?.principal?.id !== review.principalId ||
    receipt.economic_context?.store_id !== review.storeId
  )
    issues.push('Native receipt scope does not match the reviewed operator and store.');
  return issues;
}

export class OrderOperations extends GovernedOperations {
  constructor(options) {
    super(options, {
      name: 'order',
      title: 'Order',
      resource: 'order',
      commandType: 'orders.transition',
      validate: orderAction,
      receiptIssues: orderReceiptIssues,
      validVersion: Number.isSafeInteger,
      context: () => null,
      validContext: () => true,
      matchesAggregate: (receipt, params) =>
        !receipt.aggregate_id || receipt.aggregate_id === params.orderId,
      historySummary: (params) => ({ orderId: params.orderId, status: params.status }),
    });
  }
}
