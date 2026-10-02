/**
 * Declaration-surface and pagination tests for the customers, orders,
 * products, custom objects, inventory, returns, payments, shipments,
 * warranties, purchase orders, invoices, BOM, work orders, carts, analytics
 * and currency APIs.
 *
 * The first block reads the regenerated `index.d.ts` and proves the literal
 * unions from `scripts/types/g1.d.ts` are declared and referenced from the
 * generated interfaces. The rest drives the paginated `list()` methods
 * through the native binding.
 */

const { readFileSync } = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const { Commerce } = require('../index.js');

const dts = readFileSync(path.join(__dirname, '..', 'index.d.ts'), 'utf8');

async function rejects(fn) {
  try {
    await fn();
  } catch (error) {
    return error;
  }
  throw new assert.AssertionError({ message: 'expected the call to reject' });
}

/** Extract the body of `export interface Name { ... }` from the d.ts. */
function interfaceBody(name) {
  const match = dts.match(new RegExp(`export interface ${name} \\{([^}]*)\\}`));
  assert.ok(match, `index.d.ts must declare interface ${name}`);
  return match[1];
}

/** Extract the body of `export declare class Name { ... }` from the d.ts. */
function classBody(name) {
  const match = dts.match(new RegExp(`export declare class ${name} \\{([^}]*)\\}`));
  assert.ok(match, `index.d.ts must declare class ${name}`);
  return match[1];
}

// ---------------------------------------------------------------------------
// index.d.ts surface
// ---------------------------------------------------------------------------

test('g1 literal unions are declared in index.d.ts', () => {
  const unions = {
    CustomerStatus: ['active', 'deleted'],
    AddressType: ['shipping', 'billing', 'both'],
    OrderStatus: ['partially_shipped', 'refunded'],
    OrderStatusUpdate: ['confirmed', 'cancelled'],
    PaymentStatus: ['partially_refunded'],
    FulfillmentStatus: ['partially_fulfilled'],
    StockPolicy: ['allow_backorder', 'reject_if_insufficient', 'reject-if-insufficient'],
    ProductStatus: ['draft', 'active', 'archived'],
    CustomFieldType: ['date_time', 'json'],
    CustomFieldTypeInput: ['datetime', 'bool'],
    ReservationStatus: ['allocated', 'fulfilled'],
    ReturnStatus: ['in_transit', 'inspecting'],
    ReturnReason: ['better_price_found', 'other'],
    PaymentTransactionStatus: ['requires_action', 'disputed'],
    PaymentMethodType: ['credit_card', 'ach', 'cod', 'usdc'],
    RefundStatus: ['processing', 'cancelled'],
    ShipmentStatus: ['ready_to_ship', 'out_for_delivery'],
    ShippingCarrier: ['fed_ex', 'laser_ship'],
    ShippingCarrierInput: ['fedex', 'other'],
    ShippingCarrierFilter: ['ontrac'],
    ShipmentMethod: ['two_day', 'same_day'],
    ShipmentMethodInput: ['twoday', 'sameday'],
    WarrantyStatus: ['voided', 'transferred'],
    WarrantyType: ['accidental_damage', 'comprehensive'],
    WarrantyTypeInput: ['lifetime'],
    WarrantyClaimStatus: ['info_requested', 'under_review'],
    WarrantyClaimResolution: ['store_credit', 'none'],
    WarrantyClaimResolutionInput: ['storecredit'],
    PurchaseOrderStatus: ['pending_approval', 'on_hold'],
    InvoiceStatus: ['written_off', 'disputed'],
    BomStatus: ['obsolete'],
    WorkOrderStatus: ['partially_completed', 'on_hold'],
    WorkOrderPriority: ['low', 'urgent'],
    CartStatus: ['ready_for_payment', 'payment_pending'],
    CartPaymentStatus: ['method_selected', 'captured'],
    FulfillmentType: ['shipping', 'pick-up', 'digital'],
    AnalyticsPeriod: ['last30days', 'this_quarter', 'all'],
    AnalyticsGranularity: ['hourly', 'quarter'],
    DemandTrend: ['Rising', 'Stable', 'Falling'],
    RoundingMode: ['half_even', 'half_up'],
  };
  for (const [name, members] of Object.entries(unions)) {
    const match = dts.match(new RegExp(`export type ${name} =([^\\n]*(?:\\n  \\|[^\\n]*)*)`));
    assert.ok(match, `index.d.ts must declare type ${name}`);
    for (const member of members) {
      assert.ok(match[1].includes(`'${member}'`), `${name} must include '${member}'`);
    }
  }
  assert.match(dts, /export type CustomerMetadata = Record<string, unknown>/);
});

