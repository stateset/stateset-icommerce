/**
 * The governed-write ("kernel") surface is typed, and the types tell the truth.
 *
 * Two halves, deliberately separate:
 *
 * 1. `declarations` reads `index.d.ts` and asserts the four kernel methods
 *    carry the interfaces from `scripts/types/kernel.d.ts` rather than `any`.
 *    It reflects the LAST BUILD: after editing `src/lib.rs` or the fragment it
 *    fails until `npm run build:debug` (which runs `scripts/postbuild.mjs`)
 *    regenerates the file.
 *
 * 2. `behaviour` runs real commands through the CURRENT native binary and
 *    checks that every field the declared interfaces promise is actually on
 *    the wire with the declared JavaScript type. It does not need a rebuild.
 */

const assert = require('assert');
const { test } = require('node:test');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');

const { Commerce } = require('../index.js');

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
const DECIMAL = /^-?\d+(\.\d+)?$/;

/** Assert `value[key]` has JavaScript type `type`, or is `null` when allowed. */
function field(value, key, type, { nullable = false } = {}) {
  assert.ok(Object.hasOwn(value, key), `receipt is missing declared field "${key}"`);
  const actual = value[key];
  if (actual === null) {
    assert.ok(nullable, `"${key}" is null but the interface declares it non-nullable`);
    return;
  }
  const kind = Array.isArray(actual) ? 'array' : typeof actual;
  assert.strictEqual(kind, type, `"${key}" should be ${type}, got ${kind}`);
}

function moneyWire(value, label) {
  assert.strictEqual(typeof value, 'object', `${label} should be a MoneyWire`);
  assert.match(value.amount, DECIMAL, `${label}.amount must be an exact decimal string`);
  assert.match(value.currency, /^[A-Z]{3}$/, `${label}.currency must be ISO 4217`);
}

/** The storefront journey commands: account, cart, capture, fulfilment and return. */
const STOREFRONT_COMMANDS = [
  'customers.create',
  'carts.create',
  'carts.item.add',
  'carts.shipping_address.set',
  'carts.payment_method.set',
  'carts.coupon.apply',
  'carts.tax.calculate',
  'payments.complete',
  'shipments.create',
  'returns.create',
  'returns.tracking.add',
];

/** Command types matched in `Commerce::execute_kernel_command` (stateset-embedded). */
function dispatchedCommandTypes() {
  const source = readFileSync(
    path.join(__dirname, '..', '..', '..', 'crates', 'stateset-embedded', 'src', 'commerce', 'kernel.rs'),
    'utf8',
  );
  const start = source.indexOf('pub fn execute_kernel_command(');
  assert.ok(start !== -1, 'stateset-embedded no longer defines execute_kernel_command');
  const end = source.indexOf('unsupported governed kernel command type', start);
  assert.ok(end !== -1, 'execute_kernel_command no longer refuses unknown command types');
  const types = [...source.slice(start, end).matchAll(/^\s*"([a-z0-9_.]+)" => /gm)].map((m) => m[1]);
  assert.ok(types.length > 0, 'no command types found in execute_kernel_command');
  return types;
}

// ---------------------------------------------------------------------------
// 1. Declarations — reflects the last build; passes after `npm run build:debug`.
// ---------------------------------------------------------------------------

