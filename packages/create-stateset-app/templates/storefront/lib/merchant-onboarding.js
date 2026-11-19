import { digitalProducts } from './digital-access.js';
import { getStripeConfiguration } from './stripe-config.js';
import {
  startStripeCheckout,
  reconcileStripeCheckout,
  refundStripePayment,
  verifyStripeTestWallet,
} from './stripe-checkout.js';
import { randomUUID } from 'node:crypto';
import { decimalToUnits } from './money.js';
import { firstMerchantRecord, readMerchantRecord } from './merchant-records.js';
import { inspectStoreDatabase } from './store-database-check.js';
import { assessStoreConfiguration } from './store-readiness.js';

export function validateCatalog(rows) {
  if (!Array.isArray(rows) || !rows.length)
    throw new Error('Catalog must contain at least one product');
  const skus = new Set(),
    slugs = new Set();
  for (const row of rows) {
    if (
      !row ||
      typeof row.name !== 'string' ||
      !row.name.trim() ||
      !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(row.slug || '') ||
      typeof row.sku !== 'string' ||
      !/^[\w-]{1,100}$/.test(row.sku) ||
      typeof row.price !== 'string' ||
      decimalToUnits(row.price, 2) < 1n ||
      !Number.isSafeInteger(row.stock) ||
      row.stock < 0 ||
      skus.has(row.sku) ||
      slugs.has(row.slug)
    )
      throw new Error(
        'Catalog requires unique SKUs and slugs, a name, positive USD price string and initial stock',
      );
    skus.add(row.sku);
    slugs.add(row.slug);
  }
  return rows;
}

export async function importCatalog(commerce, rows) {
  validateCatalog(rows);
  let inserted = 0;
  for (const row of rows) {
    const existing = await commerce.products.getBySlug(row.slug);
    const variant = await commerce.products.getVariantBySku(row.sku);
    if (existing || variant) {
      if (
        !existing ||
        !variant ||
        variant.productId !== existing.id ||
        existing.name !== row.name ||
        decimalToUnits(variant.priceExact, 2) !== decimalToUnits(row.price, 2)
      ) {
        throw new Error('Existing catalog conflicts with ' + row.sku);
      }
    } else {
      await commerce.products.create({
        name: row.name,
        slug: row.slug,
        description: row.description,
        variants: [{ sku: row.sku, name: 'Default', priceExact: row.price, isDefault: true }],
      });
      inserted++;
    }
    const product = existing || (await commerce.products.getBySlug(row.slug));
    if (!(await commerce.inventory.getStock(row.sku))) {
      await commerce.inventory.createItem({
        sku: row.sku,
        name: row.name,
        initialQuantity: row.stock,
      });
    }
    if (product.status === 'draft') await commerce.products.activate(product.id);
  }
  return { inserted, total: rows.length };
}

export function localLaunchChecks(env) {
  const config = assessStoreConfiguration(env, { launch: true });
  return [...config.checks, inspectStoreDatabase(env, { launch: true })].map((check) => ({
    ...check,
    evidence: 'local',
  }));
}

