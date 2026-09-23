/**
 * Enum strictness at the binding boundary.
 *
 * Every hand-written enum parser used to map an unknown string onto a default
 * variant: a garbage `accountType` became an asset, a garbage `promotionType`
 * became percentage-off, and an unknown status *filter* became "no filter".
 * The README promises a malformed input is refused with `code: 'VALIDATION'`,
 * never coerced, so each family below checks that an unknown value rejects
 * with a message naming the field and listing the accepted spellings, and
 * that a known spelling in mixed case still works.
 */

'use strict';

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { Commerce } = require('../index.js');

const UNKNOWN_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

/** Rejects with `VALIDATION`, names the field, and lists the accepted spellings. */
const invalid = (field, value) => (err) =>
  err.code === 'VALIDATION' &&
  err.message.includes(`Invalid ${field} '${value}'`) &&
  /expected one of:/.test(err.message);

async function customer(commerce, tag = 'strict') {
  return commerce.customers.create({ email: `${tag}@example.com`, firstName: 'S', lastName: 'E' });
}

async function orderFor(commerce, cust, sku = 'SE-SKU') {
  await commerce.inventory.createItem({ sku, name: 'Widget', initialQuantity: 5 });
  return commerce.orders.create({
    customerId: cust.id,
    items: [{ sku, name: 'Widget', quantity: 1, unitPriceExact: '1.00' }],
  });
}

async function shippedOrder(commerce, cust) {
  const order = await orderFor(commerce, cust);
  await commerce.orders.updateStatus(order.id, 'confirmed');
  await commerce.orders.ship(order.id, 'TRACK-SE');
  return commerce.orders.get(order.id);
}

test('accountsReceivable.createCreditMemo refuses an unknown reason instead of filing it as Other', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  await assert.rejects(
    commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'because', amount: 1 }),
    invalid('credit memo reason', 'because'),
  );
  assert.equal((await commerce.accountsReceivable.listCreditMemos()).length, 0);

  const goodwill = await commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'GOODWILL', amount: 1 });
  assert.equal(goodwill.reason, 'GoodwillAdjustment');
  const other = await commerce.accountsReceivable.createCreditMemo({ customerId: cust.id, reason: 'Other', amount: 1 });
  assert.equal(other.reason, 'Other');
});

test('workOrders.create refuses an unknown priority instead of dropping it', async () => {
  const commerce = new Commerce(':memory:');
  const product = await commerce.products.create({ name: 'WO Product' });
  await assert.rejects(
    commerce.workOrders.create({ productId: product.id, quantityToBuild: 1, priority: 'whenever' }),
    invalid('work order priority', 'whenever'),
  );
  assert.equal((await commerce.workOrders.list()).length, 0);

  const wo = await commerce.workOrders.create({ productId: product.id, quantityToBuild: 1, priority: 'URGENT' });
  assert.match(String(wo.priority), /urgent/i);
});

test('costAccounting.setItemCost refuses an unknown cost method instead of averaging', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.costAccounting.setItemCost({ sku: 'CM-1', costMethod: 'made_up', standardCost: 1 }),
    invalid('cost method', 'made_up'),
  );
  assert.equal(await commerce.costAccounting.getItemCost('CM-1'), null);

  const cost = await commerce.costAccounting.setItemCost({ sku: 'CM-1', costMethod: 'LIFO', standardCost: 1 });
  assert.equal(cost.costMethod, 'Lifo');
});

test('analytics refuses an unknown period or granularity instead of reporting the default window', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(commerce.analytics.salesSummary({ period: 'fortnight' }), invalid('analytics period', 'fortnight'));
  await assert.rejects(
    commerce.analytics.salesSummary({ granularity: 'decade' }),
    invalid('analytics granularity', 'decade'),
  );
  await assert.rejects(commerce.analytics.revenueForecast(2, 'decade'), invalid('analytics granularity', 'decade'));

  await commerce.analytics.salesSummary({ period: 'LAST_7_DAYS', granularity: 'Weekly' });
  await commerce.analytics.revenueForecast(1, 'QUARTERLY');
});

test('backorder.createBackorder refuses an unknown priority instead of filing it as Normal', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const order = await orderFor(commerce, cust);
  const base = { orderId: order.id, customerId: cust.id, sku: 'SE-SKU', quantity: 1 };
  await assert.rejects(
    commerce.backorder.createBackorder({ ...base, priority: 'whenever' }),
    invalid('backorder priority', 'whenever'),
  );
  assert.equal((await commerce.backorder.listBackorders()).length, 0);

  const bo = await commerce.backorder.createBackorder({ ...base, priority: 'High' });
  assert.equal(bo.priority, 'High');
});

