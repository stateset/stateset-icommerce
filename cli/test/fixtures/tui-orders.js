import { getCommerceCtor, adaptCommerceApis } from '../../src/commerce.js';

export function operatorKernel() {
  return {
    strict: true,
    storeId: 'store:tui',
    principal: {
      id: 'user:tui',
      kind: 'human',
      tenantId: 'tenant:tui',
      capabilities: ['orders.transition'],
    },
    policy: {
      version: 'tui-order-test-v1',
      commands: {
        'orders.transition': {
          required_capabilities: ['orders.transition'],
          requires_approval: false,
          requires_tenant: true,
          requires_store: true,
          allowed_tenant_ids: ['tenant:tui'],
          allowed_store_ids: ['store:tui'],
          requires_agent_delegation: false,
          requires_signed_authority: false,
        },
      },
      trusted_authority_keys: {},
    },
  };
}

export async function orderFixture(database) {
  const Commerce = getCommerceCtor();
  const commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    const customer = await commerce.customers.create({
      email: 'operator@owned.example',
      firstName: 'Owned',
      lastName: 'Fixture',
    });
    const product = await commerce.products.create({
      name: 'Order fixture',
      variants: [{ sku: 'OPERATOR-FIXTURE', priceExact: '12.51', isDefault: true }],
    });
    await commerce.products.activate(product.id);
    await commerce.inventory.createItem({
      sku: 'OPERATOR-FIXTURE',
      name: 'Order fixture',
      initialQuantity: 100,
    });
    const orders = [];
    for (let i = 0; i < 4; i++)
      orders.push(
        await commerce.orders.createExact({
          customerId: customer.id,
          currency: 'USD',
          stockPolicy: 'reject_if_insufficient',
          items: [
            {
              productId: product.id,
              sku: 'OPERATOR-FIXTURE',
              name: 'Order fixture',
              quantity: 2,
              unitPrice: '12.51',
            },
          ],
        }),
      );
    const payment = await commerce.payments.create({
      orderId: orders[3].id,
      amountExact: '25.02',
      currency: 'USD',
      paymentMethod: 'credit_card',
    });
    await commerce.payments.markCompleted(payment.id);
    return { orders, payment };
  } finally {
    await commerce.close();
  }
}
