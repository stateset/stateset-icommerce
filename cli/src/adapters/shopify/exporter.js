/**
 * StateSet → External Format Exporter
 *
 * Exports StateSet data for parity testing and round-trip verification.
 */

import { mapCustomerFromStateSet, mapProductFromStateSet } from './mapper.js';

/**
 * Export StateSet entities to Shopify-compatible JSON format.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {string} entityType - 'customers', 'products', 'orders'
 * @returns {Promise<Object[]>}
 */
export async function exportToJson(commerce, entityType) {
  switch (entityType) {
    case 'customers': {
      const customers = await commerce.customers.list();
      return (customers || []).map((c) => mapCustomerFromStateSet(c));
    }
    case 'products': {
      const products = await commerce.products.list();
      return (products || []).map((p) => mapProductFromStateSet(p));
    }
    case 'orders': {
      const orders = await commerce.orders.list();
      return orders || [];
    }
    case 'inventory': {
      if (commerce.inventory?.list) {
        const items = await commerce.inventory.list();
        return items || [];
      }
      return [];
    }
    default:
      throw new Error(`Unknown entity type: ${entityType}`);
  }
}
