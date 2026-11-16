// Native calls whose contract changed in engine 1.41. A packed CLI pins its own
// engine release, but the pre-release consumer check runs the CLI against the
// published engine, which can be the previous one. Each helper keeps the 1.41
// behavior and falls back to what earlier engines always needed.

/**
 * Cost opening stock at the plan's unit cost: from 1.41 the engine refuses to
 * issue on-hand units that no cost layer covers. Earlier engines take float
 * costs and need no cost before stock is placed, so their refusal to convert
 * the decimal string leaves the item uncosted.
 */
export async function costOpeningStock(commerce, sku, unitCost) {
  try {
    await commerce.costAccounting.setItemCost({
      sku,
      costMethod: 'average',
      standardCost: unitCost,
    });
  } catch (error) {
    if (!String(error?.message).includes('into rust type `f64`')) throw error;
  }
}

async function onHand(commerce, sku) {
  const stock = await commerce.inventory.getStock(sku);
  return Number(stock?.totalOnHand ?? 0);
}

/**
 * Receive purchase-order lines (`{ itemId, sku, quantity }`, quantity a decimal
 * string) into a location. From 1.41 the receipt posts its stock in the same
 * transaction; earlier engines ignore the location and post nothing, so the
 * units are added separately. A refused receipt adds nothing either way.
 */
export async function receivePurchase(commerce, purchaseId, lines, warehouseId, locationId) {
  const before = new Map();
  for (const { sku } of lines) before.set(sku, await onHand(commerce, sku));
  const purchase = await commerce.purchaseOrders.receive(
    purchaseId,
    lines.map(({ itemId, quantity }) => ({ itemId, quantityReceivedExact: quantity })),
    warehouseId,
    locationId,
  );
  const received = new Map();
  for (const { sku, quantity } of lines)
    received.set(sku, (received.get(sku) ?? 0) + Number(quantity));
  for (const [sku, quantity] of received) {
    if ((await onHand(commerce, sku)) === before.get(sku))
      await commerce.inventory.adjust(sku, quantity, 'Simulated purchase receipt');
  }
  return purchase;
}
