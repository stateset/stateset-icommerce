/**
 * Commerce Tools Test Suite
 *
 * Comprehensive tests for all commerce tool modules:
 * - customers.js
 * - orders.js
 * - inventory.js
 * - returns.js
 * - products.js
 * - analytics.js
 * - carts.js
 * - payments.js
 * - shipments.js
 * - suppliers.js
 * - invoices.js
 * - warranties.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { customerTools } from '../../src/tools/customers.js';
import { orderTools } from '../../src/tools/orders.js';
import { inventoryTools } from '../../src/tools/inventory.js';
import { returnTools } from '../../src/tools/returns.js';
import { productTools } from '../../src/tools/products.js';
import { analyticsTools } from '../../src/tools/analytics.js';
import { cartTools } from '../../src/tools/carts.js';
import { paymentTools } from '../../src/tools/payments.js';
import { shipmentTools } from '../../src/tools/shipments.js';
import { supplierTools } from '../../src/tools/suppliers.js';
import { invoiceTools } from '../../src/tools/invoices.js';
import { warrantyTools } from '../../src/tools/warranties.js';

import { createMockCommerce } from '../helpers/mocks.js';
import { testCustomer } from '../helpers/fixtures.js';
import { assertSuccess, assertError, assertPreview, assertHasField } from '../helpers/assertions.js';

// ============================================================================
// CUSTOMER TOOLS
// ============================================================================

describe('Customer Tools', () => {
  describe('list_customers', () => {
    it('returns expected shape with count and customers array', async () => {
      const commerce = createMockCommerce({
        customers: {
          list: async () => [
            { id: 'c1', email: 'a@example.com', firstName: 'Alice', lastName: 'Smith', status: 'active', acceptsMarketing: false, createdAt: '2026-01-01' },
            { id: 'c2', email: 'b@example.com', firstName: 'Bob', lastName: 'Jones', status: 'active', acceptsMarketing: true, createdAt: '2026-01-02' }
          ],
          count: async () => 2
        }
      });
      const tool = customerTools.find(t => t.name === 'list_customers');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'count');
      assertHasField(result, 'customers');
      assert.strictEqual(result.count, 2);
      assert.strictEqual(result.customers.length, 2);
      assert.strictEqual(result.customers[0].email, 'a@example.com');
    });

    it('has read permission', () => {
      const tool = customerTools.find(t => t.name === 'list_customers');
      assert.strictEqual(tool.permission, 'read');
    });

    it('has valid input schema', () => {
      const tool = customerTools.find(t => t.name === 'list_customers');
      assert.ok(tool.inputSchema);
      assert.strictEqual(typeof tool.inputSchema, 'object');
    });
  });

  describe('get_customer', () => {
    it('returns customer by ID', async () => {
      const commerce = createMockCommerce({
        customers: {
          get: async (id) => ({ id, email: 'alice@example.com', firstName: 'Alice', lastName: 'Smith', status: 'active', createdAt: '2026-01-01' })
        }
      });
      const tool = customerTools.find(t => t.name === 'get_customer');
      const result = await tool.handler({ commerce, params: { identifier: 'c1' } });

      assertSuccess(result);
      assertHasField(result, 'customer');
      assert.strictEqual(result.customer.id, 'c1');
    });

    it('returns customer by email', async () => {
      const commerce = createMockCommerce({
        customers: {
          getByEmail: async (email) => ({ id: 'c1', email, firstName: 'Alice', lastName: 'Smith', status: 'active', createdAt: '2026-01-01' })
        }
      });
      const tool = customerTools.find(t => t.name === 'get_customer');
      const result = await tool.handler({ commerce, params: { identifier: 'alice@example.com' } });

      assertSuccess(result);
      assert.strictEqual(result.customer.email, 'alice@example.com');
    });

    it('returns error when customer not found', async () => {
      const commerce = createMockCommerce();
      const tool = customerTools.find(t => t.name === 'get_customer');
      const result = await tool.handler({ commerce, params: { identifier: 'nonexistent' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = customerTools.find(t => t.name === 'get_customer');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_customer', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = customerTools.find(t => t.name === 'create_customer');
      const result = await tool.handler({ commerce, params: testCustomer, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('creates customer when allowApply is true', async () => {
      const commerce = createMockCommerce({
        customers: {
          create: async (data) => ({ id: 'new_cust', ...data })
        }
      });
      const tool = customerTools.find(t => t.name === 'create_customer');
      const result = await tool.handler({ commerce, params: testCustomer, allowApply: true });

      assertSuccess(result);
      assertHasField(result, 'customer');
      assert.strictEqual(result.customer.email, testCustomer.email);
    });

    it('has write permission', () => {
      const tool = customerTools.find(t => t.name === 'create_customer');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// ORDER TOOLS
// ============================================================================

describe('Order Tools', () => {
  describe('list_orders', () => {
    it('returns expected shape with orders array', async () => {
      const commerce = createMockCommerce({
        orders: {
          list: async () => [
            { id: 'o1', orderNumber: 'ORD-001', customerId: 'c1', status: 'pending', totalAmount: 100, currency: 'USD', items: [{ id: 'i1' }] }
          ],
          count: async () => 1
        }
      });
      const tool = orderTools.find(t => t.name === 'list_orders');
      const result = await tool.handler({ commerce, params: { limit: 50 } });

      assertSuccess(result);
      assertHasField(result, 'orders');
      assert.strictEqual(result.orders.length, 1);
      assert.strictEqual(result.orders[0].itemCount, 1);
    });

    it('respects limit parameter', async () => {
      const commerce = createMockCommerce({
        orders: {
          list: async () => Array(100).fill(null).map((_, i) => ({ id: `o${i}`, orderNumber: `ORD-${i}`, totalAmount: 10 })),
          count: async () => 100
        }
      });
      const tool = orderTools.find(t => t.name === 'list_orders');
      const result = await tool.handler({ commerce, params: { limit: 10 } });

      assertSuccess(result);
      assert.strictEqual(result.orders.length, 10);
      assert.strictEqual(result.totalCount, 100);
    });

    it('has read permission', () => {
      const tool = orderTools.find(t => t.name === 'list_orders');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_order', () => {
    it('returns order details with items', async () => {
      const commerce = createMockCommerce({
        orders: {
          get: async (id) => ({
            id,
            orderNumber: 'ORD-001',
            customerId: 'c1',
            status: 'pending',
            totalAmount: 100,
            items: [{ id: 'i1', sku: 'W1', quantity: 2 }]
          })
        }
      });
      const tool = orderTools.find(t => t.name === 'get_order');
      const result = await tool.handler({ commerce, params: { identifier: 'o1' } });

      assertSuccess(result);
      assertHasField(result, 'order');
      assert.strictEqual(result.order.items.length, 1);
    });

    it('returns error when order not found', async () => {
      const commerce = createMockCommerce();
      const tool = orderTools.find(t => t.name === 'get_order');
      const result = await tool.handler({ commerce, params: { identifier: 'nonexistent' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = orderTools.find(t => t.name === 'get_order');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_order', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = orderTools.find(t => t.name === 'create_order');
      const result = await tool.handler({
        commerce,
        params: { customerId: 'c1', items: [{ sku: 'W1', name: 'Widget', quantity: 1, unitPrice: 10 }], currency: 'USD' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
      assertHasField(result, 'wouldCreate');
    });

    it('creates order when allowApply is true', async () => {
      const commerce = createMockCommerce({
        orders: {
          create: async (data) => ({ id: 'new_ord', orderNumber: 'ORD-001', status: 'pending', totalAmount: 10, ...data })
        }
      });
      const tool = orderTools.find(t => t.name === 'create_order');
      const result = await tool.handler({
        commerce,
        params: { customerId: 'c1', items: [{ sku: 'W1', name: 'Widget', quantity: 1, unitPrice: 10 }], currency: 'USD' },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'order');
    });

    it('has write permission', () => {
      const tool = orderTools.find(t => t.name === 'create_order');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('update_order_status', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = orderTools.find(t => t.name === 'update_order_status');
      const result = await tool.handler({ commerce, params: { orderId: 'o1', status: 'confirmed' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('updates status when allowApply is true', async () => {
      const commerce = createMockCommerce({
        orders: {
          updateStatus: async (id, status) => ({ id, orderNumber: 'ORD-001', status })
        }
      });
      const tool = orderTools.find(t => t.name === 'update_order_status');
      const result = await tool.handler({ commerce, params: { orderId: 'o1', status: 'confirmed' }, allowApply: true });

      assertSuccess(result);
      assert.strictEqual(result.order.status, 'confirmed');
    });

    it('has write permission', () => {
      const tool = orderTools.find(t => t.name === 'update_order_status');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('ship_order', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = orderTools.find(t => t.name === 'ship_order');
      const result = await tool.handler({ commerce, params: { orderId: 'o1', trackingNumber: 'TRACK123' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('ships order when allowApply is true', async () => {
      const commerce = createMockCommerce({
        orders: {
          ship: async (id, tracking) => ({ id, orderNumber: 'ORD-001', status: 'shipped', trackingNumber: tracking })
        }
      });
      const tool = orderTools.find(t => t.name === 'ship_order');
      const result = await tool.handler({ commerce, params: { orderId: 'o1', trackingNumber: 'TRACK123' }, allowApply: true });

      assertSuccess(result);
      assert.strictEqual(result.order.trackingNumber, 'TRACK123');
    });

    it('has write permission', () => {
      const tool = orderTools.find(t => t.name === 'ship_order');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('cancel_order', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = orderTools.find(t => t.name === 'cancel_order');
      const result = await tool.handler({ commerce, params: { orderId: 'o1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('cancels order when allowApply is true', async () => {
      const commerce = createMockCommerce({
        orders: {
          cancel: async (id) => ({ id, orderNumber: 'ORD-001', status: 'cancelled' })
        }
      });
      const tool = orderTools.find(t => t.name === 'cancel_order');
      const result = await tool.handler({ commerce, params: { orderId: 'o1' }, allowApply: true });

      assertSuccess(result);
      assert.strictEqual(result.order.status, 'cancelled');
    });

    it('has delete permission', () => {
      const tool = orderTools.find(t => t.name === 'cancel_order');
      assert.strictEqual(tool.permission, 'delete');
    });
  });
});

// ============================================================================
// INVENTORY TOOLS
// ============================================================================

describe('Inventory Tools', () => {
  describe('get_stock', () => {
    it('returns stock levels', async () => {
      const commerce = createMockCommerce({
        inventory: {
          getStock: async (sku) => ({ sku, name: 'Widget', totalOnHand: 100, totalAllocated: 20, totalAvailable: 80 })
        }
      });
      const tool = inventoryTools.find(t => t.name === 'get_stock');
      const result = await tool.handler({ commerce, params: { sku: 'W1' } });

      assertSuccess(result);
      assertHasField(result, 'stock');
      assert.strictEqual(result.stock.totalOnHand, 100);
      assert.strictEqual(result.stock.totalAvailable, 80);
    });

    it('returns error when SKU not found', async () => {
      const commerce = createMockCommerce({
        inventory: {
          getStock: async () => null
        }
      });
      const tool = inventoryTools.find(t => t.name === 'get_stock');
      const result = await tool.handler({ commerce, params: { sku: 'NONEXISTENT' } });

      assertError(result, 'No inventory item found');
    });

    it('has read permission', () => {
      const tool = inventoryTools.find(t => t.name === 'get_stock');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_inventory_item', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = inventoryTools.find(t => t.name === 'create_inventory_item');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', name: 'Widget', initialQuantity: 100 },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('creates item when allowApply is true', async () => {
      const commerce = createMockCommerce({
        inventory: {
          createItem: async (data) => ({ id: 'inv1', ...data })
        }
      });
      const tool = inventoryTools.find(t => t.name === 'create_inventory_item');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', name: 'Widget', initialQuantity: 100 },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'item');
    });

    it('has write permission', () => {
      const tool = inventoryTools.find(t => t.name === 'create_inventory_item');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('adjust_inventory', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce({
        inventory: {
          getStock: async () => ({ totalOnHand: 100 })
        }
      });
      const tool = inventoryTools.find(t => t.name === 'adjust_inventory');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', quantity: 50, reason: 'Received shipment' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
      assertHasField(result, 'wouldAdjust');
    });

    it('adjusts inventory when allowApply is true', async () => {
      const commerce = createMockCommerce({
        inventory: {
          adjust: async (sku, qty, reason) => ({ sku, adjusted: qty }),
          getStock: async () => ({ sku: 'W1', totalOnHand: 150, totalAvailable: 150 })
        }
      });
      const tool = inventoryTools.find(t => t.name === 'adjust_inventory');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', quantity: 50, reason: 'Received shipment' },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'stock');
    });

    it('has write permission', () => {
      const tool = inventoryTools.find(t => t.name === 'adjust_inventory');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('reserve_inventory', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = inventoryTools.find(t => t.name === 'reserve_inventory');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', quantity: 10, referenceType: 'order', referenceId: 'o1' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('reserves inventory when allowApply is true', async () => {
      const commerce = createMockCommerce({
        inventory: {
          reserve: async (sku, qty, refType, refId) => ({ id: 'res1', quantity: qty, status: 'reserved' })
        }
      });
      const tool = inventoryTools.find(t => t.name === 'reserve_inventory');
      const result = await tool.handler({
        commerce,
        params: { sku: 'W1', quantity: 10, referenceType: 'order', referenceId: 'o1' },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'reservation');
    });

    it('has write permission', () => {
      const tool = inventoryTools.find(t => t.name === 'reserve_inventory');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('confirm_reservation', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = inventoryTools.find(t => t.name === 'confirm_reservation');
      const result = await tool.handler({ commerce, params: { reservationId: 'res1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = inventoryTools.find(t => t.name === 'confirm_reservation');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('release_reservation', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = inventoryTools.find(t => t.name === 'release_reservation');
      const result = await tool.handler({ commerce, params: { reservationId: 'res1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = inventoryTools.find(t => t.name === 'release_reservation');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// RETURN TOOLS
// ============================================================================

describe('Return Tools', () => {
  describe('list_returns', () => {
    it('returns expected shape with returns array', async () => {
      const commerce = createMockCommerce({
        returns: {
          list: async () => [
            { id: 'r1', orderId: 'o1', status: 'pending', reason: 'defective', createdAt: '2026-01-01' }
          ],
          count: async () => 1
        }
      });
      const tool = returnTools.find(t => t.name === 'list_returns');
      const result = await tool.handler({ commerce, params: { limit: 50 } });

      assertSuccess(result);
      assertHasField(result, 'returns');
      assert.strictEqual(result.returns.length, 1);
    });

    it('has read permission', () => {
      const tool = returnTools.find(t => t.name === 'list_returns');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_return', () => {
    it('returns return details', async () => {
      const commerce = createMockCommerce({
        returns: {
          get: async (id) => ({ id, orderId: 'o1', status: 'pending', reason: 'defective' })
        }
      });
      const tool = returnTools.find(t => t.name === 'get_return');
      const result = await tool.handler({ commerce, params: { returnId: 'r1' } });

      assertSuccess(result);
      assertHasField(result, 'return');
    });

    it('returns error when return not found', async () => {
      const commerce = createMockCommerce();
      const tool = returnTools.find(t => t.name === 'get_return');
      const result = await tool.handler({ commerce, params: { returnId: 'nonexistent' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = returnTools.find(t => t.name === 'get_return');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_return', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = returnTools.find(t => t.name === 'create_return');
      const result = await tool.handler({
        commerce,
        params: { orderId: 'o1', reason: 'defective', items: [{ orderItemId: 'i1', quantity: 1 }] },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('creates return when allowApply is true', async () => {
      const commerce = createMockCommerce({
        returns: {
          create: async (data) => ({ id: 'r1', status: 'pending', ...data })
        }
      });
      const tool = returnTools.find(t => t.name === 'create_return');
      const result = await tool.handler({
        commerce,
        params: { orderId: 'o1', reason: 'defective', items: [{ orderItemId: 'i1', quantity: 1 }] },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'return');
    });

    it('has write permission', () => {
      const tool = returnTools.find(t => t.name === 'create_return');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('approve_return', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = returnTools.find(t => t.name === 'approve_return');
      const result = await tool.handler({ commerce, params: { returnId: 'r1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('approves return when allowApply is true', async () => {
      const commerce = createMockCommerce({
        returns: {
          approve: async (id) => ({ id, status: 'approved' })
        }
      });
      const tool = returnTools.find(t => t.name === 'approve_return');
      const result = await tool.handler({ commerce, params: { returnId: 'r1' }, allowApply: true });

      assertSuccess(result);
      assert.strictEqual(result.return.status, 'approved');
    });

    it('has write permission', () => {
      const tool = returnTools.find(t => t.name === 'approve_return');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('reject_return', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = returnTools.find(t => t.name === 'reject_return');
      const result = await tool.handler({
        commerce,
        params: { returnId: 'r1', reason: 'Outside return window' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('rejects return when allowApply is true', async () => {
      const commerce = createMockCommerce({
        returns: {
          reject: async (id, reason) => ({ id, status: 'rejected', reason })
        }
      });
      const tool = returnTools.find(t => t.name === 'reject_return');
      const result = await tool.handler({
        commerce,
        params: { returnId: 'r1', reason: 'Outside return window' },
        allowApply: true
      });

      assertSuccess(result);
      assert.strictEqual(result.return.status, 'rejected');
    });

    it('has write permission', () => {
      const tool = returnTools.find(t => t.name === 'reject_return');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// PRODUCT TOOLS
// ============================================================================

describe('Product Tools', () => {
  describe('list_products', () => {
    it('returns expected shape with products array', async () => {
      const commerce = createMockCommerce({
        products: {
          list: async () => [
            { id: 'p1', name: 'Widget', slug: 'widget', status: 'active', createdAt: '2026-01-01' }
          ],
          count: async () => 1
        }
      });
      const tool = productTools.find(t => t.name === 'list_products');
      const result = await tool.handler({ commerce, params: { limit: 50 } });

      assertSuccess(result);
      assertHasField(result, 'products');
      assert.strictEqual(result.products.length, 1);
    });

    it('has read permission', () => {
      const tool = productTools.find(t => t.name === 'list_products');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_product', () => {
    it('returns product details', async () => {
      const commerce = createMockCommerce({
        products: {
          get: async (id) => ({ id, name: 'Widget', slug: 'widget', status: 'active' })
        }
      });
      const tool = productTools.find(t => t.name === 'get_product');
      const result = await tool.handler({ commerce, params: { productId: 'p1' } });

      assertSuccess(result);
      assertHasField(result, 'product');
    });

    it('returns error when product not found', async () => {
      const commerce = createMockCommerce();
      const tool = productTools.find(t => t.name === 'get_product');
      const result = await tool.handler({ commerce, params: { productId: 'nonexistent' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = productTools.find(t => t.name === 'get_product');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_product_variant', () => {
    it('returns variant by SKU', async () => {
      const commerce = createMockCommerce({
        products: {
          getVariantBySku: async (sku) => ({ sku, name: 'Widget - Red', price: 29.99 })
        }
      });
      const tool = productTools.find(t => t.name === 'get_product_variant');
      const result = await tool.handler({ commerce, params: { sku: 'W1-RED' } });

      assertSuccess(result);
      assertHasField(result, 'variant');
    });

    it('returns error when variant not found', async () => {
      const commerce = createMockCommerce({
        products: {
          getVariantBySku: async () => null
        }
      });
      const tool = productTools.find(t => t.name === 'get_product_variant');
      const result = await tool.handler({ commerce, params: { sku: 'NONEXISTENT' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = productTools.find(t => t.name === 'get_product_variant');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_product', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = productTools.find(t => t.name === 'create_product');
      const result = await tool.handler({
        commerce,
        params: { name: 'Widget', description: 'A test widget' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('creates product when allowApply is true', async () => {
      const commerce = createMockCommerce({
        products: {
          create: async (data) => ({ id: 'p1', slug: 'widget', ...data })
        }
      });
      const tool = productTools.find(t => t.name === 'create_product');
      const result = await tool.handler({
        commerce,
        params: { name: 'Widget', description: 'A test widget' },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'product');
    });

    it('has write permission', () => {
      const tool = productTools.find(t => t.name === 'create_product');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// ANALYTICS TOOLS
// ============================================================================

describe('Analytics Tools', () => {
  describe('get_sales_summary', () => {
    it('returns sales metrics', async () => {
      const commerce = createMockCommerce({
        analytics: {
          salesSummary: async () => ({
            totalRevenue: 10000,
            orderCount: 50,
            averageOrderValue: 200,
            itemsSold: 150,
            uniqueCustomers: 30
          })
        }
      });
      const tool = analyticsTools.find(t => t.name === 'get_sales_summary');
      const result = await tool.handler({ commerce, params: { period: 'last30days' } });

      assertSuccess(result);
      assertHasField(result, 'summary');
      assert.strictEqual(result.summary.totalRevenue, 10000);
      assert.strictEqual(result.summary.orderCount, 50);
    });

    it('has read permission', () => {
      const tool = analyticsTools.find(t => t.name === 'get_sales_summary');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_top_products', () => {
    it('returns top products', async () => {
      const commerce = createMockCommerce({
        analytics: {
          topProducts: async () => [
            { sku: 'W1', name: 'Widget', unitsSold: 100, revenue: 2999 }
          ]
        }
      });
      const tool = analyticsTools.find(t => t.name === 'get_top_products');
      const result = await tool.handler({ commerce, params: { period: 'last30days', limit: 10 } });

      assertSuccess(result);
      assertHasField(result, 'products');
      assert.strictEqual(result.products.length, 1);
    });

    it('has read permission', () => {
      const tool = analyticsTools.find(t => t.name === 'get_top_products');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_customer_metrics', () => {
    it('returns customer metrics', async () => {
      const commerce = createMockCommerce({
        analytics: {
          customerMetrics: async () => ({
            totalCustomers: 100,
            newCustomers: 20,
            returningCustomers: 80,
            averageLifetimeValue: 500,
            averageOrdersPerCustomer: 3
          })
        }
      });
      const tool = analyticsTools.find(t => t.name === 'get_customer_metrics');
      const result = await tool.handler({ commerce, params: { period: 'last30days' } });

      assertSuccess(result);
      assertHasField(result, 'metrics');
      assert.strictEqual(result.metrics.totalCustomers, 100);
    });

    it('has read permission', () => {
      const tool = analyticsTools.find(t => t.name === 'get_customer_metrics');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_inventory_health', () => {
    it('returns inventory health metrics', async () => {
      const commerce = createMockCommerce({
        analytics: {
          inventoryHealth: async () => ({
            totalSkus: 100,
            inStockSkus: 80,
            lowStockSkus: 15,
            outOfStockSkus: 5,
            totalValue: 50000
          })
        }
      });
      const tool = analyticsTools.find(t => t.name === 'get_inventory_health');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'health');
      assert.strictEqual(result.health.totalSkus, 100);
    });

    it('has read permission', () => {
      const tool = analyticsTools.find(t => t.name === 'get_inventory_health');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_demand_forecast', () => {
    it('returns demand forecast', async () => {
      const commerce = createMockCommerce({
        analytics: {
          demandForecast: async () => [
            { sku: 'W1', name: 'Widget', averageDailyDemand: 5, forecastedDemand: 150, confidence: 0.85 }
          ]
        }
      });
      const tool = analyticsTools.find(t => t.name === 'get_demand_forecast');
      const result = await tool.handler({ commerce, params: { daysAhead: 30 } });

      assertSuccess(result);
      assertHasField(result, 'forecasts');
      assert.strictEqual(result.forecasts.length, 1);
    });

    it('has read permission', () => {
      const tool = analyticsTools.find(t => t.name === 'get_demand_forecast');
      assert.strictEqual(tool.permission, 'read');
    });
  });
});

// ============================================================================
// CART TOOLS
// ============================================================================

describe('Cart Tools', () => {
  describe('list_carts', () => {
    it('returns expected shape with carts array', async () => {
      const commerce = createMockCommerce({
        carts: {
          list: async () => [
            { id: 'cart1', cartNumber: 'CART-001', customerId: 'c1', status: 'active', itemCount: 2, grandTotal: 59.98 }
          ],
          count: async () => 1
        }
      });
      const tool = cartTools.find(t => t.name === 'list_carts');
      const result = await tool.handler({ commerce, params: { limit: 50 } });

      assertSuccess(result);
      assertHasField(result, 'carts');
      assert.strictEqual(result.carts.length, 1);
    });

    it('has read permission', () => {
      const tool = cartTools.find(t => t.name === 'list_carts');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('get_cart', () => {
    it('returns cart details', async () => {
      const commerce = createMockCommerce({
        carts: {
          get: async (id) => ({
            id,
            cartNumber: 'CART-001',
            status: 'active',
            items: [{ id: 'i1', sku: 'W1', quantity: 2 }],
            grandTotal: 59.98
          })
        }
      });
      const tool = cartTools.find(t => t.name === 'get_cart');
      const result = await tool.handler({ commerce, params: { identifier: 'cart1' } });

      assertSuccess(result);
      assertHasField(result, 'cart');
    });

    it('returns error when cart not found', async () => {
      const commerce = createMockCommerce();
      const tool = cartTools.find(t => t.name === 'get_cart');
      const result = await tool.handler({ commerce, params: { identifier: 'nonexistent' } });

      assertError(result, 'not found');
    });

    it('has read permission', () => {
      const tool = cartTools.find(t => t.name === 'get_cart');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_cart', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = cartTools.find(t => t.name === 'create_cart');
      const result = await tool.handler({
        commerce,
        params: { customerEmail: 'alice@example.com', currency: 'USD' },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('creates cart when allowApply is true', async () => {
      const commerce = createMockCommerce({
        carts: {
          create: async (data) => ({ id: 'cart1', cartNumber: 'CART-001', status: 'active', ...data })
        }
      });
      const tool = cartTools.find(t => t.name === 'create_cart');
      const result = await tool.handler({
        commerce,
        params: { customerEmail: 'alice@example.com', currency: 'USD' },
        allowApply: true
      });

      assertSuccess(result);
      assertHasField(result, 'cart');
    });

    it('has write permission', () => {
      const tool = cartTools.find(t => t.name === 'create_cart');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('add_cart_item', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = cartTools.find(t => t.name === 'add_cart_item');
      const result = await tool.handler({
        commerce,
        params: { cartId: 'cart1', sku: 'W1', name: 'Widget', quantity: 2, unitPrice: 29.99 },
        allowApply: false
      });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = cartTools.find(t => t.name === 'add_cart_item');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('complete_checkout', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce({
        carts: {
          get: async (id) => ({
            id,
            cartNumber: 'CART-001',
            customerEmail: 'alice@example.com',
            itemCount: 2,
            grandTotal: 59.98,
            currency: 'USD'
          })
        }
      });
      const tool = cartTools.find(t => t.name === 'complete_checkout');
      const result = await tool.handler({ commerce, params: { cartId: 'cart1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
      assertHasField(result, 'wouldCheckout');
    });

    it('has write permission', () => {
      const tool = cartTools.find(t => t.name === 'complete_checkout');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('cancel_cart', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = cartTools.find(t => t.name === 'cancel_cart');
      const result = await tool.handler({ commerce, params: { cartId: 'cart1' }, allowApply: false });

      assertPreview(result);
      assertError(result, '--apply');
    });

    it('has delete permission', () => {
      const tool = cartTools.find(t => t.name === 'cancel_cart');
      assert.strictEqual(tool.permission, 'delete');
    });
  });

  describe('get_abandoned_carts', () => {
    it('returns abandoned carts', async () => {
      const commerce = createMockCommerce({
        carts: {
          getAbandoned: async () => [
            { id: 'cart1', cartNumber: 'CART-001', customerEmail: 'alice@example.com', grandTotal: 59.98 }
          ]
        }
      });
      const tool = cartTools.find(t => t.name === 'get_abandoned_carts');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'carts');
      assert.strictEqual(result.carts.length, 1);
    });

    it('has read permission', () => {
      const tool = cartTools.find(t => t.name === 'get_abandoned_carts');
      assert.strictEqual(tool.permission, 'read');
    });
  });
});

// ============================================================================
// PAYMENT TOOLS
// ============================================================================

describe('Payment Tools', () => {
  describe('list_payments', () => {
    it('returns expected shape with payments array', async () => {
      const commerce = createMockCommerce({
        payments: {
          list: async () => [{ id: 'pay1', orderId: 'o1', amount: '100.00', status: 'completed' }],
          count: async () => 1
        }
      });
      const tool = paymentTools.find(t => t.name === 'list_payments');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'payments');
    });

    it('has read permission', () => {
      const tool = paymentTools.find(t => t.name === 'list_payments');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_payment', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = paymentTools.find(t => t.name === 'create_payment');
      const result = await tool.handler({
        commerce,
        params: { orderId: 'o1', amount: 100, currency: 'USD' },
        allowApply: false
      });

      assertError(result, '--apply');
      assertHasField(result, 'wouldDo');
    });

    it('has write permission', () => {
      const tool = paymentTools.find(t => t.name === 'create_payment');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('create_refund', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = paymentTools.find(t => t.name === 'create_refund');
      const result = await tool.handler({
        commerce,
        params: { paymentId: 'pay1', amount: 50, reason: 'Customer request' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = paymentTools.find(t => t.name === 'create_refund');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// SHIPMENT TOOLS
// ============================================================================

describe('Shipment Tools', () => {
  describe('list_shipments', () => {
    it('returns expected shape with shipments array', async () => {
      const commerce = createMockCommerce({
        shipments: {
          list: async () => [{ id: 'ship1', orderId: 'o1', carrier: 'FedEx', status: 'in_transit' }],
          count: async () => 1
        }
      });
      const tool = shipmentTools.find(t => t.name === 'list_shipments');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'shipments');
    });

    it('has read permission', () => {
      const tool = shipmentTools.find(t => t.name === 'list_shipments');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_shipment', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = shipmentTools.find(t => t.name === 'create_shipment');
      const result = await tool.handler({
        commerce,
        params: { orderId: 'o1', carrier: 'FedEx' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = shipmentTools.find(t => t.name === 'create_shipment');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('deliver_shipment', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = shipmentTools.find(t => t.name === 'deliver_shipment');
      const result = await tool.handler({ commerce, params: { shipmentId: 'ship1' }, allowApply: false });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = shipmentTools.find(t => t.name === 'deliver_shipment');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// SUPPLIER TOOLS
// ============================================================================

describe('Supplier Tools', () => {
  describe('list_suppliers', () => {
    it('returns expected shape with suppliers array', async () => {
      const commerce = createMockCommerce({
        purchaseOrders: {
          listSuppliers: async () => [{ id: 'sup1', name: 'Widget Supply Co', email: 'orders@widgetsupply.com' }]
        }
      });
      const tool = supplierTools.find(t => t.name === 'list_suppliers');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'suppliers');
    });

    it('has read permission', () => {
      const tool = supplierTools.find(t => t.name === 'list_suppliers');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_supplier', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = supplierTools.find(t => t.name === 'create_supplier');
      const result = await tool.handler({
        commerce,
        params: { name: 'Widget Supply Co', email: 'orders@widgetsupply.com' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = supplierTools.find(t => t.name === 'create_supplier');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('list_purchase_orders', () => {
    it('returns expected shape with POs array', async () => {
      const commerce = createMockCommerce({
        purchaseOrders: {
          list: async () => [{ id: 'po1', supplierId: 'sup1', status: 'draft' }],
          count: async () => 1
        }
      });
      const tool = supplierTools.find(t => t.name === 'list_purchase_orders');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'purchaseOrders');
    });

    it('has read permission', () => {
      const tool = supplierTools.find(t => t.name === 'list_purchase_orders');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_purchase_order', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = supplierTools.find(t => t.name === 'create_purchase_order');
      const result = await tool.handler({
        commerce,
        params: { supplierId: 'sup1', items: '[{"sku":"W1","quantity":100,"unitPrice":10}]' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = supplierTools.find(t => t.name === 'create_purchase_order');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('approve_purchase_order', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = supplierTools.find(t => t.name === 'approve_purchase_order');
      const result = await tool.handler({
        commerce,
        params: { purchaseOrderId: 'po1', approvedBy: 'Manager' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = supplierTools.find(t => t.name === 'approve_purchase_order');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});

// ============================================================================
// INVOICE TOOLS
// ============================================================================

describe('Invoice Tools', () => {
  describe('list_invoices', () => {
    it('returns expected shape with invoices array', async () => {
      const commerce = createMockCommerce({
        invoices: {
          list: async () => [{ id: 'inv1', customerId: 'c1', status: 'draft', total: 299.99 }],
          count: async () => 1
        }
      });
      const tool = invoiceTools.find(t => t.name === 'list_invoices');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'invoices');
    });

    it('has read permission', () => {
      const tool = invoiceTools.find(t => t.name === 'list_invoices');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_invoice', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = invoiceTools.find(t => t.name === 'create_invoice');
      const result = await tool.handler({
        commerce,
        params: {
          customerId: 'c1',
          items: '[{"description":"Widget","quantity":10,"unitPrice":29.99}]',
          dueDate: '2026-03-01'
        },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = invoiceTools.find(t => t.name === 'create_invoice');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('send_invoice', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = invoiceTools.find(t => t.name === 'send_invoice');
      const result = await tool.handler({ commerce, params: { invoiceId: 'inv1' }, allowApply: false });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = invoiceTools.find(t => t.name === 'send_invoice');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('record_invoice_payment', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = invoiceTools.find(t => t.name === 'record_invoice_payment');
      const result = await tool.handler({
        commerce,
        params: { invoiceId: 'inv1', amount: 299.99, paymentMethod: 'check' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = invoiceTools.find(t => t.name === 'record_invoice_payment');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('get_overdue_invoices', () => {
    it('returns overdue invoices', async () => {
      const commerce = createMockCommerce({
        invoices: {
          getOverdue: async () => [{ id: 'inv1', dueDate: '2026-01-01', daysOverdue: 10 }]
        }
      });
      const tool = invoiceTools.find(t => t.name === 'get_overdue_invoices');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'overdueInvoices');
    });

    it('has read permission', () => {
      const tool = invoiceTools.find(t => t.name === 'get_overdue_invoices');
      assert.strictEqual(tool.permission, 'read');
    });
  });
});

// ============================================================================
// WARRANTY TOOLS
// ============================================================================

describe('Warranty Tools', () => {
  describe('list_warranties', () => {
    it('returns expected shape with warranties array', async () => {
      const commerce = createMockCommerce({
        warranties: {
          list: async () => [{ id: 'war1', customerId: 'c1', warrantyType: 'standard', durationMonths: 12 }],
          count: async () => 1
        }
      });
      const tool = warrantyTools.find(t => t.name === 'list_warranties');
      const result = await tool.handler({ commerce, params: {} });

      assertSuccess(result);
      assertHasField(result, 'warranties');
    });

    it('has read permission', () => {
      const tool = warrantyTools.find(t => t.name === 'list_warranties');
      assert.strictEqual(tool.permission, 'read');
    });
  });

  describe('create_warranty', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = warrantyTools.find(t => t.name === 'create_warranty');
      const result = await tool.handler({
        commerce,
        params: { customerId: 'c1', productId: 'p1', warrantyType: 'standard', durationMonths: 12 },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = warrantyTools.find(t => t.name === 'create_warranty');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('create_warranty_claim', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = warrantyTools.find(t => t.name === 'create_warranty_claim');
      const result = await tool.handler({
        commerce,
        params: { warrantyId: 'war1', description: 'Product defective', claimType: 'replacement' },
        allowApply: false
      });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = warrantyTools.find(t => t.name === 'create_warranty_claim');
      assert.strictEqual(tool.permission, 'write');
    });
  });

  describe('approve_warranty_claim', () => {
    it('requires --apply flag', async () => {
      const commerce = createMockCommerce();
      const tool = warrantyTools.find(t => t.name === 'approve_warranty_claim');
      const result = await tool.handler({ commerce, params: { claimId: 'claim1' }, allowApply: false });

      assertError(result, '--apply');
    });

    it('has write permission', () => {
      const tool = warrantyTools.find(t => t.name === 'approve_warranty_claim');
      assert.strictEqual(tool.permission, 'write');
    });
  });
});
