import { createCommerce } from '../commerce.js';
import {
  addDecimals,
  subtractDecimals,
  multiplyDecimals,
  compareDecimals,
  formatDecimal,
} from '../utils/exact-decimal.js';
import { inventoryPlanSchema } from './schema.js';
import { projectDemand, demandMethod } from './demand.js';
import { projectSupply } from './supply.js';
import { reconcilePlan } from './reconciliation.js';
import { decideReplenishment } from './purchasing.js';
import { reconcileCashFlow } from './cashflow.js';
import { reconcileInventoryFlow } from './inventory-flow.js';

export async function executeInventoryPlan(input, database) {
  const plan = inventoryPlanSchema.parse(input);
  const commerce = createCommerce(database);
  const demandForecast = projectDemand(plan);
  const supplyProjection = projectSupply(plan);
  const supplyBySku = new Map(supplyProjection.products.map((product) => [product.sku, product]));
  const dailyDemand = new Map(
    demandForecast.products.map((product) => [product.sku, product.dailyUnits]),
  );
  let cash = formatDecimal(plan.startingCash);
  let purchasingSpend = '0.00';
  let revenue = '0.00';
  let costOfGoodsSold = '0.00';
  const timeline = [];
  const pending = [];
  const purchases = [];
  const products = new Map();
  let purchaseCount = 0;
  let receivedCount = 0;
  let receiptEvents = 0;
  let partialReceipts = 0;
  let missedDeliveries = 0;
  let constraintBlockedPurchases = 0;
  let reserveBlockedPurchases = 0;
  let deferredPurchaseReviews = 0;
  let roundedPurchaseOrders = 0;
  let extraPurchasedUnits = 0;
  let orderCount = 0;
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
    const supplier = await commerce.purchaseOrders.createSupplier({
      name: 'Simulated supplier',
      supplierCode: 'SIM-SUPPLIER',
    });
    const customer = await commerce.customers.create({
      email: 'planning@example.test',
      firstName: 'Planning',
      lastName: 'Customer',
    });
    for (const [index, item] of plan.products.entries()) {
      const product = await commerce.products.create({
        name: `Planning product ${index + 1}`,
        slug: `planning-product-${index}`,
        variants: [{ sku: item.sku, priceExact: item.unitPrice, isDefault: true }],
      });
      await commerce.products.activate(product.id);
      const variant = await commerce.products.getVariantBySku(item.sku);
      // Cost the opening stock at the plan's unit cost: the engine refuses to
      // issue on-hand units that no cost layer covers.
      await commerce.costAccounting.setItemCost({
        sku: item.sku,
        costMethod: 'average',
        standardCost: item.unitCost,
      });
      await commerce.inventory.createItem({
        sku: item.sku,
        name: 'Planning stock',
        initialQuantity: item.initialStock,
      });
      products.set(item.sku, {
        ...item,
        productId: product.id,
        variantId: variant.id,
        demanded: 0,
        sold: 0,
        lost: 0,
        stockoutDays: 0,
        purchasedUnits: 0,
        receivedUnits: 0,
      });
    }
    async function replenish(day, item) {
      const stock = await commerce.inventory.getStock(item.sku);
      const onOrder = pending
        .filter((entry) => entry.sku === item.sku)
        .reduce((sum, entry) => sum + entry.quantity, 0);
      const position = Number(stock.totalAvailable) + onOrder;
      const decision = decideReplenishment(item, position, cash, plan.cashReserve, day);
      if (decision.kind === 'none') return;
      if (decision.kind === 'review-deferred') {
        deferredPurchaseReviews += 1;
        timeline.push({
          day,
          type: 'purchase_review_deferred',
          sku: item.sku,
          inventoryPosition: position,
          reviewIntervalDays: decision.reviewIntervalDays,
          nextReviewDay: decision.nextReviewDay,
          cash,
        });
        return;
      }
      const { quantity, spend, requestedQuantity, extraUnits } = decision;
      const quantityDecision = {
        quantity,
        requestedQuantity,
        extraUnits,
        inventoryPosition: position,
        minimumOrderQuantity: decision.minimumOrderQuantity,
        orderMultiple: decision.orderMultiple,
      };
      if (decision.kind === 'constraint-blocked') {
        constraintBlockedPurchases += 1;
        timeline.push({
          day,
          type: 'purchase_constraint_blocked',
          sku: item.sku,
          ...quantityDecision,
          reason: decision.reason,
          requiredPosition: decision.requiredPosition,
          maximumInventoryPosition: decision.maximumInventoryPosition,
          required: spend,
          cash,
        });
        return;
      }
      if (decision.kind === 'budget-blocked') {
        timeline.push({
          day,
          type: 'purchase_budget_blocked',
          sku: item.sku,
          ...quantityDecision,
          required: spend,
          cash,
        });
        return;
      }
      if (decision.kind === 'reserve-blocked') {
        reserveBlockedPurchases += 1;
        timeline.push({
          day,
          type: 'purchase_reserve_blocked',
          sku: item.sku,
          ...quantityDecision,
          required: spend,
          cash,
          cashReserve: decision.cashReserve,
          spendableCash: decision.spendableCash,
        });
        return;
      }
      // Real PO creation and state transitions; these never contact a supplier.
      let po = await commerce.purchaseOrders.create({
        supplierId: supplier.id,
        currency: plan.currency,
        items: [
          { sku: item.sku, name: 'Planning replenishment', quantity, unitCostExact: item.unitCost },
        ],
      });
      if (po.currency !== plan.currency || compareDecimals(po.totalExact, spend) !== 0)
        throw new Error('Native purchase total differs from the exact planning cost');
      await commerce.purchaseOrders.submit(po.id);
      await commerce.purchaseOrders.approve(po.id, 'simulation-policy');
      po = await commerce.purchaseOrders.send(po.id);
      const lines = await commerce.purchaseOrders.getItems(po.id);
      if (lines.length !== 1 || lines[0].sku !== item.sku)
        throw new Error('Native purchase lines do not match the replenishment');
      cash = subtractDecimals(cash, po.totalExact);
      purchasingSpend = addDecimals(purchasingSpend, po.totalExact);
      item.purchasedUnits += quantity;
      if (extraUnits > 0) roundedPurchaseOrders += 1;
      extraPurchasedUnits += extraUnits;
      const alias = `purchase-${++purchaseCount}`;
      const supply = supplyBySku.get(item.sku);
      const leadTimeDays = supply.dailyLeadTimeDays[day];
      const firstReceiptBps = supply.dailyFirstReceiptBps[day];
      const delivery = {
        id: po.id,
        line: lines[0].id,
        alias,
        sku: item.sku,
        quantity,
        orderedQuantity: quantity,
        inventoryPosition: position,
        receivedQuantity: 0,
        nextReceiptQuantity: Math.floor((quantity * firstReceiptBps) / 10000),
        remainderDelayDays: supply.remainderDelayDays,
        arrivalDay: day + leadTimeDays,
      };
      pending.push(delivery);
      purchases.push(delivery);
      timeline.push({
        day,
        type: 'purchase_order_sent',
        purchase: alias,
        sku: item.sku,
        ...quantityDecision,
        arrivalDay: delivery.arrivalDay,
        leadTimeDays,
        firstReceiptBps,
        amount: formatDecimal(po.totalExact),
        cash,
      });
    }
    // Day zero places initial replenishments; a one-day lead time arrives before day one's demand.
    for (const item of products.values()) await replenish(0, item);
    for (let day = 1; day <= plan.days; day += 1) {
      for (let index = pending.length - 1; index >= 0; index -= 1) {
        const delivery = pending[index];
        if (delivery.arrivalDay !== day) continue;
        const quantity = delivery.nextReceiptQuantity;
        const remaining = delivery.quantity - quantity;
        if (quantity > 0) {
          const po = await commerce.purchaseOrders.receive(
            delivery.id,
            [{ itemId: delivery.line, quantityReceivedExact: String(quantity) }],
            warehouse.id,
            location.id,
          );
          const expectedStatus = remaining ? 'partially_received' : 'received';
          if (po.status !== expectedStatus)
            throw new Error('Native purchase receipt status does not match delivered stock');
          products.get(delivery.sku).receivedUnits += quantity;
          delivery.receivedQuantity += quantity;
          receiptEvents += 1;
          if (remaining) partialReceipts += 1;
          else receivedCount += 1;
        } else {
          // Never submit a zero-quantity receipt or fabricate received stock.
          missedDeliveries += 1;
        }
        delivery.quantity = remaining;
        if (remaining) {
          delivery.arrivalDay = day + delivery.remainderDelayDays;
          delivery.nextReceiptQuantity = remaining;
        } else pending.splice(index, 1);
        timeline.push({
          day,
          type: quantity ? 'purchase_received' : 'purchase_delivery_missed',
          purchase: delivery.alias,
          sku: delivery.sku,
          quantity,
          remaining,
          complete: remaining === 0,
          ...(remaining ? { nextArrivalDay: delivery.arrivalDay } : {}),
        });
      }
      for (const item of products.values()) {
        const demand = dailyDemand.get(item.sku)[day - 1];
        const before = await commerce.inventory.getStock(item.sku);
        const sold = Math.min(demand, Number(before.totalAvailable));
        const lost = demand - sold;
        item.demanded += demand;
        item.sold += sold;
        item.lost += lost;
        if (lost) item.stockoutDays += 1;
        if (sold) {
          const order = await commerce.orders.createExact({
            customerId: customer.id,
            currency: plan.currency,
            stockPolicy: 'reject_if_insufficient',
            items: [
              {
                productId: item.productId,
                variantId: item.variantId,
                sku: item.sku,
                name: 'Planning sale',
                quantity: sold,
                unitPrice: item.unitPrice,
              },
            ],
          });
          let payment = await commerce.payments.createExact({
            orderId: order.id,
            customerId: customer.id,
            amount: order.totalAmountExact,
            currency: plan.currency,
            paymentMethod: 'credit_card',
            idempotencyKey: `planning:${day}:${item.sku}`,
          });
          payment = await commerce.payments.markCompleted(payment.id);
          if (
            payment.status !== 'completed' ||
            compareDecimals(payment.amountExact, order.totalAmountExact) !== 0
          ) {
            throw new Error('Native payment capture does not match the planning sale');
          }
          await commerce.orders.ship(order.id);
          await commerce.inventory.fulfillOrderStock(order.id);
          revenue = addDecimals(revenue, order.totalAmountExact);
          cash = addDecimals(cash, order.totalAmountExact);
          costOfGoodsSold = addDecimals(
            costOfGoodsSold,
            multiplyDecimals(item.unitCost, String(sold)),
          );
          orderCount += 1;
        }
        const stock = await commerce.inventory.getStock(item.sku);
        if (
          Number(stock.totalAvailable) !== Number(before.totalAvailable) - sold ||
          Number(stock.totalAllocated) !== 0
        )
          throw new Error(
            `Native shipment inventory does not match fulfilled demand: before=${before.totalAvailable}, sold=${sold}, after=${stock.totalAvailable}, allocated=${stock.totalAllocated}, onHand=${stock.totalOnHand}`,
          );
        timeline.push({
          day,
          type: 'demand',
          sku: item.sku,
          demand,
          sold,
          lost,
          available: Number(stock.totalAvailable),
          cash,
        });
      }
      // All sales for the day fund purchasing. Product input order defines
      // purchase priority when cash cannot cover every replenishment.
      for (const item of products.values()) await replenish(day, item);
    }
    const inventory = [];
    for (const item of products.values()) {
      const stock = await commerce.inventory.getStock(item.sku);
      inventory.push({
        sku: item.sku,
        demanded: item.demanded,
        sold: item.sold,
        lostSales: item.lost,
        stockoutDays: item.stockoutDays,
        onHand: Number(stock.totalOnHand),
        available: Number(stock.totalAvailable),
        allocated: Number(stock.totalAllocated),
        purchasedUnits: item.purchasedUnits,
        receivedUnits: item.receivedUnits,
        onOrder: pending
          .filter((entry) => entry.sku === item.sku)
          .reduce((sum, entry) => sum + entry.quantity, 0),
      });
    }
    const report = {
      name: plan.name,
      currency: plan.currency,
      seed: plan.seed,
      days: plan.days,
      demandForecast,
      supplyProjection,
      assumptions: {
        ...(plan.cashReserve !== undefined ? { cashReserve: formatDecimal(plan.cashReserve) } : {}),
        products: plan.products.map(
          ({
            sku,
            initialStock,
            unitPrice,
            unitCost,
            reorderPoint,
            targetStock,
            leadTimeDays,
            delivery,
            purchasing,
          }) => ({
            sku,
            initialStock,
            unitPrice,
            unitCost,
            reorderPoint,
            targetStock,
            leadTimeDays,
            ...(delivery ? { delivery } : {}),
            ...(purchasing ? { purchasing } : {}),
          }),
        ),
        demand: plan.products.map((item) => ({
          sku: item.sku,
          model: demandMethod(item.demand),
          input: item.demand,
        })),
        replenishment:
          'order-up-to-target-when-available-plus-on-order-is-at-or-below-reorder-point',
        timing: 'receipts-before-demand; purchases-after-demand; initial-purchases-on-day-zero',
        purchasingPayment: 'prepaid-at-order',
        purchasePriority: 'product-input-order-after-all-daily-sales',
        salesPayment: 'immediate-local-capture',
        shortages: 'lost-sales-no-backorders',
        inventoryCost: 'constant-input-unit-cost-including-opening-stock',
        supplierConditions: supplyProjection.products.some(
          (item) =>
            item.leadTimeMethod === 'explicit-daily-values' ||
            item.firstReceiptMethod === 'explicit-daily-values',
        )
          ? 'explicit-order-placement-schedules-with-independent-sampled-streams-for-other-conditions'
          : 'independent-seeded-streams-by-sku-and-order-placement-day',
        partialDelivery:
          'floor-first-receipt-basis-points; balance-arrives-after-remainder-delay; no-permanent-supplier-loss',
        purchaseSizing:
          'round-up-max-target-shortfall-and-minimum-to-order-multiple; may-exceed-target',
        purchaseConstraintPriority: 'maximum-inventory-position-before-whole-order-cash-budget',
        purchasingReview: 'per-sku-day-zero-then-every-reviewIntervalDays; omitted-is-daily',
      },
      summary: {
        demanded: inventory.reduce((sum, item) => sum + item.demanded, 0),
        sold: inventory.reduce((sum, item) => sum + item.sold, 0),
        lostSales: inventory.reduce((sum, item) => sum + item.lostSales, 0),
        orders: orderCount,
        purchaseOrders: purchaseCount,
        receivedPurchaseOrders: receivedCount,
        receiptEvents,
        partialReceipts,
        missedDeliveries,
        constraintBlockedPurchases,
        reserveBlockedPurchases,
        deferredPurchaseReviews,
        roundedPurchaseOrders,
        extraPurchasedUnits,
        startingCash: formatDecimal(plan.startingCash),
        revenue,
        lostRevenue: addDecimals(
          [...products.values()].map((item) => multiplyDecimals(item.unitPrice, String(item.lost))),
        ),
        endingInventoryValue: addDecimals(
          inventory.map((item) =>
            multiplyDecimals(products.get(item.sku).unitCost, String(item.onHand)),
          ),
        ),
        inTransitInventoryValue: addDecimals(
          pending.map((item) =>
            multiplyDecimals(products.get(item.sku).unitCost, String(item.quantity)),
          ),
        ),
        purchasingSpend,
        endingCash: cash,
        costOfGoodsSold,
        grossMargin: subtractDecimals(revenue, costOfGoodsSold),
      },
      inventory,
      outstandingPurchases: pending.map(
        ({ alias, sku, quantity, arrivalDay, orderedQuantity, receivedQuantity }) => ({
          purchase: alias,
          sku,
          quantity,
          arrivalDay,
          orderedQuantity,
          receivedQuantity,
        }),
      ),
      timeline,
    };
    report.cashFlow = reconcileCashFlow({
      startingCash: plan.startingCash,
      cashReserve: plan.cashReserve,
      days: plan.days,
      products: plan.products,
      timeline,
      summary: report.summary,
    });
    report.summary.minimumCash = report.cashFlow.minimumCash;
    report.inventoryFlow = reconcileInventoryFlow({
      days: plan.days,
      products: plan.products,
      demandForecast,
      supplyProjection,
      timeline,
      inventory,
      summary: report.summary,
    });
    report.reconciliation = reconcilePlan({
      startingCash: plan.startingCash,
      openingInventoryValue: addDecimals(
        plan.products.map((item) => multiplyDecimals(item.unitCost, String(item.initialStock))),
      ),
      summary: report.summary,
      inventory,
      products: plan.products,
    });
    if (
      (await commerce.orders.count()) !== orderCount ||
      (await commerce.payments.count()) !== orderCount ||
      (await commerce.purchaseOrders.count()) !== purchaseCount
    ) {
      throw new Error('Native record counts do not match the planning report');
    }
    report.purchases = [];
    for (const purchase of purchases) {
      const persisted = await commerce.purchaseOrders.get(purchase.id);
      const lines = await commerce.purchaseOrders.getItems(purchase.id);
      const expectedStatus =
        purchase.quantity === 0
          ? 'received'
          : purchase.receivedQuantity > 0
            ? 'partially_received'
            : 'sent';
      const item = products.get(purchase.sku);
      const minimum = item.purchasing?.minimumOrderQuantity ?? 1;
      const multiple = item.purchasing?.orderMultiple ?? 1;
      const maximumPosition = item.purchasing?.maximumInventoryPosition;
      if (
        persisted.status !== expectedStatus ||
        persisted.currency !== plan.currency ||
        compareDecimals(
          persisted.totalExact,
          multiplyDecimals(item.unitCost, String(purchase.orderedQuantity)),
        ) !== 0 ||
        lines.length !== 1 ||
        lines[0].id !== purchase.line ||
        lines[0].sku !== purchase.sku ||
        Number(lines[0].quantityOrderedExact) !== purchase.orderedQuantity ||
        Number(lines[0].quantityReceivedExact) !== purchase.receivedQuantity ||
        compareDecimals(lines[0].unitCostExact, item.unitCost) !== 0 ||
        compareDecimals(lines[0].lineTotalExact, persisted.totalExact) !== 0 ||
        purchase.orderedQuantity - purchase.receivedQuantity !== purchase.quantity
      )
        throw new Error('Native purchase quantities do not match the planning report');
      if (
        Number(lines[0].quantityOrderedExact) < minimum ||
        Number(lines[0].quantityOrderedExact) % multiple !== 0 ||
        (maximumPosition !== undefined &&
          purchase.inventoryPosition + Number(lines[0].quantityOrderedExact) > maximumPosition)
      )
        throw new Error('Native purchase quantity violates supplied purchasing constraints');
      report.purchases.push({
        purchase: purchase.alias,
        sku: lines[0].sku,
        status: persisted.status,
        currency: persisted.currency,
        total: formatDecimal(persisted.totalExact),
        quantityOrdered: Number(lines[0].quantityOrderedExact),
        quantityReceived: Number(lines[0].quantityReceivedExact),
        unitCost: formatDecimal(lines[0].unitCostExact),
        lineTotal: formatDecimal(lines[0].lineTotalExact),
      });
    }
    report.reconciliation.purchaseOrdersBalanced = true;
    report.reconciliation.purchasingConstraintsBalanced = true;
    report.reconciliation.cashFlowBalanced = true;
    report.reconciliation.inventoryFlowBalanced = true;
    return report;
  } finally {
    await commerce.close();
  }
}