test('g1 unions are referenced from the generated interfaces', () => {
  const refs = {
    CustomerOutput: ['status: CustomerStatus', 'metadata?: CustomerMetadata'],
    CreateCustomerInput: ['metadata?: CustomerMetadata'],
    UpdateCustomerInput: ['status?: CustomerStatus'],
    CreateCustomerAddressInput: ['addressType?: AddressType'],
    CustomerAddressOutput: ['addressType: AddressType'],
    CreateOrderInput: ['stockPolicy?: StockPolicy'],
    CreateOrderExactInput: ['stockPolicy?: StockPolicy'],
    OrderOutput: [
      'status: OrderStatus',
      'paymentStatus: PaymentStatus',
      'fulfillmentStatus: FulfillmentStatus',
    ],
    ProductOutput: ['status: ProductStatus'],
    UpdateProductInput: ['status?: ProductStatus'],
    CustomFieldDefinitionInput: ['fieldType: CustomFieldTypeInput'],
    CustomFieldDefinitionOutput: ['fieldType: CustomFieldType'],
    ReservationOutput: ['status: ReservationStatus'],
    CreateReturnInput: ['reason: ReturnReason'],
    ReturnOutput: ['status: ReturnStatus', 'reason: ReturnReason'],
    CreatePaymentInput: ['paymentMethod?: PaymentMethodType'],
    CreatePaymentExactInput: ['paymentMethod?: PaymentMethodType'],
    PaymentOutput: ['status: PaymentTransactionStatus'],
    RefundOutput: ['status: RefundStatus'],
    CreateShipmentInput: ['carrier?: ShippingCarrierInput', 'shippingMethod?: ShipmentMethodInput'],
    ShipmentOutput: [
      'status: ShipmentStatus',
      'carrier: ShippingCarrier',
      'shippingMethod: ShipmentMethod',
    ],
    CreateWarrantyInput: ['warrantyType?: WarrantyTypeInput'],
    WarrantyOutput: ['status: WarrantyStatus', 'warrantyType: WarrantyType'],
    WarrantyClaimOutput: ['status: WarrantyClaimStatus', 'resolution: WarrantyClaimResolution'],
    PurchaseOrderOutput: ['status: PurchaseOrderStatus'],
    InvoiceOutput: ['status: InvoiceStatus'],
    BomOutput: ['status: BomStatus'],
    CreateWorkOrderInput: ['priority?: WorkOrderPriority'],
    WorkOrderOutput: ['status: WorkOrderStatus', 'priority: WorkOrderPriority'],
    CartOutput: [
      'status: CartStatus',
      'paymentStatus: CartPaymentStatus',
      'fulfillmentType?: FulfillmentType',
    ],
    AnalyticsQueryInput: ['period?: AnalyticsPeriod', 'granularity?: AnalyticsGranularity'],
    DemandForecastOutput: ['trend: DemandTrend'],
    StoreCurrencySettingsInput: ['roundingMode?: RoundingMode'],
    StoreCurrencySettingsOutput: ['roundingMode: RoundingMode'],
    // filters
    CustomerFilterInput: ['status?: CustomerStatus', 'limit?: number', 'offset?: number'],
    OrderFilterInput: [
      "status?: OrderStatus | 'partiallyshipped' | 'canceled'",
      'paymentStatus?: PaymentStatus',
      'fulfillmentStatus?: FulfillmentStatus',
      'customerId?: string',
    ],
    ProductFilterInput: ['status?: ProductStatus', 'search?: string'],
    ReturnFilterInput: ['status?: ReturnStatus', 'reason?: ReturnReason'],
    PaymentFilterInput: [
      'status?: PaymentTransactionStatus',
      'paymentMethod?: PaymentMethodType',
    ],
    ShipmentFilterInput: ['status?: ShipmentStatus', 'carrier?: ShippingCarrierFilter'],
    WarrantyFilterInput: ['status?: WarrantyStatus', 'warrantyType?: WarrantyType'],
    InvoiceFilterInput: ['status?: InvoiceStatus'],
    BomFilterInput: ['status?: BomStatus'],
    CartFilterInput: ['status?: CartStatus'],
    SupplierFilterInput: ['activeOnly?: boolean', 'limit?: number'],
    PurchaseOrderFilterInput: ["status?: PurchaseOrderStatus | 'canceled'"],
    WorkOrderFilterInput: ['status?: WorkOrderStatus', 'priority?: WorkOrderPriority'],
    InspectionFilterInput: ["inspectionType?: 'incoming' | 'receiving'"],
    NcrFilterInput: ["severity?: 'critical' | 'major' | 'minor' | 'observation'"],
  };
  for (const [name, fields] of Object.entries(refs)) {
    const body = interfaceBody(name);
    for (const field of fields) {
      assert.ok(body.includes(field), `${name} must declare \`${field}\``);
    }
  }
});

