/**
 * Analytics & Forecasting Tools Module
 *
 * MCP tool definitions for sales analytics and forecasting.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

const periodEnum = z
  .enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time'])
  .optional();

export const analyticsTools = [
  {
    name: 'get_sales_summary',
    description:
      'Get sales summary for a time period. Returns total revenue, order count, average order value, items sold, and unique customers.',
    inputSchema: {
      period: periodEnum.default('last30days').describe('Time period for the summary'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period } = params;
      const summary = await commerce.analytics.salesSummary({ period });
      return {
        success: true,
        period,
        summary: {
          totalRevenue: summary.totalRevenue,
          orderCount: summary.orderCount,
          averageOrderValue: summary.averageOrderValue,
          itemsSold: summary.itemsSold,
          uniqueCustomers: summary.uniqueCustomers,
        },
      };
    },
  },
  {
    name: 'get_top_products',
    description: 'Get top selling products by revenue or units sold.',
    inputSchema: {
      period: periodEnum.default('last30days').describe('Time period'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(10)
        .describe('Maximum number of products to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period, limit } = params;
      const products = await commerce.analytics.topProducts({ period, limit });
      return {
        success: true,
        period,
        count: products.length,
        products: products.map((p) => ({
          sku: p.sku,
          name: p.name,
          unitsSold: p.unitsSold,
          revenue: p.revenue,
          orderCount: p.orderCount,
        })),
      };
    },
  },
  {
    name: 'get_customer_metrics',
    description:
      'Get customer metrics including total customers, new customers, returning customers, and average lifetime value.',
    inputSchema: { period: periodEnum.default('last30days').describe('Time period') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period } = params;
      const metrics = await commerce.analytics.customerMetrics({ period });
      return {
        success: true,
        period,
        metrics: {
          totalCustomers: metrics.totalCustomers,
          newCustomers: metrics.newCustomers,
          returningCustomers: metrics.returningCustomers,
          averageLifetimeValue: metrics.averageLifetimeValue,
          averageOrdersPerCustomer: metrics.averageOrdersPerCustomer,
        },
      };
    },
  },
  {
    name: 'get_top_customers',
    description: 'Get top customers by total spend.',
    inputSchema: {
      period: periodEnum.default('all_time').describe('Time period'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(10)
        .describe('Maximum number of customers to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period, limit } = params;
      const customers = await commerce.analytics.topCustomers({ period, limit });
      return {
        success: true,
        period,
        count: customers.length,
        customers: customers.map((c) => ({
          customerId: c.customerId,
          name: c.name,
          email: c.email,
          orderCount: c.orderCount,
          totalSpent: c.totalSpent,
          averageOrderValue: c.averageOrderValue,
        })),
      };
    },
  },
  {
    name: 'get_inventory_health',
    description:
      'Get inventory health summary showing total SKUs, in-stock, low stock, and out of stock counts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const health = await commerce.analytics.inventoryHealth();
      return {
        success: true,
        health: {
          totalSkus: health.totalSkus,
          inStockSkus: health.inStockSkus,
          lowStockSkus: health.lowStockSkus,
          outOfStockSkus: health.outOfStockSkus,
          totalValue: health.totalValue,
        },
      };
    },
  },
  {
    name: 'get_low_stock_items',
    description: 'Get items that are low in stock or approaching reorder point.',
    inputSchema: {
      threshold: z.number().optional().describe('Stock threshold to consider as low (default: 10)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { threshold } = params;
      const items = await commerce.analytics.lowStockItems(threshold);
      return {
        success: true,
        count: items.length,
        items: items.map((i) => ({
          sku: i.sku,
          name: i.name,
          onHand: i.onHand,
          allocated: i.allocated,
          available: i.available,
          reorderPoint: i.reorderPoint,
          averageDailySales: i.averageDailySales,
          daysOfStock: i.daysOfStock,
        })),
      };
    },
  },
  {
    name: 'get_demand_forecast',
    description:
      'Get demand forecast for inventory items based on historical sales. Predicts future demand and days until stockout.',
    inputSchema: {
      skus: z
        .array(z.string())
        .optional()
        .describe('List of SKUs to forecast (all items if not specified)'),
      daysAhead: z.number().optional().default(30).describe('Number of days to forecast ahead'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { skus, daysAhead } = params;
      const forecasts = await commerce.analytics.demandForecast(skus, daysAhead);
      return {
        success: true,
        daysAhead,
        count: forecasts.length,
        forecasts: forecasts.map((f) => ({
          sku: f.sku,
          name: f.name,
          averageDailyDemand: f.averageDailyDemand,
          forecastedDemand: f.forecastedDemand,
          confidence: f.confidence,
          currentStock: f.currentStock,
          daysUntilStockout: f.daysUntilStockout,
          recommendedReorderQty: f.recommendedReorderQty,
          trend: f.trend,
        })),
      };
    },
  },
  {
    name: 'get_revenue_forecast',
    description: 'Get revenue forecast based on historical trends.',
    inputSchema: {
      periodsAhead: z.number().optional().default(3).describe('Number of periods to forecast'),
      granularity: z
        .enum(['day', 'week', 'month'])
        .optional()
        .default('month')
        .describe('Time granularity'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { periodsAhead, granularity } = params;
      const forecasts = await commerce.analytics.revenueForecast(periodsAhead, granularity);
      return {
        success: true,
        granularity,
        count: forecasts.length,
        forecasts: forecasts.map((f) => ({
          period: f.period,
          forecastedRevenue: f.forecastedRevenue,
          lowerBound: f.lowerBound,
          upperBound: f.upperBound,
          confidenceLevel: f.confidenceLevel,
          basedOnPeriods: f.basedOnPeriods,
        })),
      };
    },
  },
  {
    name: 'get_order_status_breakdown',
    description: 'Get breakdown of orders by status.',
    inputSchema: { period: periodEnum.default('last30days').describe('Time period') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period } = params;
      const breakdown = await commerce.analytics.orderStatusBreakdown({ period });
      return {
        success: true,
        period,
        breakdown: {
          pending: breakdown.pending,
          confirmed: breakdown.confirmed,
          processing: breakdown.processing,
          shipped: breakdown.shipped,
          delivered: breakdown.delivered,
          cancelled: breakdown.cancelled,
          refunded: breakdown.refunded,
        },
      };
    },
  },
  {
    name: 'get_return_metrics',
    description: 'Get return metrics including return rate and total refunds.',
    inputSchema: { period: periodEnum.default('last30days').describe('Time period') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { period } = params;
      const metrics = await commerce.analytics.returnMetrics({ period });
      return {
        success: true,
        period,
        metrics: {
          totalReturns: metrics.totalReturns,
          returnRatePercent: metrics.returnRatePercent,
          totalRefunded: metrics.totalRefunded,
        },
      };
    },
  },
];

export default analyticsTools;