test('declarations: index.d.ts types the kernel surface (needs a rebuild after editing lib.rs)', () => {
  const dts = readFileSync(path.join(__dirname, '..', 'index.d.ts'), 'utf8');

  const signatures = [
    /^\s*checkoutSnapshot\(cartId: string\): Promise<CheckoutSnapshot>$/m,
    /^\s*executeKernelCommand\(command: KernelCommand, policy: KernelPolicy\): Promise<KernelReceipt>$/m,
    /^\s*provisionEconomicBudget\(budget: EconomicBudget\): Promise<EconomicBudgetStatus>$/m,
    /^\s*economicBudgetStatus\(budgetId: string\): Promise<EconomicBudgetStatus \| null>$/m,
  ];
  for (const signature of signatures) {
    assert.match(dts, signature, `index.d.ts lacks ${signature}`);
  }
  for (const stale of [
    /checkoutSnapshot\(cartId: string\): Promise<any>/,
    /executeKernelCommand\(command: any, policy: any\)/,
    /provisionEconomicBudget\(budget: any\)/,
    /economicBudgetStatus\(budgetId: string\): Promise<any>/,
  ]) {
    assert.doesNotMatch(dts, stale, `index.d.ts still carries the untyped signature ${stale}`);
  }

  // The fragment must have been appended by scripts/postbuild-types.mjs.
  assert.ok(
    dts.includes('// ---- fragment: scripts/types/kernel.d.ts ----'),
    'the kernel.d.ts fragment was not appended to index.d.ts',
  );
  for (const name of [
    'export interface KernelPolicy ',
    'export interface KernelCommandPolicy ',
    'export interface KernelPrincipal ',
    'export type KernelCommand =',
    'export type KernelCommandType =',
    'export interface KernelReceipt {',
    'export interface EconomicBudget ',
    'export interface EconomicBudgetStatus ',
    'export interface CheckoutSnapshot ',
    'export interface CartSnapshot ',
    'export interface MoneyWire ',
  ]) {
    assert.ok(dts.includes(name), `index.d.ts lacks "${name}"`);
  }
  // The union is exactly the catalog the engine dispatches: read from the
  // match in `Commerce::execute_kernel_command`, so a command added there
  // fails here until it is declared.
  const dispatched = dispatchedCommandTypes();
  for (const commandType of [
    'checkout.commit',
    'orders.ship_reserved',
    'returns.item.disposition.set',
    ...STOREFRONT_COMMANDS,
  ]) {
    assert.ok(dispatched.includes(commandType), `dispatch parse missed '${commandType}'`);
  }
  const declared = [...dts.matchAll(/KernelCommandEnvelope<'([a-z0-9_.]+)',/g)].map((m) => m[1]);
  assert.deepStrictEqual(
    [...new Set(declared)].sort(),
    [...dispatched].sort(),
    'the KernelCommand union and the engine dispatch disagree',
  );
  assert.strictEqual(declared.length, new Set(declared).size, 'a command is declared twice');

  // Each storefront command carries its own payload interface, not `any`.
  for (const [commandType, payload] of [
    ['customers.create', 'KernelCreateCustomerPayload'],
    ['carts.create', 'KernelCreateCartPayload'],
    ['carts.item.add', 'KernelAddCartItemPayload'],
    ['carts.shipping_address.set', 'KernelSetCartShippingAddressPayload'],
    ['carts.payment_method.set', 'KernelSetCartPaymentMethodPayload'],
    ['carts.coupon.apply', 'KernelApplyCartCouponPayload'],
    ['carts.tax.calculate', 'KernelCalculateCartTaxPayload'],
    ['payments.complete', 'KernelCompletePaymentPayload'],
    ['shipments.create', 'KernelCreateShipmentPayload'],
    ['returns.create', 'KernelCreateReturnPayload'],
    ['returns.tracking.add', 'KernelAddReturnTrackingPayload'],
    ['orders.ship_reserved', 'KernelShipOrderPayload'],
  ]) {
    assert.ok(
      dts.includes(`KernelCommandEnvelope<'${commandType}', ${payload}>`),
      `'${commandType}' is not typed with ${payload}`,
    );
    assert.ok(dts.includes(`export interface ${payload} {`), `index.d.ts lacks ${payload}`);
  }
});

// ---------------------------------------------------------------------------
// 2. Behaviour — runs against the current binary; passes now.
// ---------------------------------------------------------------------------

const TENANT = 'tenant:test';
const STORE = 'store:test';

