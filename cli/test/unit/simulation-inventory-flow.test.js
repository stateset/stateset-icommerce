import { test } from 'node:test';
import assert from 'node:assert/strict';
import { reconcileInventoryFlow } from '../../src/simulation/inventory-flow.js';

function episode() {
  return {
    days: 3,
    products: [
      { sku: 'A', initialStock: 1 },
      { sku: 'B', initialStock: 3 },
    ],
    demandForecast: {
      products: [
        { sku: 'A', dailyUnits: [2, 2, 1] },
        { sku: 'B', dailyUnits: [0, 1, 1] },
      ],
    },
    supplyProjection: {
      products: ['A', 'B'].map((sku) => ({
        sku,
        dailyLeadTimeDays: [2, 2, 2, 2],
        dailyFirstReceiptBps: [5000, 5000, 5000, 5000],
        remainderDelayDays: 1,
      })),
    },
    timeline: [
      {
        day: 0,
        type: 'purchase_order_sent',
        sku: 'A',
        purchase: 'purchase-1',
        quantity: 4,
        inventoryPosition: 1,
        leadTimeDays: 2,
        firstReceiptBps: 5000,
        arrivalDay: 2,
      },
      { day: 1, type: 'demand', sku: 'A', demand: 2, sold: 1, lost: 1, available: 0 },
      { day: 1, type: 'demand', sku: 'B', demand: 0, sold: 0, lost: 0, available: 3 },
      {
        day: 2,
        type: 'purchase_received',
        sku: 'A',
        purchase: 'purchase-1',
        quantity: 2,
        remaining: 2,
        complete: false,
        nextArrivalDay: 3,
      },
      { day: 2, type: 'demand', sku: 'A', demand: 2, sold: 2, lost: 0, available: 0 },
      { day: 2, type: 'demand', sku: 'B', demand: 1, sold: 1, lost: 0, available: 2 },
      {
        day: 3,
        type: 'purchase_received',
        sku: 'A',
        purchase: 'purchase-1',
        quantity: 2,
        remaining: 0,
        complete: true,
      },
      { day: 3, type: 'demand', sku: 'A', demand: 1, sold: 1, lost: 0, available: 1 },
      { day: 3, type: 'demand', sku: 'B', demand: 1, sold: 1, lost: 0, available: 1 },
      { day: 3, type: 'purchase_budget_blocked', sku: 'A', inventoryPosition: 1 },
    ],
    inventory: [
      {
        sku: 'A',
        onHand: 1,
        available: 1,
        allocated: 0,
        onOrder: 0,
        demanded: 5,
        sold: 4,
        lostSales: 1,
        purchasedUnits: 4,
        receivedUnits: 4,
        stockoutDays: 1,
      },
      {
        sku: 'B',
        onHand: 1,
        available: 1,
        allocated: 0,
        onOrder: 0,
        demanded: 2,
        sold: 2,
        lostSales: 0,
        purchasedUnits: 0,
        receivedUnits: 0,
        stockoutDays: 0,
      },
    ],
    summary: {
      demanded: 7,
      sold: 6,
      lostSales: 1,
      orders: 5,
      purchaseOrders: 1,
      receivedPurchaseOrders: 1,
      receiptEvents: 2,
      partialReceipts: 1,
      missedDeliveries: 0,
      deferredPurchaseReviews: 0,
    },
  };
}

test('inventory flow exposes day-zero inbound orders, split receipts and stockouts without mutating inputs', () => {
  const input = episode();
  const before = structuredClone(input);
  const result = reconcileInventoryFlow(input);
  assert.deepEqual(result.firstStockout, { day: 1, sku: 'A', demanded: 2, sold: 1, lostSales: 1 });
  assert.deepEqual(
    result.daily.map((day) => day.products[0]),
    [
      {
        sku: 'A',
        openingStock: 1,
        openingOnOrder: 0,
        receivedUnits: 0,
        demanded: 0,
        sold: 0,
        lostSales: 0,
        orderedUnits: 4,
        closingStock: 1,
        onOrder: 4,
      },
      {
        sku: 'A',
        openingStock: 1,
        openingOnOrder: 4,
        receivedUnits: 0,
        demanded: 2,
        sold: 1,
        lostSales: 1,
        orderedUnits: 0,
        closingStock: 0,
        onOrder: 4,
      },
      {
        sku: 'A',
        openingStock: 0,
        openingOnOrder: 4,
        receivedUnits: 2,
        demanded: 2,
        sold: 2,
        lostSales: 0,
        orderedUnits: 0,
        closingStock: 0,
        onOrder: 2,
      },
      {
        sku: 'A',
        openingStock: 0,
        openingOnOrder: 2,
        receivedUnits: 2,
        demanded: 1,
        sold: 1,
        lostSales: 0,
        orderedUnits: 0,
        closingStock: 1,
        onOrder: 0,
      },
    ],
  );
  assert.deepEqual(input, before);
});

