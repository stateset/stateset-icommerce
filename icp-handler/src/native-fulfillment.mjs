import { createHash } from 'node:crypto';
import { canonicalJson } from './codec.mjs';
import { amount } from './quote-money.mjs';

const hash = (value) => createHash('sha256').update(canonicalJson(value)).digest('hex');
const uuid = (hex) =>
  `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-8${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
const label = (value) => typeof value === 'string' && value.trim() && value.length <= 256;
const units = (value) => Number.isSafeInteger(value) && value > 0 && value <= 2147483647;
const progressWith = (progress, status) => {
  if (!progress) return { status };
  return progress.order_status === 'partially_shipped'
    ? { ...progress, next_dispatch_status: status }
    : progress;
};

/** Merge historical shipment evidence without letting a stale worker reduce
 * shipped progress. Attention at the same version remains until a later
 * verified dispatch advances it. Pre-upgrade recorded results were full ships. */
export function mergeNativeFulfillment(previous, incoming) {
  if (!incoming) return previous;
  if (previous?.status !== 'recorded') return incoming;
  if (incoming.status !== 'recorded') return previous;
  if (!previous.order_status || previous.order_status === 'shipped') return previous;
  if (incoming.order_version > previous.order_version) return incoming;
  if (incoming.order_version !== previous.order_version) return previous;
  const rank = { pending: 1, reconciling: 2, needs_attention: 3 };
  return (rank[incoming.next_dispatch_status] ?? 0) > (rank[previous.next_dispatch_status] ?? 0)
    ? incoming
    : previous;
}

/** Record operator-approved dispatches, one new command per reconciliation.
 * This does not buy labels, send goods, prove delivery or settle escrow. */
export class NativeMerchantFulfillment {
  constructor({
    store,
    commerce,
    principal,
    policy,
    storeId,
    readReceipt,
    resolveDispatch,
    allowApply = false,
  }) {
    if (
      typeof resolveDispatch !== 'function' ||
      typeof readReceipt !== 'function' ||
      typeof commerce?.orders?.get !== 'function'
    )
      throw new Error('operator dispatch resolver and native order reader required');
    Object.assign(this, { store, commerce, storeId, readReceipt, resolveDispatch, allowApply });
    this.principal = structuredClone(principal);
    this.policy = structuredClone(policy);
    this.scope = hash([principal.id, principal.tenant_id, storeId]);
    this.operations = store.collection('native_fulfillment_operations');
    this.claims = store.collection('native_fulfillment_claims');
    this.heads = store.collection('native_fulfillment_heads');
  }

  head(orderKey) {
    return this.heads.get(orderKey) ?? (this.operations.get(orderKey) ? orderKey : undefined);
  }

  async reconcile(terms) {
    if (this.allowApply !== true) return { status: 'preview' };
    terms = structuredClone(terms);
    const orderKey = hash([this.scope, terms.orderId]);
    let progress;
    try {
      const observed = this.head(orderKey);
      const prior = observed ? this.operations.get(observed) : undefined;
      if (observed) {
        const recovered = await this.recover(observed, terms);
        if (recovered.status !== 'recorded') return recovered;
        progress = recovered;
        // A recovered but previously uncheckpointed command is the only
        // dispatch handled by this request. The next approval waits for retry.
        if (!prior?.result || recovered.order_status !== 'partially_shipped') return recovered;
        if (recovered.next_dispatch_status) return recovered;
      }
      const features = await this.commerce.kernelFeatures();
      if (!features.includes('orders.ship_reserved.v1'))
        return progressWith(progress, 'needs_attention');
      const dispatch = await this.resolveDispatch(structuredClone(terms));
      if (dispatch === null) return progressWith(progress, 'pending');
      // Repeating any already-used approval for this order cannot ship it again.
      // A foreign reference claim remains a refusal, never an order rebind.
      if (label(dispatch?.reference)) {
        const owner = this.claims.get(hash([this.storeId, dispatch.reference]));
        if (owner) {
          const claimed = this.operations.get(owner);
          if (progress && canonicalJson(claimed?.terms) === canonicalJson(terms)) return progress;
          return progressWith(progress, 'needs_attention');
        }
      }
      const order = await this.commerce.orders.get(terms.orderId);
      if (!this.validDispatch(dispatch, order, terms))
        return progressWith(progress, 'needs_attention');
      const fullInitial = order.items.every(
        (item) =>
          item.shippedQuantity === 0 &&
          dispatch.items.some(
            (line) => line.orderItemId === item.id && line.quantity === item.quantity,
          ),
      );
      if (!fullInitial && !features.includes('orders.ship_reserved.partial.v1'))
        return progressWith(progress, 'needs_attention');
      const id = observed ? hash([this.scope, terms.orderId, dispatch.reference]) : orderKey;
      const command = {
        contract_version: '1.0',
        command_id: uuid(id),
        command_type: 'orders.ship_reserved',
        idempotency_key: `native-fulfillment:${id}`,
        principal: this.principal,
        store_id: this.storeId,
        policy_version: this.policy.version,
        mode: 'apply',
        issued_at: new Date().toISOString(),
        expected_version: dispatch.orderVersion,
        commitment: {
          quantity: dispatch.items
            .reduce((total, item) => total + BigInt(item.quantity), 0n)
            .toString(),
          evidence: [hash(dispatch.reference), terms.quoteId],
        },
        payload: {
          order_id: terms.orderId,
          tracking_number: dispatch.trackingNumber,
          lines: dispatch.items.map((item) => ({
            order_item_id: item.orderItemId,
            quantity: item.quantity,
          })),
        },
      };
      const selected = this.store.atomic(() => {
        const current = this.head(orderKey);
        // Another worker owns the next dispatch. Recover its immutable command.
        if (current !== observed) return current;
        const claim = hash([this.storeId, dispatch.reference]);
        if (this.claims.get(claim)) throw new Error('dispatch already claimed');
        if (this.operations.get(id)) throw new Error('dispatch identity already used');
        this.claims.set(claim, id);
        this.operations.set(id, {
          format: 2,
          terms,
          reference: dispatch.reference,
          command,
          beforeItems: order.items.map((item) => ({
            id: item.id,
            quantity: item.quantity,
            shipped_quantity: item.shippedQuantity,
          })),
          ...(progress ? { previousResult: progress } : {}),
        });
        this.heads.set(orderKey, id);
        return id;
      });
      return await this.recover(selected, terms);
    } catch {
      return progressWith(progress, 'reconciling');
    }
  }

  async recover(id, terms) {
    let progress;
    try {
      const operation = this.operations.get(id);
      if (!operation || canonicalJson(operation.terms) !== canonicalJson(terms))
        throw new Error('dispatch binding changed');
      progress = operation.result ?? operation.previousResult;
      const command = operation.command;
      if (!['orders.ship', 'orders.ship_reserved'].includes(command.command_type))
        throw new Error('unsupported shipment command');
      // Only authoritative absence permits execution; no resolver is involved.
      let receipt = await this.readReceipt(command.idempotency_key);
      if (receipt === null)
        receipt = await this.commerce.executeKernelCommand(
          structuredClone(command),
          structuredClone(this.policy),
        );
      if (
        !receipt ||
        receipt.command_id !== command.command_id ||
        receipt.idempotency_key !== command.idempotency_key ||
        receipt.command_type !== command.command_type
      )
        throw new Error('shipment receipt identity mismatch');
      if (receipt.status === 'rejected') return progressWith(progress, 'needs_attention');
      if (receipt.status !== 'succeeded') return progressWith(progress, 'reconciling');
      const order = receipt.result;
      if (
        !order ||
        order.id !== terms.orderId ||
        order.payment_status !== 'paid' ||
        order.currency !== terms.currency ||
        amount(order.total_amount) !== amount(terms.amount) ||
        order.tracking_number !== command.payload.tracking_number ||
        receipt.version_before !== command.expected_version ||
        !Array.isArray(order.items)
      )
        throw new Error('shipment receipt result mismatch');
      if (operation.format === 2) {
        const before = operation.beforeItems;
        const ids = new Set(order.items.map((item) => item.id));
        if (
          command.command_type !== 'orders.ship_reserved' ||
          !Number.isSafeInteger(receipt.version_after) ||
          receipt.version_after !== receipt.version_before + 1 ||
          order.version !== receipt.version_after ||
          order.items.length !== before.length ||
          ids.size !== before.length ||
          !before.every((item) =>
            order.items.some(
              (after) =>
                after.id === item.id &&
                after.quantity === item.quantity &&
                after.shipped_quantity ===
                  item.shipped_quantity +
                    (command.payload.lines.find((line) => line.order_item_id === item.id)
                      ?.quantity ?? 0),
            ),
          )
        )
          throw new Error('shipment receipt progress mismatch');
        const status = order.items.every((item) => item.shipped_quantity === item.quantity)
          ? 'shipped'
          : 'partially_shipped';
        if (order.status !== status) throw new Error('shipment receipt status mismatch');
      } else if (
        order.status !== 'shipped' ||
        order.items.length !== command.payload.lines.length ||
        !command.payload.lines.every((line) =>
          order.items.some(
            (item) =>
              item.id === line.order_item_id &&
              item.quantity === line.quantity &&
              item.shipped_quantity === line.quantity,
          ),
        )
      ) {
        throw new Error('legacy shipment receipt result mismatch');
      }
      const result = {
        status: 'recorded',
        order_id: terms.orderId,
        tracking_number: order.tracking_number,
        inventory:
          command.command_type === 'orders.ship_reserved'
            ? 'reserved_stock_consumed'
            : 'depletion_unverified',
        ...(operation.format === 2
          ? {
              order_status: order.status,
              order_version: receipt.version_after,
              items: order.items
                .map((item) => ({
                  order_item_id: item.id,
                  quantity: item.quantity,
                  shipped_quantity: item.shipped_quantity,
                }))
                .sort((a, b) => a.order_item_id.localeCompare(b.order_item_id)),
            }
          : {}),
      };
      if (operation.result && canonicalJson(operation.result) !== canonicalJson(result))
        throw new Error('recorded shipment changed');
      if (!operation.result)
        this.store.atomic(() => {
          const saved = this.operations.get(id);
          this.operations.set(id, { ...saved, result });
        });
      return result;
    } catch {
      return progressWith(progress, 'reconciling');
    }
  }

  validDispatch(dispatch, order, terms) {
    if (
      !dispatch ||
      !label(dispatch.reference) ||
      !label(dispatch.trackingNumber) ||
      dispatch.orderId !== terms.orderId ||
      !Number.isSafeInteger(dispatch.orderVersion) ||
      !order ||
      order.id !== terms.orderId ||
      order.version !== dispatch.orderVersion ||
      !['processing', 'partially_shipped'].includes(order.status) ||
      order.paymentStatus !== 'paid' ||
      order.currency !== terms.currency ||
      amount(order.totalAmountExact) !== amount(terms.amount) ||
      !Array.isArray(order.items) ||
      !order.items.length ||
      !Array.isArray(dispatch.items) ||
      !dispatch.items.length
    )
      return false;
    const orderIds = new Set();
    if (
      !order.items.every((line) => {
        if (
          !label(line.id) ||
          orderIds.has(line.id) ||
          !units(line.quantity) ||
          !Number.isSafeInteger(line.shippedQuantity) ||
          line.shippedQuantity < 0 ||
          line.shippedQuantity > line.quantity
        )
          return false;
        orderIds.add(line.id);
        return true;
      })
    )
      return false;
    const ids = new Set();
    return dispatch.items.every((item) => {
      if (!item || ids.has(item.orderItemId) || !units(item.quantity)) return false;
      ids.add(item.orderItemId);
      return order.items.some(
        (line) =>
          line.id === item.orderItemId && item.quantity <= line.quantity - line.shippedQuantity,
      );
    });
  }
}