// The operator CLI calls this only for its own paid Stripe test-mode session.
// Stable command envelopes allow another process to resume after any receipt.
export async function shipTestPurchase(commerce, orderId) {
  const order = await commerce.orders.get(orderId);
  if (!order?.shippingAddress) throw new Error('Test order requires a shipping address');
  const buyer = await commerce.customers.get(order.customerId);
  const address = order.shippingAddress;
  const tracking = 'TEST-' + orderId;
  const store = 'store:launch-drill';
  const tenant = 'tenant:launch-drill';
  async function apply(commandType, payload) {
    const key = 'drill-' + orderId + '-' + commandType.replaceAll('.', '-');
    const stamp = await firstMerchantRecord(commerce, key, {
      id: randomUUID(),
      at: new Date().toISOString(),
    });
    const version = 'launch-drill-v1';
    const receipt = await commerce.executeKernelCommand(
      {
        contract_version: '1.0',
        command_id: stamp.id,
        issued_at: stamp.at,
        mode: 'apply',
        idempotency_key: key,
        command_type: commandType,
        store_id: store,
        policy_version: version,
        principal: {
          id: 'operator:launch-drill',
          kind: 'human',
          tenant_id: tenant,
          capabilities: [commandType],
        },
        payload,
      },
      {
        version,
        trusted_authority_keys: {},
        commands: {
          [commandType]: {
            required_capabilities: [commandType],
            requires_approval: false,
            requires_tenant: true,
            requires_store: true,
            allowed_tenant_ids: [tenant],
            allowed_store_ids: [store],
            requires_agent_delegation: false,
            requires_signed_authority: false,
          },
        },
      },
    );
    if (receipt.status !== 'succeeded')
      throw new Error(
        commandType +
          ' failed: ' +
          JSON.stringify(receipt.error || receipt.reason || receipt.status),
      );
    return receipt.result;
  }
  await apply('orders.transition', {
    order_id: orderId,
    status: 'processing',
    payment_status: null,
  });
  const shipment = await apply('shipments.create', {
    order_id: orderId,
    carrier: 'other',
    shipping_method: null,
    tracking_number: tracking,
    recipient_name: [buyer.firstName, buyer.lastName].filter(Boolean).join(' '),
    shipping_address: [
      address.line1,
      address.line2,
      address.city,
      address.state,
      address.postalCode,
      address.country,
    ]
      .filter(Boolean)
      .join(', '),
    recipient_email: null,
    recipient_phone: null,
    weight_kg: null,
    dimensions: null,
    shipping_cost: null,
    insurance_amount: null,
    signature_required: null,
    estimated_delivery: null,
    notes: 'Merchant launch drill',
    items: null,
  });
  await apply('orders.ship', { order_id: orderId, tracking_number: tracking, lines: null });
  const recorded = await commerce.shipments.get(shipment.id);
  if (recorded?.status !== 'shipped' || recorded.trackingNumber !== tracking)
    throw new Error('Test shipment was not recorded');
  return { evidence: 'local', id: recorded.id, status: recorded.status, tracking };
}

export async function verifyStripeConnection(stripe, origin, recovery = false, env = process.env) {
  const account = await stripe.accounts.retrieve();
  const expected = new URL('/api/checkout/stripe-webhook', origin).href;
  const required = [
    'checkout.session.completed',
    'checkout.session.expired',
    'refund.created',
    'refund.updated',
    'refund.failed',
  ];
  if (recovery) required.push('invoice.payment_failed');
  if (recovery || Object.values(digitalProducts(env)).some((p) => p.recurring))
    required.push(
      'invoice.paid',
      'customer.subscription.updated',
      'customer.subscription.deleted',
      'customer.subscription.paused',
      'customer.subscription.resumed',
    );
  let subscribed = false;
  for await (const endpoint of stripe.webhookEndpoints.list({ limit: 100 })) {
    if (
      endpoint.url === expected &&
      endpoint.status === 'enabled' &&
      (endpoint.enabled_events.includes('*') ||
        required.every((event) => endpoint.enabled_events.includes(event)))
    )
      subscribed = true;
  }
  return {
    evidence: 'provider',
    account: account.id,
    chargesEnabled: account.charges_enabled,
    checkoutWebhookRegistered: subscribed,
    settlementVerified: false,
  };
}