/** The policy shape the CLI's kernel fixtures use (cli/test/integration/kernel-a2a-shared-store.test.js). */
function policyAllowing(...commandTypes) {
  return {
    version: 'kernel-types-v1',
    commands: Object.fromEntries(
      commandTypes.map((commandType) => [
        commandType,
        {
          required_capabilities: [commandType],
          requires_approval: false,
          requires_tenant: true,
          requires_store: true,
          allowed_tenant_ids: [TENANT],
          allowed_store_ids: [STORE],
          requires_agent_delegation: true,
          requires_signed_authority: false,
        },
      ]),
    ),
    trusted_authority_keys: {},
  };
}

/** The envelope shape cli/src/kernel-tool-execution.js builds. */
function envelope(commandType, payload, { mode = 'preview', idempotencyKey } = {}) {
  return {
    contract_version: '1.0',
    command_id: randomUUID(),
    idempotency_key: idempotencyKey ?? `kernel-types-${randomUUID()}`,
    command_type: commandType,
    principal: {
      id: 'agent:kernel-types',
      kind: 'agent',
      tenant_id: TENANT,
      delegated_by: 'user:kernel-types',
      capabilities: [commandType],
    },
    store_id: STORE,
    correlation_id: null,
    causation_id: null,
    expected_version: null,
    policy_version: 'kernel-types-v1',
    approval: null,
    authority: null,
    mandate: null,
    commitment: null,
    deadline: null,
    trace_id: null,
    mode,
    payload,
    issued_at: new Date().toISOString(),
  };
}

/** Every field `KernelReceipt` declares, with its declared type. */
function assertReceiptShape(receipt, { commandType, status }) {
  field(receipt, 'contract_version', 'string');
  assert.strictEqual(receipt.contract_version, '1.0');
  field(receipt, 'receipt_id', 'string');
  assert.match(receipt.receipt_id, UUID);
  field(receipt, 'command_id', 'string');
  assert.match(receipt.command_id, UUID);
  field(receipt, 'idempotency_key', 'string');
  field(receipt, 'command_type', 'string');
  assert.strictEqual(receipt.command_type, commandType);
  field(receipt, 'status', 'string');
  assert.strictEqual(receipt.status, status);
  field(receipt, 'result', 'object', { nullable: true });
  field(receipt, 'error_code', 'string', { nullable: true });
  field(receipt, 'error_message', 'string', { nullable: true });
  field(receipt, 'retry', 'string');
  assert.ok(
    ['never', 'same_key', 'after_conflict', 'after_delay'].includes(receipt.retry),
    `retry disposition ${receipt.retry} is not in the declared union`,
  );
  field(receipt, 'aggregate_type', 'string', { nullable: true });
  field(receipt, 'aggregate_id', 'string', { nullable: true });
  field(receipt, 'version_before', 'number', { nullable: true });
  field(receipt, 'version_after', 'number', { nullable: true });
  field(receipt, 'event_ids', 'array');
  for (const id of receipt.event_ids) assert.match(id, UUID);
  field(receipt, 'policy', 'object', { nullable: true });
  if (receipt.policy) {
    field(receipt.policy, 'policy_version', 'string');
    field(receipt.policy, 'decision_id', 'string');
    field(receipt.policy, 'allowed', 'boolean');
    field(receipt.policy, 'reason_codes', 'array');
  }
  field(receipt, 'economic_context', 'object');
  const context = receipt.economic_context;
  field(context, 'principal', 'object');
  field(context.principal, 'id', 'string');
  field(context.principal, 'kind', 'string');
  field(context.principal, 'tenant_id', 'string', { nullable: true });
  field(context.principal, 'delegated_by', 'string', { nullable: true });
  field(context.principal, 'capabilities', 'array');
  field(context, 'store_id', 'string', { nullable: true });
  field(context, 'correlation_id', 'string', { nullable: true });
  field(context, 'mandate', 'object', { nullable: true });
  field(context, 'commitment', 'object', { nullable: true });
  field(context, 'approval_id', 'string', { nullable: true });
  field(context, 'authority_issuer', 'string', { nullable: true });
  field(receipt, 'audit_hash', 'string', { nullable: true });
  assert.match(receipt.audit_hash, /^[0-9a-f]{64}$/, 'sealed receipts carry a hex SHA-256 link');
  field(receipt, 'started_at', 'string');
  assert.match(receipt.started_at, RFC3339);
  field(receipt, 'completed_at', 'string');
  assert.match(receipt.completed_at, RFC3339);
}