test('g1 method signatures carry unions and optional filters', () => {
  const methods = {
    Customers: [
      'list(filter?: CustomerFilterInput | undefined | null): Promise<Array<CustomerOutput>>',
      'setDefaultAddress(customerId: string, addressId: string, addressType: AddressType): Promise<void>',
    ],
    Orders: [
      'list(filter?: OrderFilterInput | undefined | null): Promise<Array<OrderOutput>>',
      'updateStatus(id: string, status: OrderStatusUpdate): Promise<OrderOutput>',
    ],
    Products: ['list(filter?: ProductFilterInput | undefined | null): Promise<Array<ProductOutput>>'],
    Returns: ['list(filter?: ReturnFilterInput | undefined | null): Promise<Array<ReturnOutput>>'],
    Payments: ['list(filter?: PaymentFilterInput | undefined | null): Promise<Array<PaymentOutput>>'],
    Shipments: [
      'list(filter?: ShipmentFilterInput | undefined | null): Promise<Array<ShipmentOutput>>',
    ],
    Warranties: [
      'list(filter?: WarrantyFilterInput | undefined | null): Promise<Array<WarrantyOutput>>',
      'completeClaim(id: string, resolution: WarrantyClaimResolutionInput): Promise<WarrantyClaimOutput>',
    ],
    PurchaseOrders: [
      'listSuppliers(filter?: SupplierFilterInput | undefined | null): Promise<Array<SupplierOutput>>',
    ],
    Invoices: ['list(filter?: InvoiceFilterInput | undefined | null): Promise<Array<InvoiceOutput>>'],
    Bom: ['list(filter?: BomFilterInput | undefined | null): Promise<Array<BomOutput>>'],
    Carts: ['list(filter?: CartFilterInput | undefined | null): Promise<Array<CartOutput>>'],
    Analytics: [
      'revenueForecast(periodsAhead?: number | undefined | null, granularity?: AnalyticsGranularity | undefined | null): Promise<Array<RevenueForecastOutput>>',
    ],
  };
  for (const [name, signatures] of Object.entries(methods)) {
    const body = classBody(name);
    for (const signature of signatures) {
      assert.ok(body.includes(signature), `${name} must declare \`${signature}\``);
    }
  }
});

// ---------------------------------------------------------------------------
// Pagination behaviour
// ---------------------------------------------------------------------------

async function seedCustomers(commerce, n, prefix = 'page') {
  const out = [];
  for (let i = 0; i < n; i += 1) {
    out.push(
      await commerce.customers.create({
        email: `${prefix}${i}@example.com`,
        firstName: 'Page',
        lastName: `Number${i}`,
      }),
    );
  }
  return out;
}

/** A product with one variant and a tracked inventory item for its SKU. */
async function seedProductWithVariant(commerce, name, sku, price, { activate = true } = {}) {
  const created = await commerce.products.create({
    name,
    variants: [{ sku, name, priceExact: price, isDefault: true }],
  });
  // Products are created in draft; only an active product's SKU is purchasable.
  const product = activate
    ? await commerce.products.update(created.id, { status: 'active' })
    : created;
  await commerce.inventory.createItem({ sku, name, initialQuantity: 100 });
  return product;
}

