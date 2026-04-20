/**
 * Analytics & Forecasting Tools Test Suite
 *
 * Tests for cli/src/tools/analytics.js
 * Covers: get_sales_summary, get_revenue_by_period, get_top_products,
 *         get_product_performance, get_customer_metrics, get_top_customers,
 *         get_inventory_health, get_low_stock_items, get_inventory_movement,
 *         get_demand_forecast, get_revenue_forecast, get_order_status_breakdown,
 *         get_fulfillment_metrics, get_return_metrics
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { analyticsTools } from '../../src/tools/analytics.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = analyticsTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockSalesSummary = {
  totalRevenue: 12500.0,
  orderCount: 150,
  averageOrderValue: 83.33,
  itemsSold: 420,
  uniqueCustomers: 95,
};

const mockTopProduct = {
  sku: 'WIDGET-001',
  name: 'Widget Pro',
  unitsSold: 85,
  revenue: 2550.0,
  orderCount: 60,
};

const mockRevenueByPeriod = {
  period: '2026-03-01',
  revenue: 4200.0,
  orderCount: 48,
};

const mockProductPerformance = {
  sku: 'WIDGET-001',
  name: 'Widget Pro',
  revenue: 2550.0,
  unitsSold: 85,
  grossMargin: 0.42,
};

const mockCustomerMetrics = {
  totalCustomers: 250,
  newCustomers: 35,
  returningCustomers: 215,
  averageLifetimeValue: 320.0,
  averageOrdersPerCustomer: 3.2,
};

const mockTopCustomer = {
  customerId: 'cust_001',
  name: 'Jane Doe',
  email: 'jane@example.com',
  orderCount: 12,
  totalSpent: 1450.0,
  averageOrderValue: 120.83,
};

const mockInventoryHealth = {
  totalSkus: 500,
  inStockSkus: 420,
  lowStockSkus: 50,
  outOfStockSkus: 30,
  totalValue: 125000.0,
};

const mockLowStockItem = {
  sku: 'GADGET-003',
  name: 'Gadget Mini',
  onHand: 5,
  allocated: 2,
  available: 3,
  reorderPoint: 10,
  averageDailySales: 2.5,
  daysOfStock: 1.2,
};

const mockInventoryMovement = {
  sku: 'WIDGET-001',
  movementType: 'received',
  quantity: 24,
  occurredAt: '2026-03-15T00:00:00Z',
};

const mockDemandForecast = {
  sku: 'WIDGET-001',
  name: 'Widget Pro',
  averageDailyDemand: 3.5,
  forecastedDemand: 105,
  confidence: 0.85,
  currentStock: 200,
  daysUntilStockout: 57,
  recommendedReorderQty: 100,
  trend: 'stable',
};

const mockRevenueForecast = {
  period: '2026-04',
  forecastedRevenue: 14200.0,
  lowerBound: 12000.0,
  upperBound: 16400.0,
  confidenceLevel: 0.9,
  basedOnPeriods: 6,
};

const mockOrderStatusBreakdown = {
  pending: 15,
  confirmed: 22,
  processing: 8,
  shipped: 45,
  delivered: 90,
  cancelled: 5,
  refunded: 3,
};

const mockFulfillmentMetrics = {
  averagePickTimeMinutes: 12.5,
  averagePackTimeMinutes: 6.2,
  onTimeShipmentRate: 0.96,
};

const mockReturnMetrics = {
  totalReturns: 18,
  returnRatePercent: 4.5,
  totalRefunded: 1250.0,
};

function makeAnalyticsCommerce() {
  return {
    analytics: {
      salesSummary: async () => mockSalesSummary,
      revenueByPeriod: async () => [mockRevenueByPeriod],
      topProducts: async () => [mockTopProduct],
      productPerformance: async () => [mockProductPerformance],
      customerMetrics: async () => mockCustomerMetrics,
      topCustomers: async () => [mockTopCustomer],
      inventoryHealth: async () => mockInventoryHealth,
      lowStockItems: async () => [mockLowStockItem],
      inventoryMovement: async () => [mockInventoryMovement],
      demandForecast: async () => [mockDemandForecast],
      revenueForecast: async () => [mockRevenueForecast],
      orderStatusBreakdown: async () => mockOrderStatusBreakdown,
      fulfillmentMetrics: async () => mockFulfillmentMetrics,
      returnMetrics: async () => mockReturnMetrics,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('analyticsTools — module exports', () => {
  it('exports an array of 14 tools', () => {
    assert.ok(Array.isArray(analyticsTools));
    assert.equal(analyticsTools.length, 14);
  });

  it('exports expected tool names', () => {
    const names = analyticsTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'get_sales_summary',
      'get_revenue_by_period',
      'get_top_products',
      'get_product_performance',
      'get_customer_metrics',
      'get_top_customers',
      'get_inventory_health',
      'get_low_stock_items',
      'get_inventory_movement',
      'get_demand_forecast',
      'get_revenue_forecast',
      'get_order_status_breakdown',
      'get_fulfillment_metrics',
      'get_return_metrics',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of analyticsTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of analyticsTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of analyticsTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have read permission (analytics is read-only)', () => {
    for (const tool of analyticsTools) {
      assert.equal(tool.permission, 'read', `${tool.name} should have read permission`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('analyticsTools — input schemas', () => {
  it('get_sales_summary has period field', () => {
    const schema = findTool('get_sales_summary').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_top_products has period and limit fields', () => {
    const schema = findTool('get_top_products').inputSchema;
    assert.ok(schema.period, 'missing period field');
    assert.ok(schema.limit, 'missing limit field');
  });

  it('get_revenue_by_period has period and granularity fields', () => {
    const schema = findTool('get_revenue_by_period').inputSchema;
    assert.ok(schema.period, 'missing period field');
    assert.ok(schema.granularity, 'missing granularity field');
  });

  it('get_product_performance has period field', () => {
    const schema = findTool('get_product_performance').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_customer_metrics has period field', () => {
    const schema = findTool('get_customer_metrics').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_top_customers has period and limit fields', () => {
    const schema = findTool('get_top_customers').inputSchema;
    assert.ok(schema.period, 'missing period field');
    assert.ok(schema.limit, 'missing limit field');
  });

  it('get_inventory_health has empty inputSchema', () => {
    const schema = findTool('get_inventory_health').inputSchema;
    assert.deepStrictEqual(schema, {});
  });

  it('get_low_stock_items has threshold field', () => {
    const schema = findTool('get_low_stock_items').inputSchema;
    assert.ok(schema.threshold, 'missing threshold field');
  });

  it('get_inventory_movement has period field', () => {
    const schema = findTool('get_inventory_movement').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_demand_forecast has skus and daysAhead fields', () => {
    const schema = findTool('get_demand_forecast').inputSchema;
    assert.ok(schema.skus, 'missing skus field');
    assert.ok(schema.daysAhead, 'missing daysAhead field');
  });

  it('get_revenue_forecast has periodsAhead and granularity fields', () => {
    const schema = findTool('get_revenue_forecast').inputSchema;
    assert.ok(schema.periodsAhead, 'missing periodsAhead field');
    assert.ok(schema.granularity, 'missing granularity field');
  });

  it('get_order_status_breakdown has period field', () => {
    const schema = findTool('get_order_status_breakdown').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_fulfillment_metrics has period field', () => {
    const schema = findTool('get_fulfillment_metrics').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });

  it('get_return_metrics has period field', () => {
    const schema = findTool('get_return_metrics').inputSchema;
    assert.ok(schema.period, 'missing period field');
  });
});

// ============================================================================
// Handler: get_sales_summary
// ============================================================================

describe('analyticsTools — get_sales_summary handler', () => {
  it('returns sales summary with correct shape', async () => {
    const tool = findTool('get_sales_summary');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'last30days');
    assert.ok(result.summary);
    assert.equal(result.summary.totalRevenue, 12500.0);
    assert.equal(result.summary.orderCount, 150);
    assert.equal(result.summary.averageOrderValue, 83.33);
    assert.equal(result.summary.itemsSold, 420);
    assert.equal(result.summary.uniqueCustomers, 95);
  });

  it('handles commerce error gracefully', async () => {
    const tool = findTool('get_sales_summary');
    try {
      await tool.handler({
        commerce: {},
        params: { period: 'last30days' },
      });
      assert.fail('should have thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });
});

describe('analyticsTools — get_revenue_by_period handler', () => {
  it('returns revenue rows', async () => {
    const tool = findTool('get_revenue_by_period');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days', granularity: 'day' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.rows[0].period, '2026-03-01');
  });
});

// ============================================================================
// Handler: get_top_products
// ============================================================================

describe('analyticsTools — get_top_products handler', () => {
  it('returns top products with correct shape', async () => {
    const tool = findTool('get_top_products');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days', limit: 10 },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'last30days');
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.products));
    assert.equal(result.products[0].sku, 'WIDGET-001');
    assert.equal(result.products[0].name, 'Widget Pro');
    assert.equal(result.products[0].unitsSold, 85);
    assert.equal(result.products[0].revenue, 2550.0);
    assert.equal(result.products[0].orderCount, 60);
  });
});

describe('analyticsTools — get_product_performance handler', () => {
  it('returns product performance rows', async () => {
    const tool = findTool('get_product_performance');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.products[0].sku, 'WIDGET-001');
  });
});

// ============================================================================
// Handler: get_customer_metrics
// ============================================================================

describe('analyticsTools — get_customer_metrics handler', () => {
  it('returns customer metrics with correct shape', async () => {
    const tool = findTool('get_customer_metrics');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'last30days');
    assert.ok(result.metrics);
    assert.equal(result.metrics.totalCustomers, 250);
    assert.equal(result.metrics.newCustomers, 35);
    assert.equal(result.metrics.returningCustomers, 215);
    assert.equal(result.metrics.averageLifetimeValue, 320.0);
    assert.equal(result.metrics.averageOrdersPerCustomer, 3.2);
  });
});

// ============================================================================
// Handler: get_top_customers
// ============================================================================

describe('analyticsTools — get_top_customers handler', () => {
  it('returns top customers with correct shape', async () => {
    const tool = findTool('get_top_customers');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'all_time', limit: 10 },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'all_time');
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.customers));
    assert.equal(result.customers[0].customerId, 'cust_001');
    assert.equal(result.customers[0].name, 'Jane Doe');
    assert.equal(result.customers[0].email, 'jane@example.com');
    assert.equal(result.customers[0].orderCount, 12);
    assert.equal(result.customers[0].totalSpent, 1450.0);
    assert.equal(result.customers[0].averageOrderValue, 120.83);
  });
});

// ============================================================================
// Handler: get_inventory_health
// ============================================================================

describe('analyticsTools — get_inventory_health handler', () => {
  it('returns inventory health with correct shape', async () => {
    const tool = findTool('get_inventory_health');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.ok(result.health);
    assert.equal(result.health.totalSkus, 500);
    assert.equal(result.health.inStockSkus, 420);
    assert.equal(result.health.lowStockSkus, 50);
    assert.equal(result.health.outOfStockSkus, 30);
    assert.equal(result.health.totalValue, 125000.0);
  });
});

// ============================================================================
// Handler: get_low_stock_items
// ============================================================================

describe('analyticsTools — get_low_stock_items handler', () => {
  it('returns low stock items with correct shape', async () => {
    const tool = findTool('get_low_stock_items');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { threshold: 10 },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.items));
    const item = result.items[0];
    assert.equal(item.sku, 'GADGET-003');
    assert.equal(item.name, 'Gadget Mini');
    assert.equal(item.onHand, 5);
    assert.equal(item.allocated, 2);
    assert.equal(item.available, 3);
    assert.equal(item.reorderPoint, 10);
    assert.equal(item.averageDailySales, 2.5);
    assert.equal(item.daysOfStock, 1.2);
  });
});

describe('analyticsTools — get_inventory_movement handler', () => {
  it('returns inventory movement rows', async () => {
    const tool = findTool('get_inventory_movement');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.movements[0].movementType, 'received');
  });
});

// ============================================================================
// Handler: get_demand_forecast
// ============================================================================

describe('analyticsTools — get_demand_forecast handler', () => {
  it('returns demand forecast with correct shape', async () => {
    const tool = findTool('get_demand_forecast');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { daysAhead: 30 },
    });
    assert.equal(result.success, true);
    assert.equal(result.daysAhead, 30);
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.forecasts));
    const f = result.forecasts[0];
    assert.equal(f.sku, 'WIDGET-001');
    assert.equal(f.averageDailyDemand, 3.5);
    assert.equal(f.forecastedDemand, 105);
    assert.equal(f.confidence, 0.85);
    assert.equal(f.currentStock, 200);
    assert.equal(f.daysUntilStockout, 57);
    assert.equal(f.recommendedReorderQty, 100);
    assert.equal(f.trend, 'stable');
  });

  it('accepts optional skus parameter', async () => {
    const tool = findTool('get_demand_forecast');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { skus: ['WIDGET-001'], daysAhead: 30 },
    });
    assert.equal(result.success, true);
  });
});

// ============================================================================
// Handler: get_revenue_forecast
// ============================================================================

describe('analyticsTools — get_revenue_forecast handler', () => {
  it('returns revenue forecast with correct shape', async () => {
    const tool = findTool('get_revenue_forecast');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { periodsAhead: 3, granularity: 'month' },
    });
    assert.equal(result.success, true);
    assert.equal(result.granularity, 'month');
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.forecasts));
    const f = result.forecasts[0];
    assert.equal(f.period, '2026-04');
    assert.equal(f.forecastedRevenue, 14200.0);
    assert.equal(f.lowerBound, 12000.0);
    assert.equal(f.upperBound, 16400.0);
    assert.equal(f.confidenceLevel, 0.9);
    assert.equal(f.basedOnPeriods, 6);
  });
});

// ============================================================================
// Handler: get_order_status_breakdown
// ============================================================================

describe('analyticsTools — get_order_status_breakdown handler', () => {
  it('returns order status breakdown with correct shape', async () => {
    const tool = findTool('get_order_status_breakdown');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'last30days');
    assert.ok(result.breakdown);
    assert.equal(result.breakdown.pending, 15);
    assert.equal(result.breakdown.confirmed, 22);
    assert.equal(result.breakdown.processing, 8);
    assert.equal(result.breakdown.shipped, 45);
    assert.equal(result.breakdown.delivered, 90);
    assert.equal(result.breakdown.cancelled, 5);
    assert.equal(result.breakdown.refunded, 3);
  });
});

describe('analyticsTools — get_fulfillment_metrics handler', () => {
  it('returns fulfillment metrics', async () => {
    const tool = findTool('get_fulfillment_metrics');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.metrics.onTimeShipmentRate, 0.96);
  });
});

// ============================================================================
// Handler: get_return_metrics
// ============================================================================

describe('analyticsTools — get_return_metrics handler', () => {
  it('returns return metrics with correct shape', async () => {
    const tool = findTool('get_return_metrics');
    const result = await tool.handler({
      commerce: makeAnalyticsCommerce(),
      params: { period: 'last30days' },
    });
    assert.equal(result.success, true);
    assert.equal(result.period, 'last30days');
    assert.ok(result.metrics);
    assert.equal(result.metrics.totalReturns, 18);
    assert.equal(result.metrics.returnRatePercent, 4.5);
    assert.equal(result.metrics.totalRefunded, 1250.0);
  });
});

// ============================================================================
// Error paths — commerce object missing methods
// ============================================================================

describe('analyticsTools — error paths (empty commerce)', () => {
  for (const tool of analyticsTools) {
    it(`${tool.name} throws TypeError when commerce.analytics is missing`, async () => {
      try {
        await tool.handler({ commerce: {}, params: {} });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err instanceof TypeError);
      }
    });
  }
});