export async function runLaunchDrill(commerce, stripe, env, values) {
  const config = getStripeConfiguration(env);
  if (!config.key.startsWith('sk_test_'))
    throw new Error('The onboarding drill requires a Stripe test-mode key');
  if (!/^[a-zA-Z0-9_-]{1,48}$/.test(values.key || '')) throw new Error('Supply a stable drill key');
  if (Boolean(values.purchase) === Boolean(values.finish))
    throw new Error('Choose a test purchase or a paid test session');
  if (values.wallet && !['apple_pay', 'google_pay'].includes(values.wallet))
    throw new Error('Choose apple_pay or google_pay for a wallet drill');
  const report = {};
  const key = 'test-purchase-' + values.key;
  if (values.purchase) {
    if (!values.address || !values.email || !values.shipping)
      throw new Error('Supply --address JSON_FILE --email BUYER_EMAIL and --shipping METHOD');
    const address = values.address;
    const variant = await commerce.products.getVariantBySku(values.purchase);
    if (!variant) throw new Error('Test SKU is missing');
    report.testPurchase = { sku: values.purchase, evidence: 'local', status: 'preview' };
    if (values.apply) {
      let entry = await readMerchantRecord(commerce, key);
      if (!entry) {
        const cart = await commerce.carts.create({ currency: 'USD' });
        try {
          await commerce.carts.addItemExact(cart.id, {
            productId: variant.productId,
            variantId: variant.id,
            sku: variant.sku,
            name: variant.name || variant.sku,
            quantity: 1,
            unitPrice: variant.priceExact,
          });
          entry = await firstMerchantRecord(commerce, key, {
            sku: values.purchase,
            cartId: cart.id,
            ...(values.wallet && { wallet: values.wallet }),
          });
        } finally {
          if (entry?.cartId !== cart.id) await commerce.carts.cancel(cart.id);
        }
      }
      if (entry.sku !== values.purchase) throw new Error('Drill key already used with another SKU');
      if (values.wallet && values.wallet !== entry.wallet)
        throw new Error('Start a new drill key to test a different wallet');
      const session = await startStripeCheckout({
        commerce,
        stripe,
        origin: config.origin,
        env,
        input: {
          cartId: entry.cartId,
          email: values.email,
          shippingAddress: address,
          shippingMethodId: values.shipping,
        },
      });
      report.testPurchase = {
        evidence: 'provider',
        status: 'awaiting-buyer',
        url: session.url,
        sessionId: session.sessionId,
        ...(entry.wallet && { expectedWallet: entry.wallet }),
      };
    }
  }
  if (values.finish) {
    const entry = await readMerchantRecord(commerce, key);
    if (!entry) throw new Error('Session does not belong to this test purchase');
    if (values.wallet && values.wallet !== entry.wallet)
      throw new Error('Start a new drill key to test a different wallet');
    const session = await stripe.checkout.sessions.retrieve(values.finish, {
      expand: ['payment_intent'],
    });
    if (
      session.id !== values.finish ||
      entry.cartId !== session.metadata?.cart_id ||
      session.livemode !== false
    )
      throw new Error('Session does not belong to this test purchase');
    report.payment = { evidence: 'provider', status: session.payment_status };
    if (session.payment_status !== 'paid')
      throw new Error('Complete the hosted test checkout before finishing the drill');
    if (entry.wallet)
      report.wallet = await verifyStripeTestWallet(commerce, stripe, session, entry.wallet);
    if (values.apply) {
      const payment = await reconcileStripeCheckout(commerce, stripe, session.id);
      if (payment.state !== 'paid') throw new Error('Payment has not settled');
      if (report.wallet) {
        const native = await commerce.payments.get(payment.paymentId);
        if (
          native?.idempotencyKey !== 'stripe:' + report.wallet.paymentIntentId ||
          native.orderId !== payment.orderId
        )
          throw new Error('Wallet charge does not match the native payment');
      }
      const amount = values.refund || '0.01';
      if (
        decimalToUnits(amount, 2) <= 0n ||
        decimalToUnits(amount, 2) >= BigInt(session.amount_total)
      )
        throw new Error('Choose a partial refund smaller than the charge');
      report.shipment = await shipTestPurchase(commerce, payment.orderId);
      const refund = await refundStripePayment(commerce, stripe, {
        paymentId: payment.paymentId,
        amount,
        reason: 'Merchant onboarding test',
        idempotencyKey: key + '-refund',
      });
      report.refund = {
        evidence: 'provider-and-native-ledger',
        status: refund.status,
        amount: refund.amountExact,
      };
      report.settlementVerified = refund.status === 'completed';
      if (report.settlementVerified)
        await firstMerchantRecord(
          commerce,
          key + '-complete',
          {
            drill: values.key,
            orderId: payment.orderId,
            shipmentId: report.shipment.id,
            refundId: refund.id,
            ...(report.wallet && { wallet: report.wallet }),
          },
          'launch-drills',
        );
    }
  }
  return report;
}
