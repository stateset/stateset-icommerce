import { getCommerceCtor, adaptCommerceApis } from '../../src/commerce.js';
import { orderFixture } from './tui-orders.js';
import { operatorKernel } from './tui-orders.js';
export function refundKernel() {
  const kernel = operatorKernel();
  kernel.principal.capabilities.push('payments.create_refund');
  kernel.policy.commands['payments.create_refund'] = {
    ...kernel.policy.commands['orders.transition'],
    required_capabilities: ['payments.create_refund'],
  };
  return kernel;
}

export async function refundFixture(database) {
  const fixture = await orderFixture(database),
    Commerce = getCommerceCtor();
  const commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    const precisionPayment = await commerce.payments.create({
      amountExact: '1.00000001',
      currency: 'BTC',
      paymentMethod: 'credit_card',
    });
    await commerce.payments.markCompleted(precisionPayment.id);
    return { ...fixture, precisionPayment };
  } finally {
    await commerce.close();
  }
}
