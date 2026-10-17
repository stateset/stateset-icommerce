import { SIMULATION_LIMITS as limits } from './limits.js';

// Replay physical units independently of the planner's running counters. Final
// balances alone cannot prove that stock existed when a sale was fulfilled.
export function reconcileInventoryFlow({
  days,
  products,
  demandForecast,
  supplyProjection,
  timeline,
  inventory,
  summary,
}) {
  const fail = () => {
    throw new Error('Simulation inventory-flow reconciliation failed');
  };
  const integer = (value, minimum = 0, maximum = Number.MAX_SAFE_INTEGER) =>
    Number.isSafeInteger(value) && value >= minimum && value <= maximum;
  if (!integer(days, 1, limits.maxPlanningDays)) fail();
  const bySku = new Map(products.map((product) => [product.sku, product]));
  const indexProducts = (rows) => {
    const indexed = new Map(rows.map((row) => [row.sku, row]));
    if (
      rows.length !== bySku.size ||
      indexed.size !== bySku.size ||
      [...indexed.keys()].some((sku) => !bySku.has(sku))
    )
      fail();
    return indexed;
  };
  if (bySku.size !== products.length || products.length === 0) fail();
  const demand = indexProducts(demandForecast.products);
  const supply = indexProducts(supplyProjection.products);
  const finalInventory = indexProducts(inventory);
  const balances = new Map(
    products.map((product) => {
      if (!integer(product.initialStock, 0, 1000000)) fail();
      const forecast = demand.get(product.sku);
      const conditions = supply.get(product.sku);
      if (
        forecast.dailyUnits.length !== days ||
        !forecast.dailyUnits.every((value) => integer(value, 0, 10000)) ||
        conditions.dailyLeadTimeDays.length !== days + 1 ||
        !conditions.dailyLeadTimeDays.every((value) => integer(value, 1, 60)) ||
        conditions.dailyFirstReceiptBps.length !== days + 1 ||
        !conditions.dailyFirstReceiptBps.every((value) => integer(value, 0, 10000)) ||
        !integer(conditions.remainderDelayDays, 1, 60)
      )
        fail();
      return [
        product.sku,
        {
          stock: product.initialStock,
          onOrder: 0,
          demanded: 0,
          sold: 0,
          lostSales: 0,
          purchasedUnits: 0,
          receivedUnits: 0,
          stockoutDays: 0,
        },
      ];
    }),
  );
  let previousDay = 0;
  for (const event of timeline) {
    if (!integer(event.day, previousDay, days)) fail();
    previousDay = event.day;
  }
  const purchases = new Map();
  const counts = {
    orders: 0,
    purchaseOrders: 0,
    receivedPurchaseOrders: 0,
    receiptEvents: 0,
    partialReceipts: 0,
    missedDeliveries: 0,
    deferredPurchaseReviews: 0,
  };
  const blockedTypes = [
    'purchase_budget_blocked',
    'purchase_constraint_blocked',
    'purchase_reserve_blocked',
  ];
  const receiptsDue = (day) =>
    [...purchases.values()].some(
      (purchase) => purchase.remaining > 0 && purchase.arrivalDay <= day,
    );
  let cursor = 0;
  let firstStockout = null;
  const daily = [];
  for (let day = 0; day <= days; day += 1) {
    const rows = new Map(
      products.map(({ sku }) => {
        const balance = balances.get(sku);
        return [
          sku,
          {
            sku,
            openingStock: balance.stock,
            openingOnOrder: balance.onOrder,
            receivedUnits: 0,
            demanded: 0,
            sold: 0,
            lostSales: 0,
            orderedUnits: 0,
          },
        ];
      }),
    );
    const observedDemand = new Set();
    const observedPurchasing = new Set();
    let phase = 0;
    while (cursor < timeline.length && timeline[cursor].day === day) {
      const event = timeline[cursor++];
      const balance = balances.get(event.sku);
      if (!balance) fail();
      const row = rows.get(event.sku);
      if (event.type === 'purchase_received' || event.type === 'purchase_delivery_missed') {
        if (day === 0 || phase !== 0) fail();
        const purchase = purchases.get(event.purchase);
        if (
          !purchase ||
          purchase.sku !== event.sku ||
          purchase.remaining <= 0 ||
          purchase.arrivalDay !== day ||
          !integer(event.quantity) ||
          event.quantity !== purchase.nextQuantity ||
          event.remaining !== purchase.remaining - event.quantity ||
          event.complete !== (event.remaining === 0) ||
          (event.remaining > 0
            ? event.nextArrivalDay !== day + purchase.remainderDelayDays
            : event.nextArrivalDay !== undefined) ||
          (event.type === 'purchase_received' ? event.quantity === 0 : event.quantity !== 0)
        )
          fail();
        balance.stock += event.quantity;
        balance.onOrder -= event.quantity;
        balance.receivedUnits += event.quantity;
        row.receivedUnits += event.quantity;
        purchase.remaining = event.remaining;
        purchase.nextQuantity = event.remaining;
        purchase.arrivalDay = event.nextArrivalDay;
        if (event.quantity === 0) counts.missedDeliveries += 1;
        else {
          counts.receiptEvents += 1;
          if (event.remaining > 0) counts.partialReceipts += 1;
          else counts.receivedPurchaseOrders += 1;
        }
      } else if (event.type === 'demand') {
        if (day === 0 || phase > 1 || observedDemand.has(event.sku) || receiptsDue(day)) fail();
        phase = 1;
        observedDemand.add(event.sku);
        if (
          !integer(event.demand, 0, 10000) ||
          event.demand !== demand.get(event.sku).dailyUnits[day - 1] ||
          !integer(event.sold) ||
          event.sold !== Math.min(event.demand, balance.stock) ||
          event.lost !== event.demand - event.sold ||
          event.available !== balance.stock - event.sold
        )
          fail();
        balance.stock -= event.sold;
        balance.demanded += event.demand;
        balance.sold += event.sold;
        balance.lostSales += event.lost;
        row.demanded = event.demand;
        row.sold = event.sold;
        row.lostSales = event.lost;
        if (event.sold > 0) counts.orders += 1;
        if (event.lost > 0) {
          balance.stockoutDays += 1;
          firstStockout ??= {
            day,
            sku: event.sku,
            demanded: event.demand,
            sold: event.sold,
            lostSales: event.lost,
          };
        }
      } else if (
        event.type === 'purchase_order_sent' ||
        blockedTypes.includes(event.type) ||
        event.type === 'purchase_review_deferred'
      ) {
        if (day > 0 && observedDemand.size !== products.length) fail();
        if (observedPurchasing.has(event.sku)) fail();
        observedPurchasing.add(event.sku);
        phase = 2;
        if (event.inventoryPosition !== balance.stock + balance.onOrder) fail();
        const product = bySku.get(event.sku);
        const reviewIntervalDays = product.purchasing?.reviewIntervalDays ?? 1;
        if (event.type === 'purchase_review_deferred') {
          if (
            !integer(product.reorderPoint) ||
            day % reviewIntervalDays === 0 ||
            event.reviewIntervalDays !== reviewIntervalDays ||
            event.nextReviewDay !== day + reviewIntervalDays - (day % reviewIntervalDays) ||
            event.inventoryPosition > product.reorderPoint
          )
            fail();
          counts.deferredPurchaseReviews += 1;
        } else if (day % reviewIntervalDays !== 0) fail();
        if (event.type === 'purchase_order_sent') {
          const conditions = supply.get(event.sku);
          const lead = conditions.dailyLeadTimeDays[day];
          const firstFill = conditions.dailyFirstReceiptBps[day];
          if (
            typeof event.purchase !== 'string' ||
            event.purchase.length === 0 ||
            purchases.has(event.purchase) ||
            !integer(event.quantity, 1, limits.maxPurchaseQuantity) ||
            event.leadTimeDays !== lead ||
            event.firstReceiptBps !== firstFill ||
            event.arrivalDay !== day + lead
          )
            fail();
          purchases.set(event.purchase, {
            sku: event.sku,
            remaining: event.quantity,
            arrivalDay: event.arrivalDay,
            nextQuantity: Math.floor((event.quantity * firstFill) / 10000),
            remainderDelayDays: conditions.remainderDelayDays,
          });
          balance.onOrder += event.quantity;
          balance.purchasedUnits += event.quantity;
          row.orderedUnits += event.quantity;
          counts.purchaseOrders += 1;
        }
      } else fail();
    }
    if ((day > 0 && observedDemand.size !== products.length) || receiptsDue(day)) fail();
    daily.push({
      day,
      products: [...rows.values()].map((row) => {
        const balance = balances.get(row.sku);
        if (!integer(balance.stock) || !integer(balance.onOrder)) fail();
        return { ...row, closingStock: balance.stock, onOrder: balance.onOrder };
      }),
    });
  }
  if (cursor !== timeline.length) fail();
  const unitTotals = { demanded: 0, sold: 0, lostSales: 0 };
  for (const [sku, balance] of balances) {
    const persisted = finalInventory.get(sku);
    if (
      persisted.onHand !== balance.stock ||
      persisted.available !== balance.stock ||
      persisted.allocated !== 0 ||
      ![
        'onOrder',
        'demanded',
        'sold',
        'lostSales',
        'purchasedUnits',
        'receivedUnits',
        'stockoutDays',
      ].every((key) => integer(persisted[key]) && persisted[key] === balance[key])
    )
      fail();
    for (const key of Object.keys(unitTotals)) unitTotals[key] += balance[key];
  }
  if (!Object.entries({ ...unitTotals, ...counts }).every(([key, value]) => summary[key] === value))
    fail();
  return { firstStockout, daily };
}