test('warranties refuse an unknown type and an unknown claim resolution', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  await assert.rejects(
    commerce.warranties.create({ customerId: cust.id, warrantyType: 'forever' }),
    invalid('warranty type', 'forever'),
  );
  assert.equal(await commerce.warranties.count(), 0);

  const warranty = await commerce.warranties.create({ customerId: cust.id, warrantyType: 'LIFETIME' });
  assert.equal(warranty.warrantyType, 'lifetime');

  const claim = await commerce.warranties.createClaim({ warrantyId: warranty.id, issueDescription: 'cracked' });
  await assert.rejects(
    commerce.warranties.completeClaim(claim.id, 'shrug'),
    invalid('warranty claim resolution', 'shrug'),
  );
  // Nothing was recorded by the refused call: the claim still walks its lifecycle.
  await commerce.warranties.approveClaim(claim.id);
  const completed = await commerce.warranties.completeClaim(claim.id, 'Store_Credit');
  assert.equal(completed.resolution, 'store_credit');
});

test('generalLedger.createAccount refuses an unknown account type instead of booking an asset', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.generalLedger.createAccount({ accountNumber: '9100', name: 'Mystery', accountType: 'liabilities' }),
    invalid('account type', 'liabilities'),
  );
  const account = await commerce.generalLedger.createAccount({ accountNumber: '9100', name: 'Rent', accountType: 'EXPENSE' });
  assert.equal(account.accountType, 'Expense');
});

test('currency.updateSettings refuses an unknown rounding mode instead of rounding half-up', async () => {
  const commerce = new Commerce(':memory:');
  const base = { baseCurrency: 'USD', enabledCurrencies: ['USD'] };
  await assert.rejects(
    commerce.currency.updateSettings({ ...base, roundingMode: 'banker' }),
    invalid('rounding mode', 'banker'),
  );
  const settings = await commerce.currency.updateSettings({ ...base, roundingMode: 'HALF_EVEN' });
  assert.equal(settings.roundingMode, 'half_even');
});

test('promotions refuse unknown type, trigger, target, stacking, and status values', async () => {
  const commerce = new Commerce(':memory:');
  const p = commerce.promotions;
  await assert.rejects(p.create({ name: 'x', promotionType: 'percent' }), invalid('promotion type', 'percent'));
  await assert.rejects(p.create({ name: 'x', trigger: 'manual' }), invalid('promotion trigger', 'manual'));
  await assert.rejects(p.create({ name: 'x', target: 'cart' }), invalid('promotion target', 'cart'));
  await assert.rejects(p.create({ name: 'x', stacking: 'maybe' }), invalid('promotion stacking behavior', 'maybe'));
  assert.equal((await p.list()).length, 0);

  // A status filter that matches nothing used to list everything.
  await assert.rejects(p.list({ status: 'live' }), invalid('promotion status', 'live'));
  await assert.rejects(p.list({ promotionType: 'percent' }), invalid('promotion type', 'percent'));
  await assert.rejects(p.list({ trigger: 'manual' }), invalid('promotion trigger', 'manual'));
  await assert.rejects(p.listCoupons({ status: 'on' }), invalid('coupon status', 'on'));

  const promo = await p.create({
    name: 'Mixed case',
    promotionType: 'FIXED_AMOUNT_OFF',
    fixedAmountOff: 1,
    trigger: 'Coupon',
    target: 'Product',
    stacking: 'Exclusive',
  });
  assert.equal(promo.promotionType, 'fixedamountoff');
  assert.equal(promo.trigger, 'couponcode');
  assert.equal(promo.target, 'product');
  assert.equal(promo.stacking, 'exclusive');
  await assert.rejects(p.update(promo.id, { status: 'live' }), invalid('promotion status', 'live'));
  assert.equal((await p.list({ status: 'DRAFT' })).length, 1);
  assert.equal((await p.listCoupons({ status: 'ACTIVE' })).length, 0);
});