test('a missed first receipt leaves units inbound until the actual later delivery', () => {
  const input = episode();
  input.supplyProjection.products[0].dailyFirstReceiptBps[0] = 0;
  input.timeline[0].firstReceiptBps = 0;
  input.timeline[3] = {
    ...input.timeline[3],
    type: 'purchase_delivery_missed',
    quantity: 0,
    remaining: 4,
  };
  input.timeline[4] = { ...input.timeline[4], sold: 0, lost: 2 };
  input.timeline[6].quantity = 4;
  input.timeline[7].available = 3;
  input.timeline[9].inventoryPosition = 3;
  Object.assign(input.inventory[0], {
    onHand: 3,
    available: 3,
    sold: 2,
    lostSales: 3,
    stockoutDays: 2,
  });
  Object.assign(input.summary, {
    sold: 4,
    lostSales: 3,
    orders: 4,
    receiptEvents: 1,
    partialReceipts: 0,
    missedDeliveries: 1,
  });
  const result = reconcileInventoryFlow(input);
  assert.equal(result.daily[2].products[0].receivedUnits, 0);
  assert.equal(result.daily[2].products[0].onOrder, 4);
  assert.equal(result.daily[3].products[0].receivedUnits, 4);
  assert.equal(result.daily[3].products[0].closingStock, 3);
});

test('inventory flow retains unreceived prepaid units beyond the horizon', () => {
  const input = episode();
  input.days = 2;
  input.timeline = input.timeline.filter((event) => event.day <= 2);
  for (const product of input.demandForecast.products) product.dailyUnits.length = 2;
  for (const product of input.supplyProjection.products) {
    product.dailyLeadTimeDays.length = 3;
    product.dailyFirstReceiptBps.length = 3;
  }
  Object.assign(input.inventory[0], {
    onHand: 0,
    available: 0,
    onOrder: 2,
    demanded: 4,
    sold: 3,
    receivedUnits: 2,
  });
  Object.assign(input.inventory[1], { onHand: 2, available: 2, demanded: 1, sold: 1 });
  Object.assign(input.summary, {
    demanded: 5,
    sold: 4,
    orders: 3,
    receivedPurchaseOrders: 0,
    receiptEvents: 1,
  });
  const result = reconcileInventoryFlow(input);
  assert.equal(result.daily[2].products[0].onOrder, 2);
  assert.equal(result.daily[2].products[0].closingStock, 0);
});

test('inventory replay rejects impossible early sales even when offsetting later sales preserve final totals', () => {
  const input = episode();
  input.timeline[1] = { ...input.timeline[1], sold: 2, lost: 0, available: -1 };
  input.timeline[4] = { ...input.timeline[4], sold: 1, lost: 1 };
  assert.equal(
    input.timeline
      .filter((event) => event.sku === 'A' && event.type === 'demand')
      .reduce((sum, event) => sum + event.sold, 0),
    input.inventory[0].sold,
  );
  assert.throws(() => reconcileInventoryFlow(input), /inventory-flow reconciliation/);
});