test('customers.list paginates and filters', async () => {
  const commerce = new Commerce(':memory:');
  const created = await seedCustomers(commerce, 3);

  assert.equal((await commerce.customers.list()).length, 3, 'no-arg call still lists all');
  assert.equal((await commerce.customers.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.customers.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.customers.list({ limit: 1, offset: 1 })).length, 1);
  assert.equal((await commerce.customers.list({ offset: 10 })).length, 0);

  const byEmail = await commerce.customers.list({ email: created[1].email });
  assert.deepEqual(
    byEmail.map((c) => c.id),
    [created[1].id],
  );

  await commerce.customers.update(created[0].id, { status: 'inactive' });
  const inactive = await commerce.customers.list({ status: 'inactive' });
  assert.deepEqual(
    inactive.map((c) => c.id),
    [created[0].id],
  );
  assert.equal((await commerce.customers.list({ status: 'active' })).length, 2);

  const bad = await rejects(() => commerce.customers.list({ status: 'sleeping' }));
  assert.equal(bad.code, 'VALIDATION');
  assert.match(bad.message, /customer status/);
});

test('orders.list paginates and filters by customer and status', async () => {
  const commerce = new Commerce(':memory:');
  const [alice, bob] = await seedCustomers(commerce, 2, 'orders');
  await seedProductWithVariant(commerce, 'Widget', 'WID-1', '5.00');

  const orders = [];
  for (const customer of [alice, alice, bob]) {
    orders.push(
      await commerce.orders.create({
        customerId: customer.id,
        items: [{ sku: 'WID-1', name: 'Widget', quantity: 1, unitPriceExact: '5.00' }],
      }),
    );
  }

  assert.equal((await commerce.orders.list()).length, 3);
  assert.equal((await commerce.orders.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.orders.list({ offset: 2 })).length, 1);

  const forAlice = await commerce.orders.list({ customerId: alice.id });
  assert.equal(forAlice.length, 2);
  assert.ok(forAlice.every((o) => o.customerId === alice.id));

  await commerce.orders.updateStatus(orders[2].id, 'confirmed');
  const confirmed = await commerce.orders.list({ status: 'confirmed' });
  assert.deepEqual(
    confirmed.map((o) => o.id),
    [orders[2].id],
  );
  assert.equal((await commerce.orders.list({ status: 'pending' })).length, 2);

  const badId = await rejects(() => commerce.orders.list({ customerId: 'nope' }));
  assert.equal(badId.code, 'VALIDATION');
  assert.match(badId.message, /customer UUID/);

  const badDate = await rejects(() => commerce.orders.list({ fromDate: 'yesterday' }));
  assert.equal(badDate.code, 'VALIDATION');
  assert.match(badDate.message, /fromDate/);
});

test('products.list paginates and filters by status and search', async () => {
  const commerce = new Commerce(':memory:');
  const hat = await seedProductWithVariant(commerce, 'Alpha Hat', 'HAT-1', '10.00', { activate: false });
  await seedProductWithVariant(commerce, 'Beta Shoe', 'SHOE-1', '20.00', { activate: false });
  await seedProductWithVariant(commerce, 'Gamma Sock', 'SOCK-1', '3.00', { activate: false });

  assert.equal((await commerce.products.list()).length, 3);
  assert.equal((await commerce.products.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.products.list({ offset: 2 })).length, 1);

  await commerce.products.activate(hat.id);
  const active = await commerce.products.list({ status: 'active' });
  assert.deepEqual(
    active.map((p) => p.id),
    [hat.id],
  );

  const search = await commerce.products.list({ search: 'Beta' });
  assert.equal(search.length, 1);
  assert.equal(search[0].name, 'Beta Shoe');

  const badPrice = await rejects(() => commerce.products.list({ minPrice: 'cheap' }));
  assert.equal(badPrice.code, 'VALIDATION');
});

async function seedOrder(commerce, customer, sku, quantity = 1) {
  return commerce.orders.create({
    customerId: customer.id,
    items: [{ sku, name: 'Widget', quantity, unitPriceExact: '5.00' }],
  });
}