test('quality refuses unknown inspection type, NCR source, severity, and hold type', async () => {
  const commerce = new Commerce(':memory:');
  const q = commerce.quality;
  const inspection = { referenceType: 'shipment', referenceId: UNKNOWN_ID };
  await assert.rejects(
    q.createInspection({ ...inspection, inspectionType: 'visual' }),
    invalid('inspection type', 'visual'),
  );
  const ncr = { sku: 'Q-1', quantityAffected: 1, description: 'bent' };
  await assert.rejects(
    q.createNcr({ ...ncr, source: 'moon', severity: 'major' }),
    invalid('non-conformance source', 'moon'),
  );
  await assert.rejects(q.createNcr({ ...ncr, source: 'supplier', severity: 'meh' }), invalid('severity', 'meh'));
  await assert.rejects(
    q.createHold({ sku: 'Q-1', quantityHeld: 1, reason: 'r', holdType: 'vibes' }),
    invalid('hold type', 'vibes'),
  );
  assert.equal((await q.listInspections()).length, 0);
  assert.equal((await q.listHolds()).length, 0);

  await q.createInspection({ ...inspection, inspectionType: 'IN_PROCESS' });
  await q.createInspection({ ...inspection, inspectionType: 'Incoming' });
  await q.createNcr({ ...ncr, source: 'Supplier', severity: 'MAJOR' });
  await q.createHold({ sku: 'Q-1', quantityHeld: 1, reason: 'r', holdType: 'Recall' });
  assert.equal((await q.listInspections()).length, 2);
  assert.equal((await q.listHolds()).length, 1);
});

test('shipments.create refuses an unknown carrier or method instead of storing other/standard', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const order = await orderFor(commerce, cust);
  const base = { orderId: order.id, recipientName: 'R', shippingAddress: '1 Main St' };
  await assert.rejects(commerce.shipments.create({ ...base, carrier: 'pigeon' }), invalid('shipping carrier', 'pigeon'));
  await assert.rejects(
    commerce.shipments.create({ ...base, shippingMethod: 'teleport' }),
    invalid('shipping method', 'teleport'),
  );
  assert.equal((await commerce.shipments.list()).length, 0);

  const shipment = await commerce.shipments.create({ ...base, carrier: 'FedEx', shippingMethod: 'Two_Day' });
  assert.equal(shipment.carrier, 'fed_ex');
  assert.equal(shipment.shippingMethod, 'two_day');
  const explicit = await commerce.shipments.create({ ...base, carrier: 'OTHER' });
  assert.equal(explicit.carrier, 'other');
});

test('receiving.createReceipt refuses an unknown receipt type instead of booking a purchase order', async () => {
  const commerce = new Commerce(':memory:');
  const wh = await commerce.warehouse.createWarehouse({ code: 'WH-SE', name: 'Strict' });
  await assert.rejects(
    commerce.receiving.createReceipt({ receiptType: 'gift', warehouseId: wh.id }),
    invalid('receipt type', 'gift'),
  );
  assert.equal(await commerce.receiving.countReceipts(), 0);

  const receipt = await commerce.receiving.createReceipt({ receiptType: 'PO', warehouseId: wh.id });
  assert.equal(receipt.receiptType, 'PurchaseOrder');
});

test('returns.create refuses an unknown reason instead of filing it as other', async () => {
  const commerce = new Commerce(':memory:');
  const cust = await customer(commerce);
  const order = await shippedOrder(commerce, cust);
  const items = [{ orderItemId: order.items[0].id, quantity: 1 }];
  await assert.rejects(
    commerce.returns.create({ orderId: order.id, reason: 'because', items }),
    invalid('return reason', 'because'),
  );
  assert.equal((await commerce.returns.list()).length, 0);

  const ret = await commerce.returns.create({ orderId: order.id, reason: 'DAMAGED', items });
  assert.equal(ret.reason, 'damaged');
});

test('warehouse refuses an unknown warehouse type or location type', async () => {
  const commerce = new Commerce(':memory:');
  await assert.rejects(
    commerce.warehouse.createWarehouse({ code: 'WH-1', name: 'x', warehouseType: 'garage' }),
    invalid('warehouse type', 'garage'),
  );
  const wh = await commerce.warehouse.createWarehouse({ code: 'WH-1', name: 'x', warehouseType: 'THIRD_PARTY' });
  // Outputs render the engine's Debug form (see `WarehouseType` in index.d.ts).
  assert.equal(wh.warehouseType, 'ThirdParty');

  // `bin` used to become a pick location without a word.
  await assert.rejects(
    commerce.warehouse.createLocation({ warehouseId: wh.id, locationType: 'bin' }),
    invalid('location type', 'bin'),
  );
  const location = await commerce.warehouse.createLocation({ warehouseId: wh.id, locationType: 'PICK' });
  assert.match(String(location.locationType), /pick/i);
});

