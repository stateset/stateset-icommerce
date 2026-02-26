/**
 * Shopify Shadow Adapter
 *
 * Shadow mode never mutates external systems. It is intended for parity checks,
 * dry-run imports, and compatibility validation against live Shopify data.
 */

import { ShopifyAdapter } from '../shopify/index.js';

export class ShopifyShadowAdapter extends ShopifyAdapter {
  /**
   * @param {Object} [config]
   */
  constructor(config = {}) {
    super(config);
    this.platformName = 'shopify-shadow';
    this.shadowMode = true;
  }

  /**
   * Include fulfillments explicitly so order lifecycle parity is validated.
   * @returns {string[]}
   */
  getSupportedEntities() {
    return ['customers', 'products', 'inventory', 'orders', 'fulfillments'];
  }

  /**
   * Preserve dependency ordering for downstream imports.
   * @returns {string[]}
   */
  getImportOrder() {
    return ['customers', 'products', 'inventory', 'orders', 'fulfillments'];
  }
}

export default ShopifyShadowAdapter;