test('payments.list paginates and filters', async () => {
  const commerce = new Commerce(':memory:');
  const [customer] = await seedCustomers(commerce, 1, 'pay');
  await seedProductWithVariant(commerce, 'Widget', 'WID-2', '5.00');
  const order = await seedOrder(commerce, customer, 'WID-2', 3);

  // Three partial captures that together stay within the 15.00 order total.
  for (let i = 0; i < 3; i += 1) {
    await commerce.payments.create({ orderId: order.id, amountExact: '4.00' });
  }
  assert.equal((await commerce.payments.list()).length, 3);
  assert.equal((await commerce.payments.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.payments.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.payments.list({ orderId: order.id })).length, 3);
  assert.equal((await commerce.payments.list({ status: 'completed' })).length, 0);
  assert.equal((await commerce.payments.list({ status: 'pending' })).length, 3);
  assert.equal((await commerce.payments.list({ paymentMethod: 'credit_card' })).length, 3);
  const badMethod = await rejects(() => commerce.payments.list({ paymentMethod: 'iou' }));
  assert.equal(badMethod.code, 'VALIDATION');
  const badAmount = await rejects(() => commerce.payments.list({ minAmount: 'lots' }));
  assert.equal(badAmount.code, 'VALIDATION');
});

test('returns.list paginates and filters', async () => {
  const commerce = new Commerce(':memory:');
  const [customer] = await seedCustomers(commerce, 1, 'ret');
  await seedProductWithVariant(commerce, 'Widget', 'WID-4', '5.00');

  // A return requires a shipped order (the engine enforces this).
  const orders = [];
  for (let i = 0; i < 3; i += 1) {
    const order = await seedOrder(commerce, customer, 'WID-4');
    await commerce.orders.ship(order.id, `TRACK-${i}`);
    orders.push(await commerce.orders.get(order.id));
  }
  for (const [i, order] of orders.entries()) {
    await commerce.returns.create({
      orderId: order.id,
      reason: i === 0 ? 'damaged' : 'changed_mind',
      items: [{ orderItemId: order.items[0].id, quantity: 1 }],
    });
  }
  assert.equal((await commerce.returns.list()).length, 3);
  assert.equal((await commerce.returns.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.returns.list({ offset: 2 })).length, 1);
  const damaged = await commerce.returns.list({ reason: 'damaged' });
  assert.equal(damaged.length, 1);
  assert.equal(damaged[0].orderId, orders[0].id);
  assert.equal((await commerce.returns.list({ status: 'requested' })).length, 3);
  assert.equal((await commerce.returns.list({ orderId: orders[1].id })).length, 1);
  assert.equal((await commerce.returns.list({ customerId: customer.id })).length, 3);
  const badReason = await rejects(() => commerce.returns.list({ reason: 'because' }));
  assert.equal(badReason.code, 'VALIDATION');
});

test('shipments.list paginates and filters', async () => {
  const commerce = new Commerce(':memory:');
  const [customer] = await seedCustomers(commerce, 1, 'ship');
  await seedProductWithVariant(commerce, 'Widget', 'WID-5', '5.00');
  const order = await seedOrder(commerce, customer, 'WID-5', 3);

  for (const carrier of ['ups', 'ups', 'dhl']) {
    await commerce.shipments.create({
      orderId: order.id,
      recipientName: 'Page Number0',
      shippingAddress: '1 Main St',
      carrier,
    });
  }
  assert.equal((await commerce.shipments.list()).length, 3);
  assert.equal((await commerce.shipments.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.shipments.list({ offset: 2 })).length, 1);
  const ups = await commerce.shipments.list({ carrier: 'ups' });
  assert.equal(ups.length, 2);
  assert.ok(ups.every((s) => s.carrier === 'ups'));
  assert.equal((await commerce.shipments.list({ orderId: order.id })).length, 3);
  assert.equal((await commerce.shipments.list({ status: 'pending' })).length, 3);
  assert.equal((await commerce.shipments.list({ status: 'delivered' })).length, 0);
  const badCarrier = await rejects(() => commerce.shipments.list({ carrier: 'pigeon' }));
  assert.equal(badCarrier.code, 'VALIDATION');
});

