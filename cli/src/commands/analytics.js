/**
 * Analytics Commands Module
 *
 * Read-oriented analytics and forecasting commands for stateset-direct.
 */

const DEFAULT_PERIOD = 'last30days';

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'sales': {
      const period = args[0] || DEFAULT_PERIOD;
      const summary = await commerce.analytics.salesSummary({ period });
      return formatSalesSummary(period, summary, { jsonOutput });
    }

    case 'revenue': {
      const period = args[0] || DEFAULT_PERIOD;
      const granularity = args[1] || 'day';
      const rows = await commerce.analytics.revenueByPeriod({ period, granularity });
      return formatRevenueRows(period, granularity, rows, { output, jsonOutput });
    }

    case 'products': {
      const period = args[0] || DEFAULT_PERIOD;
      const limit = Number.parseInt(args[1] || '10', 10);
      const products = await commerce.analytics.topProducts({ period, limit });
      return formatTopProducts(period, products, { output, jsonOutput });
    }

    case 'customers': {
      const period = args[0] || DEFAULT_PERIOD;
      const metrics = await commerce.analytics.customerMetrics({ period });
      return formatCustomerMetrics(period, metrics, { jsonOutput });
    }

    case 'health': {
      const health = await commerce.analytics.inventoryHealth();
      return formatInventoryHealth(health, { jsonOutput });
    }

    case 'low': {
      const threshold = Number.parseInt(args[0] || '10', 10);
      const items = await commerce.analytics.lowStockItems(threshold);
      return formatLowStock(threshold, items, { output, jsonOutput });
    }

    case 'forecast': {
      const daysAhead = Number.parseInt(args[0] || '30', 10);
      const skus = args.slice(1);
      const forecasts = await commerce.analytics.demandForecast(
        skus.length > 0 ? skus : undefined,
        daysAhead,
      );
      return formatForecasts(daysAhead, forecasts, { output, jsonOutput });
    }

    case 'fulfillment': {
      const period = args[0] || DEFAULT_PERIOD;
      const metrics = await commerce.analytics.fulfillmentMetrics({ period });
      return formatFulfillmentMetrics(period, metrics, { jsonOutput });
    }

    case 'returns': {
      const period = args[0] || DEFAULT_PERIOD;
      const metrics = await commerce.analytics.returnMetrics({ period });
      return formatReturnMetrics(period, metrics, { jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: analytics ${action}\n\n` +
          'Available actions:\n' +
          '  sales [period]               Sales summary\n' +
          '  revenue [period] [granularity]  Revenue over time\n' +
          '  products [period] [limit]    Top products\n' +
          '  customers [period]           Customer metrics\n' +
          '  health                       Inventory health\n' +
          '  low [threshold]              Low stock items\n' +
          '  forecast [days] [sku...]     Demand forecast\n' +
          '  fulfillment [period]         Fulfillment metrics\n' +
          '  returns [period]             Return metrics',
      );
  }
}

function formatSalesSummary(period, summary, { jsonOutput }) {
  if (jsonOutput) return { period, summary };
  return {
    summary,
    formatted:
      `Sales summary (${period})\n` +
      `${'-'.repeat(32)}\n` +
      `Revenue:        ${summary.totalRevenue}\n` +
      `Orders:         ${summary.orderCount}\n` +
      `AOV:            ${summary.averageOrderValue}\n` +
      `Items sold:     ${summary.itemsSold}\n` +
      `Customers:      ${summary.uniqueCustomers}`,
  };
}

function formatRevenueRows(period, granularity, rows, { output, jsonOutput }) {
  if (jsonOutput) return { period, granularity, rows };
  if (rows.length === 0) return { formatted: `No revenue data found for ${period}.` };
  const formatted = output.table(rows, [
    { key: 'period', header: 'Period' },
    { key: 'revenue', header: 'Revenue', align: 'right' },
    { key: 'orders', header: 'Orders', align: 'right' },
  ]);
  return { rows, formatted: `Revenue by ${granularity} (${period})\n\n${formatted}` };
}

function formatTopProducts(period, products, { output, jsonOutput }) {
  if (jsonOutput) return { period, products };
  if (products.length === 0) return { formatted: `No product analytics found for ${period}.` };
  const formatted = output.table(products, [
    { key: 'sku', header: 'SKU' },
    { key: 'name', header: 'Name' },
    { key: 'unitsSold', header: 'Units', align: 'right' },
    { key: 'revenue', header: 'Revenue', align: 'right' },
    { key: 'orderCount', header: 'Orders', align: 'right' },
  ]);
  return { products, formatted: `Top products (${period})\n\n${formatted}` };
}

function formatCustomerMetrics(period, metrics, { jsonOutput }) {
  if (jsonOutput) return { period, metrics };
  return {
    metrics,
    formatted:
      `Customer metrics (${period})\n` +
      `${'-'.repeat(32)}\n` +
      `Total customers:      ${metrics.totalCustomers}\n` +
      `New customers:        ${metrics.newCustomers}\n` +
      `Returning customers:  ${metrics.returningCustomers}\n` +
      `Avg lifetime value:   ${metrics.averageLifetimeValue}\n` +
      `Avg orders/customer:  ${metrics.averageOrdersPerCustomer}`,
  };
}

function formatInventoryHealth(health, { jsonOutput }) {
  if (jsonOutput) return { health };
  return {
    health,
    formatted:
      'Inventory health\n' +
      `${'-'.repeat(24)}\n` +
      `Total SKUs:       ${health.totalSkus}\n` +
      `In stock:         ${health.inStockSkus}\n` +
      `Low stock:        ${health.lowStockSkus}\n` +
      `Out of stock:     ${health.outOfStockSkus}\n` +
      `Inventory value:  ${health.totalValue}`,
  };
}

function formatLowStock(threshold, items, { output, jsonOutput }) {
  if (jsonOutput) return { threshold, items };
  if (items.length === 0) return { formatted: `No low stock items at threshold ${threshold}.` };
  const formatted = output.table(items, [
    { key: 'sku', header: 'SKU' },
    { key: 'name', header: 'Name' },
    { key: 'available', header: 'Available', align: 'right' },
    { key: 'reorderPoint', header: 'Reorder', align: 'right' },
    { key: 'daysOfStock', header: 'Days Left', align: 'right' },
  ]);
  return { items, formatted: `Low stock items (threshold ${threshold})\n\n${formatted}` };
}

function formatForecasts(daysAhead, forecasts, { output, jsonOutput }) {
  if (jsonOutput) return { daysAhead, forecasts };
  if (forecasts.length === 0)
    return { formatted: `No forecasts available for next ${daysAhead} days.` };
  const formatted = output.table(forecasts, [
    { key: 'sku', header: 'SKU' },
    { key: 'forecastDemand', header: 'Forecast', align: 'right' },
    { key: 'currentStock', header: 'Stock', align: 'right' },
    { key: 'daysUntilStockout', header: 'Days to Stockout', align: 'right' },
  ]);
  return { forecasts, formatted: `Demand forecast (${daysAhead} days)\n\n${formatted}` };
}

function formatFulfillmentMetrics(period, metrics, { jsonOutput }) {
  if (jsonOutput) return { period, metrics };
  return {
    metrics,
    formatted:
      `Fulfillment metrics (${period})\n` +
      `${'-'.repeat(35)}\n` +
      `Orders fulfilled:      ${metrics.ordersFulfilled}\n` +
      `Avg fulfill time:      ${metrics.averageFulfillmentTime}\n` +
      `On-time rate:          ${metrics.onTimeRate}\n` +
      `Exception rate:        ${metrics.exceptionRate}`,
  };
}

function formatReturnMetrics(period, metrics, { jsonOutput }) {
  if (jsonOutput) return { period, metrics };
  return {
    metrics,
    formatted:
      `Return metrics (${period})\n` +
      `${'-'.repeat(30)}\n` +
      `Return count:       ${metrics.returnCount}\n` +
      `Return rate:        ${metrics.returnRate}\n` +
      `Refund amount:      ${metrics.totalRefundAmount}\n` +
      `Top reason:         ${metrics.topReason || 'N/A'}`,
  };
}

export const metadata = {
  name: 'analytics',
  aliases: ['an', 'stats'],
  description: 'Analytics and forecasting commands',
  actions: {
    sales: { description: 'Sales summary', args: ['[period]'] },
    revenue: { description: 'Revenue by time period', args: ['[period]', '[granularity]'] },
    products: { description: 'Top products', args: ['[period]', '[limit]'] },
    customers: { description: 'Customer metrics', args: ['[period]'] },
    health: { description: 'Inventory health summary', args: [] },
    low: { description: 'Low stock items', args: ['[threshold]'] },
    forecast: { description: 'Demand forecast', args: ['[days]', '[sku...]'] },
    fulfillment: { description: 'Fulfillment metrics', args: ['[period]'] },
    returns: { description: 'Return metrics', args: ['[period]'] },
  },
};

export default { execute, metadata };
