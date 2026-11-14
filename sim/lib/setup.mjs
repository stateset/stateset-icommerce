/**
 * Operator-side setup: the catalogue, the scarce stock and one durable budget
 * per principal. These are operator actions, not agent actions, so they go
 * through the ordinary (ungoverned) APIs exactly as a host would provision
 * them; every agent write afterwards goes through `executeKernelCommand`.
 */
import { CURRENCY, SKU, STORE_ID, TENANT_ID, principalCount, principalPlan } from './plan.mjs';

export async function prepareStore({ Commerce, dbPath, config }) {
  const commerce = await Commerce.open(dbPath);
  try {
    const product = await commerce.products.create({
      name: 'Swarm Widget',
      slug: 'swarm-widget',
      description: 'The one scarce SKU every simulated agent competes for.',
      variants: [{ sku: SKU, name: 'Swarm Widget', priceExact: config.price, isDefault: true }],
    });
    await commerce.products.activate(product.id);
    await commerce.inventory.createItem({
      sku: SKU,
      name: 'Swarm Widget',
      initialQuantity: config.stock,
    });

    const now = Date.now();
    const validFrom = new Date(now - 60 * 60 * 1000).toISOString();
    const expiresAt = new Date(now + 24 * 60 * 60 * 1000).toISOString();
    for (let index = 0; index < principalCount(config); index += 1) {
      const plan = principalPlan(index, config);
      await commerce.provisionEconomicBudget({
        budget_id: plan.budgetId,
        principal_id: plan.principal.id,
        tenant_id: TENANT_ID,
        store_id: STORE_ID,
        limit: { amount: plan.budgetLimit, currency: CURRENCY },
        valid_from: validFrom,
        expires_at: expiresAt,
      });
    }
  } finally {
    await commerce.close();
  }
}

/** The engine's own view of every budget, for comparison with the oracle's. */
export async function readBudgetStatuses({ Commerce, dbPath, config }) {
  const commerce = await Commerce.open(dbPath);
  try {
    const statuses = {};
    for (let index = 0; index < principalCount(config); index += 1) {
      const { budgetId } = principalPlan(index, config);
      statuses[budgetId] = await commerce.economicBudgetStatus(budgetId);
    }
    return statuses;
  } finally {
    await commerce.close();
  }
}
