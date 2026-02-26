/**
 * Tests for Zod schema validation constraints across tool files.
 *
 * Verifies that .int(), .positive(), .min(), .max(), .email(), .url()
 * constraints are properly enforced on tool input schemas.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

// Helper: extract inputSchema from a named tool in a module
function getSchema(tools, name) {
  const tool = tools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found`);
  return z.object(tool.inputSchema);
}

// Helper: assert parse failure
function expectFail(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(!result.success, msg || `Expected parse to fail for: ${JSON.stringify(data)}`);
}

// Helper: assert parse success
function expectPass(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(result.success, msg || `Expected parse to pass for: ${JSON.stringify(data)}, errors: ${JSON.stringify(result.error?.issues)}`);
}

describe('Zod validation constraints', () => {
  describe('orders', () => {
    let tools;
    it('loads order tools', async () => {
      const mod = await import('../../src/tools/orders.js');
      tools = mod.orderTools;
    });

    it('list_orders rejects non-integer limit', async () => {
      const schema = getSchema(tools, 'list_orders');
      expectFail(schema, { limit: 1.5 });
    });

    it('list_orders rejects limit < 1', async () => {
      const schema = getSchema(tools, 'list_orders');
      expectFail(schema, { limit: 0 });
    });

    it('list_orders rejects limit > 500', async () => {
      const schema = getSchema(tools, 'list_orders');
      expectFail(schema, { limit: 501 });
    });

    it('create_order rejects non-integer quantity', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, {
        customerId: 'abc',
        items: [{ sku: 'X', name: 'Y', quantity: 1.5, unitPrice: 10 }],
      });
    });

    it('create_order rejects zero quantity', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, {
        customerId: 'abc',
        items: [{ sku: 'X', name: 'Y', quantity: 0, unitPrice: 10 }],
      });
    });

    it('create_order rejects negative unitPrice', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, {
        customerId: 'abc',
        items: [{ sku: 'X', name: 'Y', quantity: 1, unitPrice: -5 }],
      });
    });

    it('create_order rejects zero unitPrice', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, {
        customerId: 'abc',
        items: [{ sku: 'X', name: 'Y', quantity: 1, unitPrice: 0 }],
      });
    });

    it('create_order rejects empty items array', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, { customerId: 'abc', items: [] });
    });

    it('create_order accepts valid input', async () => {
      const schema = getSchema(tools, 'create_order');
      expectPass(schema, {
        customerId: 'abc-123',
        items: [{ sku: 'WIDGET-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }],
      });
    });

    it('create_order enforces notes max length', async () => {
      const schema = getSchema(tools, 'create_order');
      expectFail(schema, {
        customerId: 'abc',
        items: [{ sku: 'X', name: 'Y', quantity: 1, unitPrice: 10 }],
        notes: 'x'.repeat(1001),
      });
    });
  });

  describe('inventory', () => {
    let tools;
    it('loads inventory tools', async () => {
      const mod = await import('../../src/tools/inventory.js');
      tools = mod.inventoryTools;
    });

    it('adjust_inventory requires integer quantity', async () => {
      const schema = getSchema(tools, 'adjust_inventory');
      expectFail(schema, { sku: 'X', quantity: 1.5, reason: 'test' });
    });

    it('reserve_inventory rejects zero quantity', async () => {
      const schema = getSchema(tools, 'reserve_inventory');
      expectFail(schema, { sku: 'X', quantity: 0, referenceType: 'order', referenceId: 'abc' });
    });

    it('reserve_inventory rejects non-positive expiresInSeconds', async () => {
      const schema = getSchema(tools, 'reserve_inventory');
      expectFail(schema, {
        sku: 'X', quantity: 1, referenceType: 'order', referenceId: 'abc',
        expiresInSeconds: 0,
      });
    });

    it('create_inventory_item accepts valid input', async () => {
      const schema = getSchema(tools, 'create_inventory_item');
      expectPass(schema, { sku: 'WIDGET-001', name: 'Widget' });
    });

    it('create_inventory_item rejects negative initialQuantity', async () => {
      const schema = getSchema(tools, 'create_inventory_item');
      expectFail(schema, { sku: 'X', name: 'Y', initialQuantity: -1 });
    });
  });

  describe('payments', () => {
    let tools;
    it('loads payment tools', async () => {
      const mod = await import('../../src/tools/payments.js');
      tools = mod.paymentTools;
    });

    it('create_payment rejects zero amount', async () => {
      const schema = getSchema(tools, 'create_payment');
      expectFail(schema, { orderId: 'abc', amount: 0 });
    });

    it('create_payment rejects negative amount', async () => {
      const schema = getSchema(tools, 'create_payment');
      expectFail(schema, { orderId: 'abc', amount: -10 });
    });

    it('create_payment accepts valid amount', async () => {
      const schema = getSchema(tools, 'create_payment');
      expectPass(schema, { orderId: 'abc', amount: 99.99 });
    });

    it('create_refund rejects zero amount', async () => {
      const schema = getSchema(tools, 'create_refund');
      expectFail(schema, { paymentId: 'abc', amount: 0 });
    });

    it('create_refund enforces reason max length', async () => {
      const schema = getSchema(tools, 'create_refund');
      expectFail(schema, { paymentId: 'abc', amount: 10, reason: 'x'.repeat(501) });
    });

    it('create_payment_intent rejects zero amount', async () => {
      const schema = getSchema(tools, 'create_payment_intent');
      expectFail(schema, { amount: 0 });
    });

    it('refund_payment_intent rejects negative amount', async () => {
      const schema = getSchema(tools, 'refund_payment_intent');
      expectFail(schema, { intentId: 'pi_1', amount: -1 });
    });

    it('list_payment_intents rejects non-integer limit', async () => {
      const schema = getSchema(tools, 'list_payment_intents');
      expectFail(schema, { limit: 10.5 });
    });

    it('list_payment_settlements rejects limit > 500', async () => {
      const schema = getSchema(tools, 'list_payment_settlements');
      expectFail(schema, { limit: 600 });
    });

    it('create_payment_settlement_batch rejects empty intentIds entries', async () => {
      const schema = getSchema(tools, 'create_payment_settlement_batch');
      expectFail(schema, { intentIds: [''] });
    });

    it('reconcile_payment_provider rejects non-boolean includeBalanced', async () => {
      const schema = getSchema(tools, 'reconcile_payment_provider');
      expectFail(schema, { includeBalanced: 'yes' });
    });

    it('ingest_payment_provider_webhook requires eventType', async () => {
      const schema = getSchema(tools, 'ingest_payment_provider_webhook');
      expectFail(schema, { providerId: 'stripe' });
    });
  });

  describe('carts', () => {
    let tools;
    it('loads cart tools', async () => {
      const mod = await import('../../src/tools/carts.js');
      tools = mod.cartTools;
    });

    it('add_cart_item rejects non-integer quantity', async () => {
      const schema = getSchema(tools, 'add_cart_item');
      expectFail(schema, { cartId: 'c1', sku: 'X', name: 'Y', quantity: 1.5, unitPrice: 10 });
    });

    it('add_cart_item rejects zero unitPrice', async () => {
      const schema = getSchema(tools, 'add_cart_item');
      expectFail(schema, { cartId: 'c1', sku: 'X', name: 'Y', quantity: 1, unitPrice: 0 });
    });

    it('add_cart_item rejects invalid imageUrl', async () => {
      const schema = getSchema(tools, 'add_cart_item');
      expectFail(schema, {
        cartId: 'c1', sku: 'X', name: 'Y', quantity: 1, unitPrice: 10,
        imageUrl: 'not-a-url',
      });
    });

    it('add_cart_item accepts valid imageUrl', async () => {
      const schema = getSchema(tools, 'add_cart_item');
      expectPass(schema, {
        cartId: 'c1', sku: 'X', name: 'Y', quantity: 1, unitPrice: 10,
        imageUrl: 'https://example.com/image.png',
      });
    });

    it('update_cart_item rejects non-integer quantity', async () => {
      const schema = getSchema(tools, 'update_cart_item');
      expectFail(schema, { itemId: 'i1', quantity: 2.5 });
    });

    it('set_cart_shipping_address enforces name max length', async () => {
      const schema = getSchema(tools, 'set_cart_shipping_address');
      expectFail(schema, {
        cartId: 'c1', firstName: 'x'.repeat(101), lastName: 'Y',
        line1: '123 Main', city: 'Anytown', postalCode: '12345', country: 'US',
      });
    });

    it('set_cart_shipping_address enforces country code length', async () => {
      const schema = getSchema(tools, 'set_cart_shipping_address');
      expectFail(schema, {
        cartId: 'c1', firstName: 'A', lastName: 'B',
        line1: '123 Main', city: 'Anytown', postalCode: '12345', country: 'X',
      });
    });

    it('create_cart rejects non-integer expiresInMinutes', async () => {
      const schema = getSchema(tools, 'create_cart');
      expectFail(schema, { expiresInMinutes: 1.5 });
    });
  });

  describe('products', () => {
    let tools;
    it('loads product tools', async () => {
      const mod = await import('../../src/tools/products.js');
      tools = mod.productTools;
    });

    it('create_product rejects empty name', async () => {
      const schema = getSchema(tools, 'create_product');
      expectFail(schema, { name: '' });
    });

    it('create_product enforces name max length', async () => {
      const schema = getSchema(tools, 'create_product');
      expectFail(schema, { name: 'x'.repeat(256) });
    });

    it('create_product variant rejects zero price', async () => {
      const schema = getSchema(tools, 'create_product');
      expectFail(schema, {
        name: 'Widget',
        variants: [{ sku: 'W1', price: 0 }],
      });
    });

    it('create_product variant rejects negative compareAtPrice', async () => {
      const schema = getSchema(tools, 'create_product');
      expectFail(schema, {
        name: 'Widget',
        variants: [{ sku: 'W1', price: 10, compareAtPrice: -5 }],
      });
    });
  });

  describe('customers', () => {
    let tools;
    it('loads customer tools', async () => {
      const mod = await import('../../src/tools/customers.js');
      tools = mod.customerTools;
    });

    it('create_customer enforces firstName max length', async () => {
      const schema = getSchema(tools, 'create_customer');
      expectFail(schema, {
        email: 'test@example.com', firstName: 'x'.repeat(101), lastName: 'Smith',
      });
    });

    it('create_customer accepts valid input', async () => {
      const schema = getSchema(tools, 'create_customer');
      expectPass(schema, { email: 'alice@example.com', firstName: 'Alice', lastName: 'Smith' });
    });
  });

  describe('suppliers', () => {
    let tools;
    it('loads supplier tools', async () => {
      const mod = await import('../../src/tools/suppliers.js');
      tools = mod.supplierTools;
    });

    it('create_supplier validates email format', async () => {
      const schema = getSchema(tools, 'create_supplier');
      expectFail(schema, { name: 'Acme', email: 'not-an-email' });
    });

    it('create_supplier accepts valid email', async () => {
      const schema = getSchema(tools, 'create_supplier');
      expectPass(schema, { name: 'Acme', email: 'contact@acme.com' });
    });

    it('create_supplier enforces name max length', async () => {
      const schema = getSchema(tools, 'create_supplier');
      expectFail(schema, { name: 'x'.repeat(256) });
    });
  });

  describe('returns', () => {
    let tools;
    it('loads return tools', async () => {
      const mod = await import('../../src/tools/returns.js');
      tools = mod.returnTools;
    });

    it('create_return rejects non-integer quantity', async () => {
      const schema = getSchema(tools, 'create_return');
      expectFail(schema, {
        orderId: 'o1', reason: 'defective',
        items: [{ orderItemId: 'i1', quantity: 1.5 }],
      });
    });

    it('create_return rejects empty items', async () => {
      const schema = getSchema(tools, 'create_return');
      expectFail(schema, { orderId: 'o1', reason: 'defective', items: [] });
    });

    it('reject_return enforces reason max length', async () => {
      const schema = getSchema(tools, 'reject_return');
      expectFail(schema, { returnId: 'r1', reason: 'x'.repeat(501) });
    });

    it('reject_return rejects empty reason', async () => {
      const schema = getSchema(tools, 'reject_return');
      expectFail(schema, { returnId: 'r1', reason: '' });
    });
  });

  describe('warranties', () => {
    let tools;
    it('loads warranty tools', async () => {
      const mod = await import('../../src/tools/warranties.js');
      tools = mod.warrantyTools;
    });

    it('create_warranty uses warrantyType enum', async () => {
      const schema = getSchema(tools, 'create_warranty');
      expectFail(schema, { customerId: 'c1', warrantyType: 'invalid_type' });
    });

    it('create_warranty accepts valid warrantyType', async () => {
      const schema = getSchema(tools, 'create_warranty');
      expectPass(schema, { customerId: 'c1', warrantyType: 'extended' });
    });

    it('create_warranty_claim uses claimType enum', async () => {
      const schema = getSchema(tools, 'create_warranty_claim');
      expectFail(schema, { warrantyId: 'w1', description: 'Broken', claimType: 'invalid' });
    });

    it('create_warranty_claim accepts valid claimType', async () => {
      const schema = getSchema(tools, 'create_warranty_claim');
      expectPass(schema, { warrantyId: 'w1', description: 'Broken', claimType: 'repair' });
    });

    it('create_warranty rejects non-integer durationMonths', async () => {
      const schema = getSchema(tools, 'create_warranty');
      expectFail(schema, { customerId: 'c1', durationMonths: 6.5 });
    });
  });

  describe('manufacturing', () => {
    let tools;
    it('loads manufacturing tools', async () => {
      const mod = await import('../../src/tools/manufacturing.js');
      tools = mod.manufacturingTools;
    });

    it('add_bom_component rejects zero quantity', async () => {
      const schema = getSchema(tools, 'add_bom_component');
      expectFail(schema, { bomId: 'b1', name: 'Onions', quantity: 0 });
    });

    it('create_work_order rejects non-integer quantityToBuild', async () => {
      const schema = getSchema(tools, 'create_work_order');
      expectFail(schema, { productId: 'p1', quantityToBuild: 5.5 });
    });

    it('create_work_order rejects zero quantityToBuild', async () => {
      const schema = getSchema(tools, 'create_work_order');
      expectFail(schema, { productId: 'p1', quantityToBuild: 0 });
    });

    it('complete_work_order rejects non-integer quantityCompleted', async () => {
      const schema = getSchema(tools, 'complete_work_order');
      expectFail(schema, { workOrderId: 'w1', quantityCompleted: 3.5 });
    });
  });

  describe('subscriptions', () => {
    let tools;
    it('loads subscription tools', async () => {
      const mod = await import('../../src/tools/subscriptions.js');
      tools = mod.subscriptionTools;
    });

    it('create_subscription_plan rejects zero price', async () => {
      const schema = getSchema(tools, 'create_subscription_plan');
      expectFail(schema, { name: 'Pro', billingInterval: 'monthly', price: 0 });
    });

    it('create_subscription_plan rejects negative price', async () => {
      const schema = getSchema(tools, 'create_subscription_plan');
      expectFail(schema, { name: 'Pro', billingInterval: 'monthly', price: -10 });
    });

    it('create_subscription_plan rejects non-integer trialDays', async () => {
      const schema = getSchema(tools, 'create_subscription_plan');
      expectFail(schema, { name: 'Pro', billingInterval: 'monthly', price: 10, trialDays: 7.5 });
    });

    it('create_subscription_plan accepts valid input', async () => {
      const schema = getSchema(tools, 'create_subscription_plan');
      expectPass(schema, { name: 'Pro', billingInterval: 'monthly', price: 29.99, trialDays: 14 });
    });
  });

  describe('tax', () => {
    let tools;
    it('loads tax tools', async () => {
      const mod = await import('../../src/tools/tax.js');
      tools = mod.taxTools;
    });

    it('calculate_tax rejects non-integer quantity', async () => {
      const schema = getSchema(tools, 'calculate_tax');
      expectFail(schema, {
        items: [{ id: 'i1', unitPrice: 10, quantity: 1.5 }],
        shippingAddress: { country: 'US' },
      });
    });

    it('calculate_tax rejects zero unitPrice', async () => {
      const schema = getSchema(tools, 'calculate_tax');
      expectFail(schema, {
        items: [{ id: 'i1', unitPrice: 0, quantity: 1 }],
        shippingAddress: { country: 'US' },
      });
    });

    it('calculate_tax_quote rejects empty items', async () => {
      const schema = getSchema(tools, 'calculate_tax_quote');
      expectFail(schema, {
        items: [],
        shippingAddress: { country: 'US' },
      });
    });

    it('validate_tax_jurisdiction_compliance rejects non-boolean strictCompliance', async () => {
      const schema = getSchema(tools, 'validate_tax_jurisdiction_compliance');
      expectFail(schema, {
        items: [{ id: 'i1', unitPrice: 10, quantity: 1 }],
        shippingAddress: { country: 'US' },
        strictCompliance: 'yes',
      });
    });

    it('calculate_tax_quote_with_failover rejects empty fallback provider IDs', async () => {
      const schema = getSchema(tools, 'calculate_tax_quote_with_failover');
      expectFail(schema, {
        items: [{ id: 'i1', unitPrice: 10, quantity: 1 }],
        shippingAddress: { country: 'US', state: 'CA', postalCode: '94105' },
        fallbackProviderIds: [''],
      });
    });

    it('list_tax_transactions rejects limit above 500', async () => {
      const schema = getSchema(tools, 'list_tax_transactions');
      expectFail(schema, { limit: 501 });
    });

    it('ingest_tax_provider_webhook requires eventType', async () => {
      const schema = getSchema(tools, 'ingest_tax_provider_webhook');
      expectFail(schema, { providerId: 'avalara' });
    });
  });

  describe('connectors', () => {
    let tools;
    it('loads connector tools', async () => {
      const mod = await import('../../src/tools/connectors.js');
      tools = mod.connectorTools;
    });

    it('publish_wasm_connector rejects invalid runtime kind', async () => {
      const schema = getSchema(tools, 'publish_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        version: '1.0.0',
        wasmPath: '/tmp/math.wasm',
        runtimeKind: 'invalid-runtime',
      });
    });

    it('install_wasm_connector rejects short connector id', async () => {
      const schema = getSchema(tools, 'install_wasm_connector');
      expectFail(schema, {
        connectorId: 'a',
      });
    });

    it('install_wasm_connector rejects non-boolean verifyStrict', async () => {
      const schema = getSchema(tools, 'install_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        verifyStrict: 'yes',
      });
    });

    it('install_wasm_connector rejects non-boolean requireCertified', async () => {
      const schema = getSchema(tools, 'install_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        requireCertified: 'true',
      });
    });

    it('install_wasm_connector rejects minSafetyScore above 100', async () => {
      const schema = getSchema(tools, 'install_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        minSafetyScore: 101,
      });
    });

    it('assess_wasm_connector_safety rejects short connector id', async () => {
      const schema = getSchema(tools, 'assess_wasm_connector_safety');
      expectFail(schema, {
        connectorId: 'a',
      });
    });

    it('certify_wasm_connector rejects unsupported status', async () => {
      const schema = getSchema(tools, 'certify_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        status: 'active',
      });
    });

    it('certify_wasm_connector rejects minSafetyScore above 100', async () => {
      const schema = getSchema(tools, 'certify_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        minSafetyScore: 200,
      });
    });

    it('sign_wasm_connector_attestation rejects short connector id', async () => {
      const schema = getSchema(tools, 'sign_wasm_connector_attestation');
      expectFail(schema, {
        connectorId: 'a',
      });
    });

    it('verify_wasm_connector_attestation rejects short connector id', async () => {
      const schema = getSchema(tools, 'verify_wasm_connector_attestation');
      expectFail(schema, {
        connectorId: 'a',
      });
    });

    it('execute_wasm_connector rejects timeout below lower bound', async () => {
      const schema = getSchema(tools, 'execute_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        action: 'add',
        timeoutMs: 20,
      });
    });

    it('execute_wasm_connector rejects non-boolean verifyStrict', async () => {
      const schema = getSchema(tools, 'execute_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        action: 'add',
        verifyStrict: 'true',
      });
    });

    it('execute_wasm_connector rejects non-boolean requireCertified', async () => {
      const schema = getSchema(tools, 'execute_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        action: 'add',
        requireCertified: 'yes',
      });
    });

    it('execute_wasm_connector rejects minSafetyScore above 100', async () => {
      const schema = getSchema(tools, 'execute_wasm_connector');
      expectFail(schema, {
        connectorId: 'math',
        action: 'add',
        minSafetyScore: 101,
      });
    });
  });

  describe('stablecoin', () => {
    let tools;
    it('loads stablecoin tools', async () => {
      const mod = await import('../../src/tools/stablecoin.js');
      tools = mod.stablecoinTools;
    });

    it('create_stablecoin_payment rejects zero amount', async () => {
      const schema = getSchema(tools, 'create_stablecoin_payment');
      expectFail(schema, { toAddress: '0xabc', amount: 0 });
    });

    it('create_stablecoin_payment rejects negative amount', async () => {
      const schema = getSchema(tools, 'create_stablecoin_payment');
      expectFail(schema, { toAddress: '0xabc', amount: -50 });
    });
  });
});
