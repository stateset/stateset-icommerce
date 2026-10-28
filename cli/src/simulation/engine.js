import { createCommerce } from '../commerce.js';
import { addDecimals, formatDecimal, subtractDecimals } from '../utils/exact-decimal.js';
import { commerceScenarioSchema } from './schema.js';
import { receivePurchase } from './native-compat.js';

function reference(map, name, kind) {
  const value = map.get(name);
  if (!value) throw new Error(`Simulation ${kind} '${name}' is unavailable`);
  return value;
}

// Called only by the fixed child-process entry point. The database filename is
// parent-owned and never comes from model arguments. No live store is accepted.
export async function executeCommerceScenario(input, database) {
  const scenario = commerceScenarioSchema.parse(input);
  const commerce = createCommerce(database);
  const products = new Map();
  const orders = new Map();
  const payments = new Map();
  const refunds = new Map();
  const purchases = new Map();
  let supplier;
  const outcomes = [];
  async function inspectState() {
    const inventory = [];
    for (const item of scenario.inventory) {
      const stock = await commerce.inventory.getStock(item.sku);
      inventory.push({
        sku: item.sku,
        onHand: Number(stock.totalOnHand),
        available: Number(stock.totalAvailable),
        allocated: Number(stock.totalAllocated),
      });
    }
    const orderState = [];
    for (const [alias, original] of orders) {
      const order = await commerce.orders.get(original.id);
      orderState.push({
        order: alias,
        status: order.status,
        total: formatDecimal(order.totalAmountExact),
      });
    }
    const paymentState = [];
    for (const [alias, original] of payments) {
      const payment = await commerce.payments.get(original.payment.id);
      paymentState.push({
        payment: alias,
        status: payment.status,
        amount: formatDecimal(payment.amountExact),
        refunded: formatDecimal(payment.amountRefundedExact),
      });
    }
    const purchaseState = [];
    for (const [alias, original] of purchases) {
      const purchase = await commerce.purchaseOrders.get(original.id);
      const lines = await commerce.purchaseOrders.getItems(original.id);
      purchaseState.push({
        purchase: alias,
        status: purchase.status,
        total: formatDecimal(purchase.totalExact),
        items: lines.map((line) => ({
          sku: line.sku,
          ordered: Number(line.quantityOrderedExact),
          received: Number(line.quantityReceivedExact),
          unitCost: formatDecimal(line.unitCostExact),
          total: formatDecimal(line.lineTotalExact),
        })),
      });
    }
    return { inventory, orders: orderState, payments: paymentState, purchases: purchaseState };
  }
  try {
    const warehouse = await commerce.warehouse.createWarehouse({
      code: 'SIM',
      name: 'Simulation warehouse',
    });
    const location = await commerce.warehouse.createLocation({
      warehouseId: warehouse.id,
      locationType: 'receiving',
      isReceivable: true,
    });
    const customer = await commerce.customers.create({
      email: 'simulation@example.test',
      firstName: 'Simulation',
      lastName: 'Customer',
    });
    for (const [index, item] of scenario.inventory.entries()) {
      const product = await commerce.products.create({
        name: `Simulation product ${index + 1}`,
        slug: `simulation-product-${index}`,
        variants: [{ sku: item.sku, priceExact: item.unitPrice, isDefault: true }],
      });
      await commerce.products.activate(product.id);
      const variant = await commerce.products.getVariantBySku(item.sku);
      await commerce.inventory.createItem({
        sku: item.sku,
        name: 'Simulation stock',
        initialQuantity: item.quantity,
      });
      products.set(item.sku, { ...item, productId: product.id, variantId: variant.id });
    }
    for (const [index, step] of scenario.steps.entries()) {
      try {
        let observation;
        switch (step.type) {
          case 'inspect_state':
            observation = await inspectState();
            break;
          case 'purchase_inventory': {
            if (!supplier)
              supplier = await commerce.purchaseOrders.createSupplier({
                name: 'Simulation supplier',
                supplierCode: 'SIM-SUPPLIER',
              });
            let purchase = await commerce.purchaseOrders.create({
              supplierId: supplier.id,
              currency: scenario.currency,
              items: step.items.map((item) => ({
                sku: item.sku,
                name: 'Simulation replenishment',
                quantity: item.quantity,
                unitCostExact: item.unitCost,
              })),
            });
            // Retain aliases immediately: later transition refusals must remain inspectable.
            purchases.set(step.id, purchase);
            await commerce.purchaseOrders.submit(purchase.id);
            await commerce.purchaseOrders.approve(purchase.id, 'simulation-policy');
            purchase = await commerce.purchaseOrders.send(purchase.id);
            observation = {
              purchase: step.id,
              status: purchase.status,
              total: formatDecimal(purchase.totalExact),
            };
            break;
          }
          case 'receive_purchase_order': {
            const original = reference(purchases, step.purchase, 'purchase');
            const lines = await commerce.purchaseOrders.getItems(original.id);
            const purchase = await receivePurchase(
              commerce,
              original.id,
              lines.map((line) => ({
                itemId: line.id,
                sku: line.sku,
                quantity: line.quantityOrderedExact,
              })),
              warehouse.id,
              location.id,
            );
            observation = {
              purchase: step.purchase,
              status: purchase.status,
              total: formatDecimal(purchase.totalExact),
            };
            break;
          }
          case 'place_order': {
            const order = await commerce.orders.createExact({
              customerId: customer.id,
              currency: scenario.currency,
              stockPolicy: 'reject_if_insufficient',
              items: step.items.map((item) => {
                const product = reference(products, item.sku, 'SKU');
                return {
                  sku: item.sku,
                  name: 'Simulation item',
                  productId: product.productId,
                  variantId: product.variantId,
                  quantity: item.quantity,
                  unitPrice: product.unitPrice,
                };
              }),
            });
            orders.set(step.id, order);
            observation = {
              order: step.id,
              status: order.status,
              total: formatDecimal(order.totalAmountExact),
            };
            break;
          }
          case 'capture_payment': {
            const order = reference(orders, step.order, 'order');
            const request = {
              orderId: order.id,
              customerId: customer.id,
              amount: order.totalAmountExact,
              currency: scenario.currency,
              paymentMethod: 'credit_card',
              idempotencyKey: `simulation:${step.key || step.id}`,
            };
            let payment = await commerce.payments.createExact(request);
            if (payment.status === 'pending')
              payment = await commerce.payments.markCompleted(payment.id);
            payments.set(step.id, { payment, request });
            observation = {
              payment: step.id,
              status: payment.status,
              amount: formatDecimal(payment.amountExact),
            };
            break;
          }
          case 'refund_payment': {
            const { payment } = reference(payments, step.payment, 'payment');
            const request = {
              paymentId: payment.id,
              amount: step.amount,
              reason: 'Synthetic simulation refund',
              idempotencyKey: `simulation:${step.key || step.id}`,
            };
            let refund = await commerce.payments.createRefundExact(request);
            refund = await commerce.payments.completeRefund(refund.id);
            refunds.set(step.id, { refund, request });
            observation = {
              refund: step.id,
              status: refund.status,
              amount: formatDecimal(refund.amountExact),
            };
            break;
          }
          case 'retry_payment': {
            const original = reference(payments, step.payment, 'payment');
            const replay = await commerce.payments.createExact(original.request);
            if (replay.id !== original.payment.id)
              throw new Error('Payment retry created another payment');
            observation = { payment: step.payment, status: replay.status, replayed: true };
            break;
          }
          case 'retry_refund': {
            const original = reference(refunds, step.refund, 'refund');
            const replay = await commerce.payments.createRefundExact(original.request);
            if (replay.id !== original.refund.id)
              throw new Error('Refund retry created another refund');
            await commerce.payments.completeRefund(replay.id);
            observation = { refund: step.refund, status: replay.status, replayed: true };
            break;
          }
          case 'restock':
            await commerce.inventory.adjust(
              step.sku,
              step.quantity,
              'Synthetic simulation restock',
            );
            observation = { sku: step.sku, added: step.quantity };
            break;
          case 'cancel_order': {
            const order = reference(orders, step.order, 'order');
            const cancelled = await commerce.orders.cancel(order.id);
            observation = { order: step.order, status: cancelled.status };
            break;
          }
          case 'ship_order': {
            const order = reference(orders, step.order, 'order');
            const shipped = await commerce.orders.ship(order.id);
            await commerce.inventory.fulfillOrderStock(order.id);
            observation = { order: step.order, status: shipped.status };
            break;
          }
          default:
            throw new Error('Unsupported simulation action');
        }
        outcomes.push({
          index,
          type: step.type,
          status: 'success',
          expected: step.expect,
          expectationMet: step.expect === 'success',
          observation,
        });
      } catch (error) {
        if (error instanceof TypeError || error instanceof ReferenceError) throw error;
        // Only synthetic records exist in this process. Do not expose random
        // native UUIDs or ephemeral paths as part of reproducible reports.
        const message = String(error.message || 'Operation rejected').replace(
          /[0-9a-f]{8}-[0-9a-f-]{27,}/gi,
          '<entity>',
        );
        outcomes.push({
          index,
          type: step.type,
          status: 'rejected',
          expected: step.expect,
          expectationMet: step.expect === 'rejected',
          reason: message.slice(0, 500),
        });
      }
    }
    const orderRows = await commerce.orders.list({ limit: 200 });
    const paymentRows = await commerce.payments.list({ limit: 200 });
    const refunded = addDecimals(paymentRows.map((payment) => payment.amountRefundedExact));
    const captured = addDecimals(
      paymentRows
        .filter((payment) =>
          ['completed', 'partially_refunded', 'refunded', 'disputed'].includes(payment.status),
        )
        .map((payment) => payment.amountExact),
    );
    const state = await inspectState();
    return {
      name: scenario.name,
      currency: scenario.currency,
      expectationsMet: outcomes.every((outcome) => outcome.expectationMet),
      outcomes,
      summary: {
        orders: orderRows.length,
        payments: paymentRows.length,
        completedRefunds: (
          await Promise.all(paymentRows.map((payment) => commerce.payments.getRefunds(payment.id)))
        )
          .flat()
          .filter((refund) => refund.status === 'completed').length,
        captured,
        refunded,
        netCaptured: subtractDecimals(captured, refunded),
        rejectedSteps: outcomes.filter((outcome) => outcome.status === 'rejected').length,
      },
      inventory: state.inventory,
      state,
    };
  } finally {
    await commerce.close();
  }
}