test('behaviour: the wire carries every field the kernel interfaces declare', async (t) => {
  const commerce = new Commerce(':memory:');
  const policy = policyAllowing('payments.create', 'checkout.commit');

  await t.test('payments.create preview receipt matches KernelReceipt', async () => {
    const receipt = await commerce.executeKernelCommand(
      envelope('payments.create', {
        amount: '12.34',
        currency: 'USD',
        payment_method: 'credit_card',
      }),
      policy,
    );
    assertReceiptShape(receipt, { commandType: 'payments.create', status: 'previewed' });
    assert.strictEqual(receipt.policy.allowed, true);
    assert.deepStrictEqual(receipt.policy.reason_codes, []);
    assert.strictEqual(receipt.policy.policy_version, 'kernel-types-v1');
    assert.strictEqual(receipt.result, null, 'a preview creates no aggregate');
    assert.strictEqual(receipt.aggregate_type, 'payment');
    assert.strictEqual(commerce.payments && (await commerce.payments.count()), 0);
  });

  await t.test('payments.create apply receipt carries the snake_case result', async () => {
    const receipt = await commerce.executeKernelCommand(
      envelope(
        'payments.create',
        { amount: '12.34', currency: 'USD', payment_method: 'credit_card' },
        { mode: 'apply' },
      ),
      policy,
    );
    assertReceiptShape(receipt, { commandType: 'payments.create', status: 'succeeded' });
    assert.strictEqual(receipt.aggregate_type, 'payment');
    assert.ok(receipt.result, 'an applied command returns its aggregate');
    assert.match(receipt.result.id, UUID);
    assert.strictEqual(receipt.result.amount, '12.34', 'money in the result is an exact decimal string');
    assert.strictEqual(receipt.result.currency, 'USD');
    assert.strictEqual(receipt.result.payment_method, 'credit_card');
    assert.ok(receipt.event_ids.length >= 1, 'apply commits at least one event');
    assert.strictEqual(receipt.error_code, null);
  });

  await t.test('a policy rejection is a receipt, not an exception', async () => {
    const receipt = await commerce.executeKernelCommand(
      envelope('payments.create', { amount: '1.00', payment_method: 'credit_card' }),
      { version: 'deny-all', commands: {}, trusted_authority_keys: {} },
    );
    assertReceiptShape(receipt, { commandType: 'payments.create', status: 'rejected' });
    assert.strictEqual(receipt.policy.allowed, false);
    assert.ok(receipt.policy.reason_codes.includes('policy.command_not_allowed'));
    assert.ok(receipt.policy.reason_codes.includes('policy.version_conflict'));
    assert.strictEqual(typeof receipt.error_code, 'string');
    assert.strictEqual(receipt.retry, 'never');
  });

  await t.test('an unknown command_type is refused before dispatch', async () => {
    await assert.rejects(
      commerce.executeKernelCommand(envelope('customers.delete_all', {}), policy),
      (error) => {
        assert.match(error.message, /unsupported governed kernel command type/);
        return true;
      },
    );
  });

  await t.test('an invalid policy is a VALIDATION error', async () => {
    await assert.rejects(
      commerce.executeKernelCommand(envelope('payments.create', {}), { commands: {} }),
      (error) => {
        assert.strictEqual(error.code, 'VALIDATION');
        assert.match(error.message, /Invalid kernel policy/);
        return true;
      },
    );
  });

  await t.test('checkoutSnapshot matches CheckoutSnapshot and feeds checkout.commit', async () => {
    const cart = await commerce.carts.create({
      customerEmail: 'quote@example.com',
      currency: 'USD',
    });
    await commerce.carts.addItem(cart.id, {
      sku: 'KT-1',
      name: 'Typed thing',
      quantity: 2,
      unitPriceExact: '4.50',
    });

    const snapshot = await commerce.checkoutSnapshot(cart.id);
    field(snapshot, 'fingerprint', 'string');
    assert.match(snapshot.fingerprint, /^sha256:[0-9a-f]{64}$/);
    field(snapshot, 'cart', 'object');
    const c = snapshot.cart;
    // Declared as snake_case engine model with exact decimals — not CartOutput.
    assert.strictEqual(c.id, cart.id);
    field(c, 'cart_number', 'string');
    field(c, 'customer_id', 'string', { nullable: true });
    field(c, 'status', 'string');
    assert.strictEqual(c.status, 'active');
    field(c, 'currency', 'string');
    field(c, 'items', 'array');
    for (const key of ['subtotal', 'tax_amount', 'shipping_amount', 'discount_amount', 'grand_total']) {
      field(c, key, 'string');
      assert.match(c[key], DECIMAL, `${key} must be an exact decimal string`);
    }
    assert.strictEqual(c.subtotal, '9.00');
    assert.strictEqual(c.grand_total, '9.00');
    field(c, 'customer_email', 'string', { nullable: true });
    field(c, 'customer_phone', 'string', { nullable: true });
    field(c, 'customer_name', 'string', { nullable: true });
    field(c, 'shipping_address', 'object', { nullable: true });
    field(c, 'billing_address', 'object', { nullable: true });
    field(c, 'billing_same_as_shipping', 'boolean');
    field(c, 'fulfillment_type', 'string', { nullable: true });
    field(c, 'shipping_method', 'string', { nullable: true });
    field(c, 'shipping_carrier', 'string', { nullable: true });
    field(c, 'estimated_delivery', 'string', { nullable: true });
    field(c, 'payment_method', 'string', { nullable: true });
    field(c, 'payment_token', 'string', { nullable: true });
    field(c, 'payment_status', 'string');
    assert.strictEqual(c.payment_status, 'none');
    field(c, 'coupon_code', 'string', { nullable: true });
    field(c, 'discount_description', 'string', { nullable: true });
    field(c, 'order_id', 'string', { nullable: true });
    field(c, 'order_number', 'string', { nullable: true });
    field(c, 'notes', 'string', { nullable: true });
    field(c, 'metadata', 'object', { nullable: true });
    field(c, 'inventory_reserved', 'boolean');
    field(c, 'reservation_expires_at', 'string', { nullable: true });
    field(c, 'x402_payment', 'object', { nullable: true });
    field(c, 'expires_at', 'string', { nullable: true });
    field(c, 'completed_at', 'string', { nullable: true });
    field(c, 'created_at', 'string');
    assert.match(c.created_at, RFC3339);
    field(c, 'updated_at', 'string');
    assert.ok(!('cartNumber' in c) && !('grandTotal' in c), 'the snapshot is not camelCased');

    assert.strictEqual(c.items.length, 1);
    const item = c.items[0];
    field(item, 'id', 'string');
    field(item, 'cart_id', 'string');
    assert.strictEqual(item.cart_id, cart.id);
    field(item, 'product_id', 'string', { nullable: true });
    field(item, 'variant_id', 'string', { nullable: true });
    field(item, 'sku', 'string');
    field(item, 'name', 'string');
    field(item, 'description', 'string', { nullable: true });
    field(item, 'image_url', 'string', { nullable: true });
    field(item, 'quantity', 'number');
    assert.strictEqual(item.quantity, 2);
    field(item, 'unit_price', 'string');
    assert.strictEqual(item.unit_price, '4.50');
    field(item, 'original_price', 'string', { nullable: true });
    field(item, 'discount_amount', 'string');
    field(item, 'tax_amount', 'string');
    field(item, 'total', 'string');
    assert.strictEqual(item.total, '9.00');
    field(item, 'weight', 'string', { nullable: true });
    field(item, 'requires_shipping', 'boolean');
    field(item, 'metadata', 'object', { nullable: true });
    field(item, 'created_at', 'string');
    field(item, 'updated_at', 'string');

    // The fingerprint is the quote commitment `checkout.commit` verifies.
    const stale = await commerce.executeKernelCommand(
      envelope('checkout.commit', {
        cart_id: cart.id,
        expected_cart_fingerprint: 'sha256:' + '0'.repeat(64),
      }),
      policy,
    );
    assertReceiptShape(stale, { commandType: 'checkout.commit', status: 'rejected' });
    assert.match(stale.error_message, /fingerprint/i);

    const fresh = await commerce.executeKernelCommand(
      envelope('checkout.commit', {
        cart_id: cart.id,
        expected_cart_fingerprint: snapshot.fingerprint,
      }),
      policy,
    );
    assert.notStrictEqual(
      fresh.error_code,
      stale.error_code,
      'the real fingerprint must get past the quote check',
    );
  });

  await t.test('provisionEconomicBudget and economicBudgetStatus match EconomicBudgetStatus', async () => {
    const budget = {
      budget_id: 'budget:kernel-types',
      principal_id: 'agent:kernel-types',
      tenant_id: TENANT,
      store_id: STORE,
      limit: { amount: '500.00', currency: 'USD' },
      valid_from: '2026-01-01T00:00:00Z',
      expires_at: '2027-01-01T00:00:00Z',
    };
    const provisioned = await commerce.provisionEconomicBudget(budget);
    for (const status of [provisioned, await commerce.economicBudgetStatus(budget.budget_id)]) {
      field(status, 'budget', 'object');
      field(status, 'committed', 'object');
      field(status, 'available', 'object');
      moneyWire(status.committed, 'committed');
      moneyWire(status.available, 'available');
      assert.strictEqual(status.available.amount, '500.00');
      assert.strictEqual(status.committed.currency, 'USD');
      const b = status.budget;
      field(b, 'budget_id', 'string');
      field(b, 'principal_id', 'string');
      field(b, 'tenant_id', 'string', { nullable: true });
      field(b, 'store_id', 'string', { nullable: true });
      field(b, 'limit', 'object');
      moneyWire(b.limit, 'budget.limit');
      field(b, 'valid_from', 'string');
      assert.match(b.valid_from, RFC3339);
      field(b, 'expires_at', 'string');
      assert.match(b.expires_at, RFC3339);
      assert.deepStrictEqual(b.limit, budget.limit);
    }

    // Re-provisioning the identical definition is idempotent.
    await commerce.provisionEconomicBudget(budget);

    // Optional scope comes back as null, as declared.
    const unscoped = await commerce.provisionEconomicBudget({
      budget_id: 'budget:unscoped',
      principal_id: 'agent:kernel-types',
      limit: { amount: '1', currency: 'EUR' },
      valid_from: '2026-01-01T00:00:00Z',
      expires_at: '2027-01-01T00:00:00Z',
    });
    assert.strictEqual(unscoped.budget.tenant_id, null);
    assert.strictEqual(unscoped.budget.store_id, null);

    assert.strictEqual(
      await commerce.economicBudgetStatus('budget:missing'),
      null,
      'a missing budget resolves to null, as the declared return type says',
    );

    await assert.rejects(
      commerce.provisionEconomicBudget({ budget_id: 'x' }),
      (error) => {
        assert.strictEqual(error.code, 'VALIDATION');
        assert.match(error.message, /Invalid economic budget/);
        return true;
      },
    );
  });
});