test('inventory flow rejects missing demand, premature sales, corrupt paths and mismatched native totals', () => {
  for (const corrupt of [
    (value) => {
      value.timeline[3].day = 1;
    },
    (value) => {
      [value.timeline[3], value.timeline[4]] = [value.timeline[4], value.timeline[3]];
    },
    (value) => {
      value.timeline.splice(2, 1);
    },
    (value) => {
      value.timeline.splice(2, 0, { ...value.timeline[1] });
    },
    (value) => {
      value.timeline[1].demand = 3;
      value.timeline[1].lost = 2;
    },
    (value) => {
      value.timeline[1].available = 1;
    },
    (value) => {
      value.timeline[0].inventoryPosition = 0;
    },
    (value) => {
      value.timeline[9].inventoryPosition = 2;
    },
    (value) => {
      value.timeline[0].leadTimeDays = 1;
    },
    (value) => {
      value.timeline[0].firstReceiptBps = 10000;
    },
    (value) => {
      value.timeline[0].arrivalDay = 3;
    },
    (value) => {
      value.timeline[0].quantity = 0;
    },
    (value) => {
      value.timeline[1].sold = 0.5;
    },
    (value) => {
      value.timeline[1].sku = 'UNKNOWN';
    },
    (value) => {
      value.timeline[1].day = 0;
    },
    (value) => {
      value.timeline[1].day = -1;
    },
    (value) => {
      value.timeline[9].day = 4;
    },
    (value) => {
      value.timeline[9].type = 'restock';
    },
    (value) => {
      value.timeline.reverse();
    },
    (value) => {
      value.inventory[0].stockoutDays = 0;
    },
    (value) => {
      value.inventory[0].purchasedUnits = 3;
    },
    (value) => {
      value.inventory[0].onHand = 2;
    },
    (value) => {
      value.inventory[0].allocated = 1;
    },
    (value) => {
      value.inventory[0].onOrder = 1;
    },
    (value) => {
      value.inventory[0].receivedUnits = 3;
    },
    (value) => {
      value.summary.demanded += 1;
    },
    (value) => {
      value.summary.sold += 1;
    },
    (value) => {
      value.summary.lostSales += 1;
    },
    (value) => {
      value.summary.orders += 1;
    },
    (value) => {
      value.summary.receivedPurchaseOrders += 1;
    },
    (value) => {
      value.summary.partialReceipts += 1;
    },
    (value) => {
      value.summary.missedDeliveries += 1;
    },
    (value) => {
      value.supplyProjection.products[0].dailyLeadTimeDays[0] = 3;
    },
    (value) => {
      value.supplyProjection.products[0].dailyFirstReceiptBps[0] = 0;
    },
    (value) => {
      value.demandForecast.products[0].dailyUnits.length = 2;
    },
    (value) => {
      value.inventory.push({ ...value.inventory[0] });
    },
  ]) {
    const broken = episode();
    corrupt(broken);
    assert.throws(() => reconcileInventoryFlow(broken), /inventory-flow reconciliation/);
  }
});

test('receipts must refer to unique placed purchases and retain their selected split-delivery terms', () => {
  for (const corrupt of [
    (value) => {
      value.timeline[3].purchase = 'unknown';
    },
    (value) => {
      value.timeline[3].sku = 'B';
    },
    (value) => {
      value.timeline[3].quantity = 1;
      value.timeline[3].remaining = 3;
    },
    (value) => {
      value.timeline[3].remaining = 1;
    },
    (value) => {
      value.timeline[3].complete = true;
    },
    (value) => {
      value.timeline[3].nextArrivalDay = 4;
    },
    (value) => {
      value.timeline[3].type = 'purchase_delivery_missed';
    },
    (value) => {
      value.timeline[6].nextArrivalDay = 4;
    },
    (value) => {
      value.timeline.splice(1, 0, { ...value.timeline[0] });
    },
    (value) => {
      value.timeline.splice(4, 0, { ...value.timeline[3] });
    },
  ]) {
    const broken = episode();
    corrupt(broken);
    assert.throws(() => reconcileInventoryFlow(broken), /inventory-flow reconciliation/);
  }
});

test('overlapping partial purchase receipts reconcile by purchase alias and preserve input product order', () => {
  const input = episode();
  input.timeline.splice(3, 0, {
    ...input.timeline[0],
    day: 1,
    purchase: 'purchase-2',
    quantity: 2,
    inventoryPosition: 4,
    arrivalDay: 3,
  });
  input.timeline.splice(7, 0, {
    day: 3,
    type: 'purchase_received',
    sku: 'A',
    purchase: 'purchase-2',
    quantity: 1,
    remaining: 1,
    complete: false,
    nextArrivalDay: 4,
  });
  input.timeline.find(
    (event) => event.day === 3 && event.type === 'demand' && event.sku === 'A',
  ).available = 2;
  input.timeline.at(-1).inventoryPosition = 3;
  Object.assign(input.inventory[0], {
    onHand: 2,
    available: 2,
    onOrder: 1,
    purchasedUnits: 6,
    receivedUnits: 5,
  });
  Object.assign(input.summary, { purchaseOrders: 2, receiptEvents: 3, partialReceipts: 2 });
  input.products.reverse();
  const result = reconcileInventoryFlow(input);
  assert.deepEqual(
    result.daily[3].products.map((item) => item.sku),
    ['B', 'A'],
  );
  assert.equal(result.daily[3].products[1].receivedUnits, 3);
  assert.equal(result.daily[3].products[1].onOrder, 1);
});

