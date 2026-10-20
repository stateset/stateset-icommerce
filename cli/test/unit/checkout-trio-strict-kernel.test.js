/**
 * Strict-kernel end-to-end for the checkout trio.
 *
 * Unlike checkout-trio.test.js (which mocks the governed executor), this file
 * drives the REAL kernel through `commerce.executeKernelCommand`:
 *
 *   update_checkout  -> each step runs as its own policy-checked kernel command
 *   complete_checkout -> checkout.commit, validated by the kernel itself
 *
 * The native binding is required. When it is unavailable (e.g. the prebuilt
 * `.node` binary was not compiled for this machine) the suite skips instead
 * of failing — CI builds the binding from source and runs these for real.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';

import { createKernelToolExecutor } from '../../src/kernel-tool-execution.js';
import { cartTools } from '../../src/tools/carts.js';

const COMMANDS = [
  'carts.shipping_address.set',
  'carts.payment_method.set',
  'carts.coupon.apply',
  'carts.tax.calculate',
  'checkout.commit',
];

function buildPolicy(commands) {
  return {
    version: 'trio-e2e-v1',
    commands: Object.fromEntries(
      commands.map((cmd) => [
        cmd,
        {
          required_capabilities: [cmd],
          requires_approval: false,
          requires_tenant: false,
          requires_store: false,
          requires_agent_delegation: false,
          requires_signed_authority: false,
        },
      ]),
    ),
  };
}

function buildKernel(commands) {
  return {
    strict: true,
    policy: buildPolicy(commands),
    principal: {
      id: 'agent:trio-e2e',
      kind: 'agent',
      tenant_id: 'tenant:trio-e2e',
      delegated_by: 'user:operator',
      capabilities: commands,
    },
    storeId: 'store:trio-e2e',
  };
}

const ADDRESS = {
  firstName: 'Ada',
  lastName: 'Lovelace',
  line1: '1 Main St',
  city: 'Novato',
  state: 'CA',
  postalCode: '94945',
  country: 'US',
};

describe('checkout trio — strict kernel e2e', () => {
  let Commerce = null;
  let nativeAvailable = false;

  before(async () => {
    try {
      ({ Commerce } = await import('@stateset/embedded'));
      // Instantiating proves the native binary actually loads.
      new Commerce(':memory:').close();
      nativeAvailable = true;
    } catch {
      nativeAvailable = false;
    }
  });

  const needsNative = () => {
    if (!nativeAvailable) {
      console.log('  (skip: native @stateset/embedded binding unavailable)');
      return false;
    }
    return true;
  };

  async function setup(commands = COMMANDS) {
    const commerce = new Commerce(':memory:');
    const kernel = buildKernel(commands);
    const executeGovernedTool = createKernelToolExecutor({
      commerce,
      kernel,
      allowApply: true,
      agentConfig: null,
    });
    const product = await commerce.products.create({ name: 'Trio Coffee' });
    const cart = await commerce.carts.create({ customerEmail: 'buyer@example.com' });
    await commerce.carts.addItem(cart.id, {
      productId: product.id,
      sku: 'TRIO-COFFEE',
      name: 'Trio Coffee',
      quantity: 2,
      unitPriceExact: '19.99',
    });
    const tools = Object.fromEntries(cartTools.map((t) => [t.name, t]));
    return { commerce, executeGovernedTool, kernel, cartId: cart.id, tools };
  }

  it('update_checkout applies shipping + payment as separate kernel commands', async () => {
    if (!needsNative()) return;
    const { commerce, executeGovernedTool, cartId, tools } = await setup();
    try {
      const result = await tools.update_checkout.handler({
        commerce,
        params: {
          cartId,
          shippingAddress: ADDRESS,
          paymentMethod: 'credit_card',
          paymentToken: 'tok_e2e',
        },
        allowApply: true,
        executeGovernedTool,
        kernelStrict: true,
      });

      assert.equal(result.success, true);
      assert.deepEqual(result.appliedSteps, ['shipping_address', 'payment_method']);
      assert.equal(result.checkout.readiness.ready, true);
      assert.deepEqual(result.checkout.readiness.missing, []);
      // Exact-decimal totals through the real kernel: 2 x 19.99 + CA tax.
      assert.equal(result.checkout.subtotalExact, '39.98');
      assert.equal(result.checkout.paymentMethod, 'credit_card');
      assert.equal(result.checkout.shippingAddress.city, 'Novato');
    } finally {
      commerce.close();
    }
  });

  it('update_checkout fails closed when the policy denies a step', async () => {
    if (!needsNative()) return;
    // Policy (and principal) without the coupon capability.
    const allowed = COMMANDS.filter((c) => c !== 'carts.coupon.apply');
    const { commerce, executeGovernedTool, cartId, tools } = await setup(allowed);
    try {
      const result = await tools.update_checkout.handler({
        commerce,
        params: { cartId, couponCode: 'WELCOME10' },
        allowApply: true,
        executeGovernedTool,
        kernelStrict: true,
      });

      assert.equal(result.success, false);
      assert.equal(result.step, 'apply_cart_discount');
    } finally {
      commerce.close();
    }
  });

  it('complete_checkout on an unready cart is rejected by the kernel', async () => {
    if (!needsNative()) return;
    const { commerce, executeGovernedTool, cartId } = await setup();
    try {
      // In strict mode complete_checkout maps straight to checkout.commit;
      // the kernel validates readiness itself.
      const result = await executeGovernedTool(
        'complete_checkout',
        { cartId },
        { requireGoverned: true },
      );

      assert.equal(result.success, false);
      assert.equal(result.commandType, 'checkout.commit');
      assert.match(result.receipt.error_code || '', /validation_failed/);
    } finally {
      commerce.close();
    }
  });

  it('full trio flow: update then complete creates a real order', async () => {
    if (!needsNative()) return;
    const { commerce, executeGovernedTool, cartId, tools } = await setup();
    try {
      const updated = await tools.update_checkout.handler({
        commerce,
        params: {
          cartId,
          shippingAddress: ADDRESS,
          paymentMethod: 'credit_card',
          paymentToken: 'tok_e2e',
        },
        allowApply: true,
        executeGovernedTool,
        kernelStrict: true,
      });
      assert.equal(updated.success, true);
      assert.equal(updated.checkout.readiness.ready, true);

      const completed = await executeGovernedTool(
        'complete_checkout',
        { cartId },
        { requireGoverned: true },
      );
      assert.equal(completed.success, true);
      assert.equal(completed.commandType, 'checkout.commit');
      assert.ok(completed.receipt?.result?.order_id, 'kernel commit returns an order id');
    } finally {
      commerce.close();
    }
  });
});
