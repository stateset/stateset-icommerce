import { getCommerceCtor, adaptCommerceApis } from '../../src/commerce.js';
import { orderFixture, operatorKernel } from './tui-orders.js';
export function shipmentKernel() {
  const kernel = operatorKernel();
  kernel.principal.capabilities.push('orders.ship_reserved');
  kernel.policy.commands['orders.ship_reserved'] = {
    ...kernel.policy.commands['orders.transition'],
    required_capabilities: ['orders.ship_reserved'],
  };
  return kernel;
}
export async function shipmentFixture(database) {
  const fixture = await orderFixture(database),
    Commerce = getCommerceCtor();
  const commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    await commerce.orders.updateStatus(fixture.orders[3].id, 'confirmed');
    await commerce.orders.updateStatus(fixture.orders[3].id, 'processing');
    return { ...fixture, order: await commerce.orders.get(fixture.orders[3].id) };
  } finally {
    await commerce.close();
  }
}