test('tax refuses unknown tax type, category, jurisdiction level, exemption type, and settings methods', async () => {
  const commerce = new Commerce(':memory:');
  const tax = commerce.tax;
  const cust = await customer(commerce);

  await assert.rejects(
    tax.createJurisdiction({ name: 'Mars', code: 'MARS', countryCode: 'US', level: 'planet' }),
    invalid('jurisdiction level', 'planet'),
  );
  await assert.rejects(tax.listJurisdictions({ level: 'planet' }), invalid('jurisdiction level', 'planet'));
  // A fresh store is demo-seeded with the real US states, so use a code it lacks.
  const state = await tax.createJurisdiction({ name: 'Strictland', code: 'ZZ-STRICT', countryCode: 'US', stateCode: 'ZZ', level: 'STATE' });
  assert.equal(state.level, 'state');

  const rate = { jurisdictionId: state.id, name: 'r', rate: 0.1, effectiveFrom: '2026-01-01' };
  await assert.rejects(tax.createRate({ ...rate, taxType: 'tariff' }), invalid('tax type', 'tariff'));
  await assert.rejects(tax.createRate({ ...rate, productCategory: 'toys' }), invalid('product tax category', 'toys'));
  await assert.rejects(tax.listRates({ taxType: 'tariff' }), invalid('tax type', 'tariff'));
  await assert.rejects(tax.listRates({ productCategory: 'toys' }), invalid('product tax category', 'toys'));
  // The demo-seeded store ships rates of its own; scope the emptiness check to the new jurisdiction.
  assert.equal((await tax.listRates({ jurisdictionId: state.id })).length, 0);
  const created = await tax.createRate({ ...rate, taxType: 'VAT', productCategory: 'Food' });
  assert.equal(created.taxType, 'vat');
  assert.equal(created.productCategory, 'food');

  const exemption = { customerId: cust.id, effectiveFrom: '2026-01-01' };
  await assert.rejects(
    tax.createExemption({ ...exemption, exemptionType: 'vibes' }),
    invalid('exemption type', 'vibes'),
  );
  await assert.rejects(
    tax.createExemption({ ...exemption, exemptionType: 'resale', exemptCategories: ['toys'] }),
    invalid('product tax category', 'toys'),
  );
  const nonProfit = await tax.createExemption({ ...exemption, exemptionType: 'NON_PROFIT' });
  assert.equal(nonProfit.exemptionType, 'nonprofit');

  await assert.rejects(tax.updateSettings({ calculationMethod: 'sideways' }), invalid('tax calculation method', 'sideways'));
  await assert.rejects(tax.updateSettings({ compoundMethod: 'stacked' }), invalid('tax compound method', 'stacked'));
  await assert.rejects(tax.updateSettings({ defaultProductCategory: 'toys' }), invalid('product tax category', 'toys'));
  const settings = await tax.updateSettings({
    calculationMethod: 'INCLUSIVE',
    compoundMethod: 'Separate',
    defaultProductCategory: 'Food',
  });
  assert.equal(settings.calculationMethod, 'inclusive');
  assert.equal(settings.compoundMethod, 'separate');
  assert.equal(settings.defaultProductCategory, 'food');

  const address = { country: 'US', state: 'CA' };
  await assert.rejects(tax.calculateForItem(1, 1, 'toys', address), invalid('product tax category', 'toys'));
  await assert.rejects(tax.getEffectiveRate(address, 'toys'), invalid('product tax category', 'toys'));
  await tax.calculateForItem(1, 1, 'Standard', address);
});

test('lots and serials refuse a malformed timestamp instead of writing the record without it', async () => {
  const commerce = new Commerce(':memory:');
  const isDate = (field) => (err) => err.code === 'VALIDATION' && err.message.includes(`Invalid ${field}`);
  await assert.rejects(
    commerce.lots.create({ sku: 'L-1', quantityProduced: 1, productionDate: 'garbage' }),
    isDate('production date'),
  );
  await assert.rejects(
    commerce.lots.create({ sku: 'L-1', quantityProduced: 1, expirationDate: 'next-week' }),
    isDate('expiration date'),
  );
  await assert.rejects(commerce.serials.create({ sku: 'S-1', manufacturedAt: 'yesterday' }), isDate('manufactured at'));
  assert.equal(await commerce.lots.count(), 0);
  assert.equal((await commerce.serials.list()).length, 0);

  const lot = await commerce.lots.create({ sku: 'L-1', quantityProduced: 1, expirationDate: '2030-01-01T00:00:00Z' });
  assert.ok(lot.expirationDate);
  // `SerialOutput` does not carry the timestamp; prove it was stored through the filter.
  const serial = await commerce.serials.create({ sku: 'S-1', manufacturedAt: '2026-05-01T00:00:00Z' });
  const stamped = await commerce.serials.list({ manufacturedAfter: '2026-04-30T00:00:00Z' });
  assert.deepEqual(
    stamped.map((s) => s.id),
    [serial.id],
  );
});