test('warranties, invoices, bom, carts and suppliers paginate', async () => {
  const commerce = new Commerce(':memory:');
  const [customer, other] = await seedCustomers(commerce, 2, 'misc');

  for (const c of [customer, customer, other]) {
    await commerce.warranties.create({ customerId: c.id, warrantyType: 'extended' });
  }
  assert.equal((await commerce.warranties.list()).length, 3);
  assert.equal((await commerce.warranties.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.warranties.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.warranties.list({ customerId: customer.id })).length, 2);
  assert.equal((await commerce.warranties.list({ warrantyType: 'extended' })).length, 3);
  assert.equal((await commerce.warranties.list({ warrantyType: 'lifetime' })).length, 0);

  for (const c of [customer, customer, other]) {
    await commerce.invoices.create({
      customerId: c.id,
      items: [{ description: 'Service', quantity: 1, unitPriceExact: '12.50' }],
    });
  }
  assert.equal((await commerce.invoices.list()).length, 3);
  assert.equal((await commerce.invoices.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.invoices.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.invoices.list({ customerId: customer.id })).length, 2);
  assert.equal((await commerce.invoices.list({ status: 'draft' })).length, 3);
  assert.equal((await commerce.invoices.list({ status: 'paid' })).length, 0);
  const badStatus = await rejects(() => commerce.invoices.list({ status: 'unpaid' }));
  assert.equal(badStatus.code, 'VALIDATION');

  const product = await commerce.products.create({ name: 'Assembly' });
  for (let i = 0; i < 3; i += 1) {
    await commerce.bom.create({ name: `BOM ${i}`, productId: product.id });
  }
  assert.equal((await commerce.bom.list()).length, 3);
  assert.equal((await commerce.bom.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.bom.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.bom.list({ productId: product.id })).length, 3);
  assert.equal((await commerce.bom.list({ status: 'draft' })).length, 3);
  assert.equal((await commerce.bom.list({ status: 'active' })).length, 0);

  for (const c of [customer, customer, other]) {
    await commerce.carts.create({ customerId: c.id });
  }
  assert.equal((await commerce.carts.list()).length, 3);
  assert.equal((await commerce.carts.list({ limit: 2 })).length, 2);
  assert.equal((await commerce.carts.list({ offset: 2 })).length, 1);
  assert.equal((await commerce.carts.list({ customerId: customer.id })).length, 2);
  assert.equal((await commerce.carts.list({ status: 'active' })).length, 3);
  assert.equal((await commerce.carts.list({ status: 'completed' })).length, 0);
  const badCart = await rejects(() => commerce.carts.list({ status: 'full' }));
  assert.equal(badCart.code, 'VALIDATION');

  for (const name of ['Acme', 'Bolt Co', 'Crate Inc']) {
    await commerce.purchaseOrders.createSupplier({ name });
  }
  assert.equal((await commerce.purchaseOrders.listSuppliers()).length, 3);
  assert.equal((await commerce.purchaseOrders.listSuppliers({ limit: 2 })).length, 2);
  assert.equal((await commerce.purchaseOrders.listSuppliers({ offset: 2 })).length, 1);
  const bolt = await commerce.purchaseOrders.listSuppliers({ name: 'Bolt' });
  assert.equal(bolt.length, 1);
  assert.equal(bolt[0].name, 'Bolt Co');
});

test('enum-typed outputs render the documented wire strings', async () => {
  const commerce = new Commerce(':memory:');
  const [customer] = await seedCustomers(commerce, 1, 'wire');
  assert.equal(customer.status, 'active');

  const address = await commerce.customers.addAddress({
    customerId: customer.id,
    firstName: 'Wire',
    lastName: 'Format',
    line1: '1 Main St',
    city: 'Springfield',
    postalCode: '00000',
    country: 'US',
  });
  assert.equal(address.addressType, 'both');

  await seedProductWithVariant(commerce, 'Widget', 'WID-3', '5.00');
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'WID-3', name: 'Widget', quantity: 1, unitPriceExact: '5.00' }],
  });
  assert.equal(order.status, 'pending');
  assert.equal(order.paymentStatus, 'pending');
  assert.equal(order.fulfillmentStatus, 'unfulfilled');

  const shipment = await commerce.shipments.create({
    orderId: order.id,
    recipientName: 'Wire Format',
    shippingAddress: '1 Main St',
    carrier: 'fedex',
    shippingMethod: 'twoday',
  });
  assert.equal(shipment.carrier, 'fed_ex', 'the engine renders the underscored carrier');
  assert.equal(shipment.shippingMethod, 'two_day');
  assert.equal(shipment.status, 'pending');

  const settings = await commerce.currency.getSettings();
  assert.ok(['half_up', 'half_down', 'up', 'down', 'half_even'].includes(settings.roundingMode));
});
