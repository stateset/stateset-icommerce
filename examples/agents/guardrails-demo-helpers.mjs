// Shared setup for the guardrails quickstarts: a paid order plus a
// demo-scoped governed kernel, so each framework example can show the
// engine refusing an over-refund with a sealed, machine-readable receipt.
import { loadEmbeddedToolkitRuntime } from './embedded-toolkit-runtime.mjs';

const REFUND_COMMAND = 'payments.create_refund';

/** Demo kernel config. In production, load operator-owned files instead — */
/** see kernel/examples/strict-policy.json and strict-principal.json. */
export function demoKernel() {
  return {
    strict: true,
    storeId: 'store:demo',
    policy: {
      version: 'demo-v1',
      commands: {
        [REFUND_COMMAND]: {
          required_capabilities: [REFUND_COMMAND],
          requires_approval: false,
          requires_tenant: true,
          requires_store: true,
          allowed_tenant_ids: ['tenant:demo'],
          allowed_store_ids: ['store:demo'],
          requires_agent_delegation: true,
          requires_signed_authority: false,
        },
      },
      trusted_authority_keys: {},
    },
    principal: {
      id: 'agent:demo',
      kind: 'agent',
      tenant_id: 'tenant:demo',
      delegated_by: 'user:operator',
      capabilities: [REFUND_COMMAND],
    },
  };
}

/** In-memory store with one customer, one paid $100 order. */
export async function setupGuardrailsScenario() {
  const runtime = await loadEmbeddedToolkitRuntime();
  const commerce = new runtime.Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: 'buyer@example.com',
    firstName: 'Demo',
    lastName: 'Buyer',
  });
  await commerce.inventory.createItem({ sku: 'WIDGET-1', name: 'Widget', initialQuantity: 10 });
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'WIDGET-1', name: 'Widget', quantity: 1, unitPrice: 100.0 }],
    currency: 'USD',
  });
  const payment = await commerce.payments.create({
    orderId: order.id,
    customerId: customer.id,
    amount: order.totalAmount,
    currency: 'USD',
    paymentMethod: 'credit_card',
  });
  await commerce.payments.markCompleted(payment.id);
  const toolkitOptions = { capabilities: ['read:*', 'payments.*'], kernel: demoKernel() };
  return { runtime, commerce, payment, toolkitOptions };
}

/** Normalize a toolkit result into { blocked, code, message, refundNumber }. */
export function receiptOutcome(result) {
  if (typeof result === 'string') {
    // LangChain-style tools return their observation as a JSON string.
    result = JSON.parse(result);
  }
  const receipt = result?.result?.receipt || null;
  if (receipt && receipt.error_code) {
    return { blocked: true, code: receipt.error_code, message: receipt.error_message };
  }
  if (result?.status === 'error') {
    return { blocked: true, code: null, message: result.error };
  }
  return {
    blocked: false,
    code: null,
    message: null,
    refundNumber: receipt?.aggregate_id || result?.result?.refundNumber || null,
  };
}

export function outcomeLine(label, outcome) {
  if (outcome.blocked) {
    return `${label}: BLOCKED (${outcome.code || 'error'}) — ${outcome.message}`;
  }
  return `${label}: executed${outcome.refundNumber ? ` (${outcome.refundNumber})` : ''}`;
}
