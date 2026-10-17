import {
  addDecimals,
  subtractDecimals,
  compareDecimals,
  multiplyDecimals,
} from '../utils/exact-decimal.js';

// Inventory quantities are bounded integers; every monetary operand is an
// exact decimal string. These checks bind projections to final native state.
export function reconcilePlan({
  startingCash,
  openingInventoryValue,
  summary,
  inventory,
  products,
}) {
  if (
    inventory.length !== products.length ||
    new Set(inventory.map((item) => item.sku)).size !== inventory.length
  )
    throw new Error('Simulation inventory reconciliation requires exactly one row per product');
  for (const item of inventory) {
    const opening = products.find((product) => product.sku === item.sku);
    if (
      !opening ||
      item.onHand !== opening.initialStock + item.receivedUnits - item.sold ||
      item.allocated !== 0 ||
      item.available !== item.onHand ||
      item.onOrder !== item.purchasedUnits - item.receivedUnits ||
      item.demanded !== item.sold + item.lostSales
    ) {
      throw new Error(`Simulation inventory reconciliation failed for ${item.sku}`);
    }
  }
  const valueOf = (field, price = 'unitCost') =>
    addDecimals(
      inventory.map((item) =>
        multiplyDecimals(
          products.find((product) => product.sku === item.sku)[price],
          String(item[field]),
        ),
      ),
    );
  const expectedValues = {
    endingInventoryValue: valueOf('onHand'),
    inTransitInventoryValue: valueOf('onOrder'),
    costOfGoodsSold: valueOf('sold'),
    purchasingSpend: valueOf('purchasedUnits'),
    revenue: valueOf('sold', 'unitPrice'),
  };
  const expectedOpening = addDecimals(
    products.map((item) => multiplyDecimals(item.unitCost, String(item.initialStock))),
  );
  if (
    compareDecimals(openingInventoryValue, expectedOpening) !== 0 ||
    Object.entries(expectedValues).some(
      ([field, value]) => compareDecimals(summary[field], value) !== 0,
    )
  )
    throw new Error(
      'Simulation inventory-value reconciliation does not match native quantities and prices',
    );
  const expectedCash = subtractDecimals(
    addDecimals(startingCash, summary.revenue),
    summary.purchasingSpend,
  );
  if (
    compareDecimals(summary.endingCash, expectedCash) !== 0 ||
    compareDecimals(summary.endingCash, '0') < 0
  ) {
    throw new Error('Simulation cash reconciliation failed');
  }
  const openingAssets = addDecimals(startingCash, openingInventoryValue);
  const endingAssets = addDecimals(
    summary.endingCash,
    summary.endingInventoryValue,
    summary.inTransitInventoryValue,
  );
  if (
    compareDecimals(subtractDecimals(endingAssets, openingAssets), summary.grossMargin) !== 0 ||
    compareDecimals(
      subtractDecimals(summary.revenue, summary.costOfGoodsSold),
      summary.grossMargin,
    ) !== 0
  ) {
    throw new Error('Simulation inventory-value reconciliation failed');
  }
  return {
    stockBalanced: true,
    cashBalanced: true,
    inventoryValueBalanced: true,
    openingAssets,
    endingAssets,
  };
}