test('zero-demand days carry physical balances without inventing a stockout or movement', () => {
  const input = episode();
  input.days = 1;
  input.products = [{ sku: 'A', initialStock: 0 }];
  input.demandForecast.products = [{ sku: 'A', dailyUnits: [0] }];
  input.supplyProjection.products = [
    {
      sku: 'A',
      dailyLeadTimeDays: [1, 1],
      dailyFirstReceiptBps: [10000, 10000],
      remainderDelayDays: 1,
    },
  ];
  input.timeline = [
    { day: 1, type: 'demand', sku: 'A', demand: 0, sold: 0, lost: 0, available: 0 },
  ];
  input.inventory = [
    {
      sku: 'A',
      onHand: 0,
      available: 0,
      allocated: 0,
      onOrder: 0,
      demanded: 0,
      sold: 0,
      lostSales: 0,
      purchasedUnits: 0,
      receivedUnits: 0,
      stockoutDays: 0,
    },
  ];
  input.summary = Object.fromEntries(Object.keys(input.summary).map((key) => [key, 0]));
  const result = reconcileInventoryFlow(input);
  assert.equal(result.firstStockout, null);
  assert.equal(result.daily.length, 2);
  assert.equal(result.daily[1].products[0].closingStock, 0);
  assert.equal(result.daily[1].products[0].lostSales, 0);
});

test('inventory flow independently checks deferred reviews and rejects off-schedule purchases or blocks', () => {
  const input = episode();
  input.days = 2;
  input.products[0] = {
    ...input.products[0],
    reorderPoint: 0,
    purchasing: { reviewIntervalDays: 3 },
  };
  input.demandForecast.products[0].dailyUnits = [1, 1];
  input.demandForecast.products[1].dailyUnits = [0, 1];
  for (const product of input.supplyProjection.products) {
    product.dailyLeadTimeDays.length = 3;
    product.dailyFirstReceiptBps.length = 3;
  }
  input.timeline = [
    { day: 1, type: 'demand', sku: 'A', demand: 1, sold: 1, lost: 0, available: 0 },
    { day: 1, type: 'demand', sku: 'B', demand: 0, sold: 0, lost: 0, available: 3 },
    {
      day: 1,
      type: 'purchase_review_deferred',
      sku: 'A',
      inventoryPosition: 0,
      reviewIntervalDays: 3,
      nextReviewDay: 3,
    },
    { day: 2, type: 'demand', sku: 'A', demand: 1, sold: 0, lost: 1, available: 0 },
    { day: 2, type: 'demand', sku: 'B', demand: 1, sold: 1, lost: 0, available: 2 },
    {
      day: 2,
      type: 'purchase_review_deferred',
      sku: 'A',
      inventoryPosition: 0,
      reviewIntervalDays: 3,
      nextReviewDay: 3,
    },
  ];
  Object.assign(input.inventory[0], {
    onHand: 0,
    available: 0,
    demanded: 2,
    sold: 1,
    purchasedUnits: 0,
    receivedUnits: 0,
  });
  Object.assign(input.inventory[1], { onHand: 2, available: 2, demanded: 1, sold: 1 });
  Object.assign(input.summary, {
    demanded: 3,
    sold: 2,
    orders: 2,
    purchaseOrders: 0,
    receivedPurchaseOrders: 0,
    receiptEvents: 0,
    partialReceipts: 0,
    deferredPurchaseReviews: 2,
  });
  const result = reconcileInventoryFlow(input);
  assert.equal(result.firstStockout.day, 2);
  assert.ok(
    result.daily.every((day) => day.products.every((product) => product.orderedUnits === 0)),
  );
  for (const corrupt of [
    (value) => {
      value.products[0].purchasing.reviewIntervalDays = 1;
    },
    (value) => {
      value.products[0].purchasing.reviewIntervalDays = 2;
      value.timeline[2].reviewIntervalDays = 2;
      value.timeline[2].nextReviewDay = 2;
    },
    (value) => {
      value.timeline[2].reviewIntervalDays = 2;
    },
    (value) => {
      value.timeline[2].nextReviewDay = 2;
    },
    (value) => {
      value.timeline[2].inventoryPosition = 1;
    },
    (value) => {
      value.products[0].reorderPoint = -1;
    },
    (value) => {
      value.summary.deferredPurchaseReviews = 3;
    },
    (value) => {
      value.timeline[2].type = 'purchase_budget_blocked';
    },
    (value) => {
      value.timeline[2] = {
        ...value.timeline[2],
        type: 'purchase_order_sent',
        purchase: 'off-cycle',
        quantity: 1,
        leadTimeDays: 2,
        firstReceiptBps: 5000,
        arrivalDay: 3,
      };
    },
    (value) => {
      value.timeline.splice(3, 0, { ...value.timeline[2] });
    },
  ]) {
    const broken = structuredClone(input);
    corrupt(broken);
    assert.throws(() => reconcileInventoryFlow(broken), /inventory-flow reconciliation/);
  }
});