test('behaviour: every storefront command accepts its declared payload', async () => {
  const commerce = new Commerce(':memory:');
  const policy = policyAllowing(
    ...STOREFRONT_COMMANDS,
    'checkout.commit',
    'payments.create',
    'orders.transition',
    'orders.ship',
    'returns.transition',
  );
  const receipts = new Map();
  /** Apply one command; a payload off the declared shape throws before a receipt exists. */
  async function apply(commandType, payload, status = 'succeeded') {
    const receipt = await commerce.executeKernelCommand(
      envelope(commandType, payload, { mode: 'apply' }),
      policy,
    );
    assertReceiptShape(receipt, { commandType, status });
    receipts.set(commandType, receipt);
    return receipt;
  }
  /** `KernelCartAddressInput`, required fields plus one optional. */
  const address = {
    first_name: 'Ada',
    last_name: 'Lovelace',
    line1: '1 Main St',
    city: 'Los Angeles',
    state: 'CA',
    postal_code: '90001',
    country: 'US',
  };

  // KernelCreateCustomerPayload
  const customer = (
    await apply('customers.create', {
      email: 'storefront-types@example.com',
      first_name: 'Ada',
      last_name: 'Lovelace',
      accepts_marketing: false,
    })
  ).result;
  // KernelCreateCartPayload
  const cart = (await apply('carts.create', { customer_id: customer.id, currency: 'USD' })).result;
  // KernelAddCartItemPayload, money as an exact decimal string
  await apply('carts.item.add', {
    cart_id: cart.id,
    item: { sku: 'KT-SF-1', name: 'Typed widget', quantity: 2, unit_price: '50.00' },
  });
  // KernelSetCartShippingAddressPayload
  await apply('carts.shipping_address.set', { cart_id: cart.id, address });
  // KernelSetCartPaymentMethodPayload
  await apply('carts.payment_method.set', {
    cart_id: cart.id,
    payment: { payment_method: 'credit_card', payment_token: 'tok_kernel_types' },
  });
  // KernelCalculateCartTaxPayload
  await apply('carts.tax.calculate', { cart_id: cart.id });
  // KernelApplyCartCouponPayload: an unknown code is a sealed business
  // rejection, so the payload decoded.
  const coupon = await apply(
    'carts.coupon.apply',
    { cart_id: cart.id, coupon_code: 'NO-SUCH-CODE' },
    'rejected',
  );
  assert.match(coupon.error_code, /^commerce\./);

  const checkout = (await apply('checkout.commit', { cart_id: cart.id })).result;
  const payment = (
    await apply('payments.create', {
      order_id: checkout.order_id,
      amount: checkout.total_charged,
      currency: 'USD',
      payment_method: 'credit_card',
    })
  ).result;
  // KernelCompletePaymentPayload
  assert.strictEqual(
    (await apply('payments.complete', { payment_id: payment.id })).result.status,
    'completed',
  );
  await apply('orders.transition', { order_id: checkout.order_id, status: 'processing' });

  const order = await commerce.orders.get(checkout.order_id);
  const orderItemId = order.items[0].id;
  // KernelCreateShipmentPayload, with the serde carrier and method spellings
  const shipment = (
    await apply('shipments.create', {
      order_id: checkout.order_id,
      carrier: 'fed_ex',
      shipping_method: 'two_day',
      recipient_name: 'Ada Lovelace',
      shipping_address: '1 Main St, Los Angeles, CA 90001, US',
      shipping_cost: '7.50',
      items: [{ order_item_id: orderItemId, sku: 'KT-SF-1', name: 'Typed widget', quantity: 2 }],
    })
  ).result;
  assert.strictEqual(shipment.carrier, 'fed_ex');
  await apply('orders.ship', { order_id: checkout.order_id, tracking_number: 'KT-TRACK-1' });

  // KernelCreateReturnPayload
  const ret = (
    await apply('returns.create', {
      order_id: checkout.order_id,
      reason: 'defective',
      items: [{ order_item_id: orderItemId, quantity: 1, condition: 'opened' }],
    })
  ).result;
  await apply('returns.transition', { return_id: ret.id, status: 'approved' });
  // KernelAddReturnTrackingPayload
  assert.strictEqual(
    (await apply('returns.tracking.add', { return_id: ret.id, tracking_number: 'KT-RET-1' }))
      .result.status,
    'in_transit',
  );

  for (const commandType of STOREFRONT_COMMANDS) {
    assert.ok(receipts.has(commandType), `${commandType} was not exercised`);
  }

  // A payload off the declared shape never reaches a receipt.
  await assert.rejects(
    commerce.executeKernelCommand(
      envelope('returns.tracking.add', { return_id: ret.id }, { mode: 'apply' }),
      policy,
    ),
    /tracking_number/,
  );
});
