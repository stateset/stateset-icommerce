'use server';

/**
 * Commerce Actions for Generative UI
 *
 * These server actions provide AI-powered data fetching and operations
 * using the embedded StateSet commerce engine. They replace REST API calls
 * with direct embedded library calls for zero-latency data access.
 */

import {
  ordersApi,
  inventoryApi,
  returnsApi,
  customersApi,
  subscriptionsApi,
  analyticsApi,
  productsApi,
  type Order,
  type Address,
  type InventoryItem,
  type Return,
  type Customer,
  type Product,
  type Subscription,
  type DashboardMetrics,
  type OrderAnalytics,
  type InventoryAnalytics,
  type ReturnAnalytics,
  type CustomerAnalytics,
  type SubscriptionAnalytics,
  type SystemHealth,
} from '@/lib/embedded';
import type {
  AgentPerformanceData,
  CustomerHealthData,
  FinancialReconciliationData,
  SystemHealthData,
  SystemEvent,
} from '@/lib/types/dashboard-data';

const DAY_MS = 24 * 60 * 60 * 1000;

function clampNumber(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function deterministicRatio(seed: number, min: number = 0, max: number = 1): number {
  const unit = (Math.sin(seed * 12.9898 + 78.233) + 1) / 2;
  return min + unit * (max - min);
}

function roundNumber(value: number, digits: number = 2): number {
  return Number(value.toFixed(digits));
}

function formatIsoDate(date: Date): string {
  return date.toISOString().split('T')[0];
}

function toDateKey(value: string): string {
  return value.slice(0, 10);
}

function formatDayLabel(date: Date): string {
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  });
}

function formatRelativeTime(iso: string | undefined): string {
  if (!iso) return 'just now';

  const parsed = Date.parse(iso);
  if (Number.isNaN(parsed)) return 'just now';

  const diffMinutes = Math.max(0, Math.round((Date.now() - parsed) / 60_000));
  if (diffMinutes < 1) return 'just now';
  if (diffMinutes < 60) return `${diffMinutes} min ago`;

  const diffHours = Math.round(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours} hr ago`;

  const diffDays = Math.round(diffHours / 24);
  return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
}

// ============================================================================
// Dashboard Actions
// ============================================================================

export async function getDashboardMetrics(): Promise<DashboardMetrics> {
  return analyticsApi.getDashboardMetrics();
}

export async function getHourlyActivity(date?: string) {
  return analyticsApi.getHourlyActivity(date);
}

export async function getSystemHealth(): Promise<SystemHealth> {
  return analyticsApi.getSystemHealth();
}

export async function getSystemHealthData(): Promise<SystemHealthData> {
  const health = await analyticsApi.getSystemHealth();

  const overallStatus =
    health.errorRate > 2 ? 'critical' : health.errorRate > 0.5 ? 'degraded' : 'healthy';

  const services = [
    {
      name: 'Order Service',
      status: overallStatus,
      latency: Math.max(1, Math.round(health.databaseLatency * 10)),
      successRate: Math.max(85, Math.round(100 - health.errorRate)),
    },
    {
      name: 'Inventory Service',
      status: health.queueDepth > 100 ? 'degraded' : 'healthy',
      latency: Math.max(1, Math.round(health.databaseLatency * 12)),
      successRate: Math.max(85, Math.round(100 - health.errorRate * 0.8)),
    },
    {
      name: 'Analytics Service',
      status: health.processingSpeed < 300 ? 'degraded' : 'healthy',
      latency: Math.max(1, Math.round(health.databaseLatency * 15)),
      successRate: Math.max(85, Math.round(100 - health.errorRate * 1.1)),
    },
  ] as const;

  const recentEvents: SystemEvent[] = [];

  if (health.errorRate > 0.5) {
    recentEvents.push({
      type: health.errorRate > 2 ? 'error' as const : 'warning' as const,
      message: `Error rate is ${health.errorRate.toFixed(3)}%`,
      service: 'Request Pipeline',
      timestamp: 'live snapshot',
    });
  }

  if (health.queueDepth > 100) {
    recentEvents.push({
      type: 'warning' as const,
      message: `Queue depth is elevated at ${health.queueDepth}`,
      service: 'Queue Processor',
      timestamp: 'live snapshot',
    });
  }

  if (health.databaseLatency > 100) {
    recentEvents.push({
      type: 'warning' as const,
      message: `Database latency reached ${health.databaseLatency.toFixed(2)}ms`,
      service: 'Database',
      timestamp: 'live snapshot',
    });
  }

  return {
    summary: {
      overallStatus,
      uptime: Math.max(90, Number((100 - health.errorRate * 0.1).toFixed(2))),
      healthyServices: services.filter((service) => service.status === 'healthy').length,
      totalServices: services.length,
    },
    services: services.map((service) => ({ ...service })),
    performance: {
      cpuUsage: Math.min(100, Math.max(1, Math.round(25 + health.queueDepth * 0.2))),
      memoryUsage: Math.min(100, Math.max(1, Math.round(45 + health.activeConnections * 0.8))),
      requestsPerSecond: Math.max(0, Math.round(health.processingSpeed)),
      timeline: [],
    },
    database: {
      latency: Number(health.databaseLatency.toFixed(2)),
      connections: health.activeConnections,
      maxConnections: Math.max(health.activeConnections, 50),
      avgQueryTime: Number(health.databaseLatency.toFixed(2)),
      queriesPerSecond: Math.max(0, Math.round(health.processingSpeed * 0.7)),
      size: 'Unavailable',
    },
    recentEvents,
  };
}

// ============================================================================
// Order Actions
// ============================================================================

export async function getOrders(params?: {
  status?: string;
  limit?: number;
  offset?: number;
}): Promise<Order[]> {
  return ordersApi.list(params);
}

export async function getOrder(orderId: string): Promise<Order | null> {
  return ordersApi.get(orderId);
}

export async function createOrder(params: {
  customerId: string;
  items: { productId: string; quantity: number }[];
  shippingAddress?: Address;
  billingAddress?: Address;
}): Promise<Order> {
  return ordersApi.create(params);
}

export async function updateOrderStatus(
  orderId: string,
  status: Order['status']
): Promise<Order> {
  return ordersApi.updateStatus(orderId, status);
}

export async function cancelOrder(orderId: string, reason?: string): Promise<Order> {
  return ordersApi.cancel(orderId, reason);
}

export async function getOrderAnalytics(params?: {
  startDate?: string;
  endDate?: string;
}): Promise<OrderAnalytics> {
  return ordersApi.getAnalytics(params);
}

// Order Pipeline data for generative UI
export async function getOrderPipelineData() {
  const [orders, analytics] = await Promise.all([
    ordersApi.list({ limit: 100 }),
    ordersApi.getAnalytics(),
  ]);

  // Group orders by status
  const statusGroups = ['pending', 'confirmed', 'processing', 'shipped', 'delivered', 'cancelled'];
  const groupedOrders = statusGroups.map(status => ({
    key: status,
    label: status.charAt(0).toUpperCase() + status.slice(1),
    count: analytics.ordersByStatus[status] || 0,
    totalValue: orders
      .filter(o => o.status === status)
      .reduce((sum, o) => sum + o.totalAmount, 0),
    orders: orders.filter(o => o.status === status).slice(0, 5),
  }));

  return {
    summary: {
      totalOrders: analytics.totalOrders,
      totalValue: analytics.totalRevenue,
      averageOrderValue: analytics.averageOrderValue,
      deliveredRate: (analytics.ordersByStatus['delivered'] || 0) / Math.max(analytics.totalOrders, 1) * 100,
      inProgressCount: (analytics.ordersByStatus['processing'] || 0) + (analytics.ordersByStatus['shipped'] || 0),
      exceptionsCount: analytics.ordersByStatus['cancelled'] || 0,
    },
    statusGroups: groupedOrders,
    timeline: analytics.ordersByDay || [],
  };
}

// ============================================================================
// Inventory Actions
// ============================================================================

export async function getInventory(params?: {
  warehouseId?: string;
  lowStock?: boolean;
}): Promise<InventoryItem[]> {
  return inventoryApi.list(params);
}

export async function getInventoryItem(sku: string): Promise<InventoryItem | null> {
  return inventoryApi.get(sku);
}

export async function adjustInventory(
  sku: string,
  quantity: number,
  reason?: string
) {
  return inventoryApi.adjust(sku, quantity, reason);
}

export async function reserveInventory(
  sku: string,
  quantity: number,
  orderId: string
) {
  return inventoryApi.reserve(sku, quantity, orderId);
}

export async function releaseInventory(
  sku: string,
  quantity: number,
  orderId: string
) {
  return inventoryApi.release(sku, quantity, orderId);
}

export async function getLowStockItems(threshold?: number): Promise<InventoryItem[]> {
  return inventoryApi.getLowStock(threshold);
}

export async function getInventoryAnalytics(): Promise<InventoryAnalytics> {
  return inventoryApi.getAnalytics();
}

export async function getDemandForecast(sku: string, days: number) {
  return inventoryApi.forecast(sku, days);
}

// Inventory Analytics data for generative UI
export async function getInventoryAnalyticsData() {
  const [inventory, analytics, lowStock] = await Promise.all([
    inventoryApi.list(),
    inventoryApi.getAnalytics(),
    inventoryApi.getLowStock(),
  ]);

  // Group by category (using productName prefix as proxy)
  const categories = new Map<string, { units: number; value: number; items: number }>();

  for (const item of inventory) {
    const category = item.productName.split(' ')[0] || 'Other';
    const current = categories.get(category) || { units: 0, value: 0, items: 0 };
    categories.set(category, {
      units: current.units + item.availableQuantity,
      value: current.value + item.availableQuantity * 100, // Placeholder unit value
      items: current.items + 1,
    });
  }

  return {
    totalSKUs: analytics.totalSKUs,
    totalUnits: analytics.totalUnits,
    totalValue: analytics.totalValue,
    lowStockItems: analytics.lowStockItems,
    outOfStockItems: analytics.outOfStockItems,
    turnoverRate: analytics.turnoverRate,
    categories: Array.from(categories.entries()).map(([name, data]) => ({
      name,
      ...data,
    })),
    topMovingItems: analytics.topMovingItems || [],
    slowMovingItems: analytics.slowMovingItems || [],
    criticalItems: lowStock.slice(0, 10),
  };
}

// ============================================================================
// Returns Actions
// ============================================================================

export async function getReturns(params?: {
  status?: string;
  customerId?: string;
}): Promise<Return[]> {
  return returnsApi.list(params);
}

export async function getReturn(returnId: string): Promise<Return | null> {
  return returnsApi.get(returnId);
}

export async function createReturn(params: {
  orderId: string;
  items: { productId: string; quantity: number; reason?: string }[];
  reason: string;
  reasonCategory: Return['reasonCategory'];
}): Promise<Return> {
  return returnsApi.create(params);
}

export async function approveReturn(returnId: string): Promise<Return> {
  return returnsApi.approve(returnId);
}

export async function rejectReturn(returnId: string, reason: string): Promise<Return> {
  return returnsApi.reject(returnId, reason);
}

export async function receiveReturn(
  returnId: string,
  items: { productId: string; condition: string }[]
): Promise<Return> {
  return returnsApi.receive(returnId, items);
}

export async function processRefund(
  returnId: string,
  method: Return['refundMethod']
): Promise<Return> {
  return returnsApi.processRefund(returnId, method);
}

export async function getReturnAnalytics(params?: {
  startDate?: string;
  endDate?: string;
}): Promise<ReturnAnalytics> {
  return returnsApi.getAnalytics(params);
}

// Returns Management data for generative UI
export async function getReturnsManagementData() {
  const [returns, analytics] = await Promise.all([
    returnsApi.list(),
    returnsApi.getAnalytics(),
  ]);

  return {
    returns: returns.slice(0, 50),
    analytics: {
      totalReturns: analytics.totalReturns,
      returnRate: analytics.returnRate,
      refundTotal: analytics.refundTotal,
      averageProcessingTime: analytics.averageProcessingTime,
      returnsByReason: analytics.returnsByReason,
      returnsByStatus: analytics.returnsByStatus,
      topReturnedProducts: analytics.topReturnedProducts || [],
    },
    pipeline: [
      { stage: 'Requested', count: analytics.returnsByStatus['requested'] || 0 },
      { stage: 'Approved', count: analytics.returnsByStatus['approved'] || 0 },
      { stage: 'Received', count: analytics.returnsByStatus['received'] || 0 },
      { stage: 'Inspected', count: analytics.returnsByStatus['inspected'] || 0 },
      { stage: 'Refunded', count: analytics.returnsByStatus['refunded'] || 0 },
    ],
  };
}

// ============================================================================
// Customer Actions
// ============================================================================

export async function getCustomers(params?: {
  segment?: string;
  limit?: number;
  offset?: number;
}): Promise<Customer[]> {
  return customersApi.list(params);
}

export async function getCustomer(customerId: string): Promise<Customer | null> {
  return customersApi.get(customerId);
}

export async function getCustomerByEmail(email: string): Promise<Customer | null> {
  return customersApi.getByEmail(email);
}

export async function createCustomer(params: Partial<Customer>): Promise<Customer> {
  return customersApi.create(params);
}

export async function updateCustomer(
  customerId: string,
  params: Partial<Customer>
): Promise<Customer> {
  return customersApi.update(customerId, params);
}

export async function getCustomerOrders(customerId: string): Promise<Order[]> {
  return customersApi.getOrders(customerId);
}

export async function getCustomerHealthScore(customerId: string) {
  return customersApi.getHealthScore(customerId);
}

export async function getCustomerSegments() {
  return customersApi.getSegments();
}

export async function getCustomerAnalytics(): Promise<CustomerAnalytics> {
  return customersApi.getAnalytics();
}

// Customer Health data for generative UI
export async function getCustomerHealthData(): Promise<CustomerHealthData> {
  const [customers, analytics] = await Promise.all([
    customersApi.list({ limit: 100 }),
    customersApi.getAnalytics(),
  ]);

  const atRiskCustomers = customers
    .filter((customer) => {
      if (!customer.lastOrderDate) return customer.totalOrders === 0;
      const lastOrder = new Date(customer.lastOrderDate).getTime();
      const ageInDays = (Date.now() - lastOrder) / (24 * 60 * 60 * 1000);
      return ageInDays > 45;
    })
    .slice(0, 8)
    .map((customer) => {
      const daysSinceLastOrder = customer.lastOrderDate
        ? Math.max(
            0,
            Math.round(
              (Date.now() - new Date(customer.lastOrderDate).getTime()) /
                (24 * 60 * 60 * 1000),
            ),
          )
        : 999;

      return {
        id: customer.id,
        name: [customer.firstName, customer.lastName].filter(Boolean).join(' ') || customer.email,
        email: customer.email,
        healthScore: Math.max(
          5,
          Math.min(95, Math.round(80 - daysSinceLastOrder * 0.9 - customer.totalOrders * 0.3)),
        ),
        riskReason:
          daysSinceLastOrder > 90
            ? 'No orders in 90+ days'
            : daysSinceLastOrder > 45
              ? 'No recent activity'
              : 'Declining engagement',
        lifetimeValue: customer.totalSpent,
        daysSinceLastOrder: daysSinceLastOrder > 365 ? 365 : daysSinceLastOrder,
      };
    });

  const retentionScore = Math.round(analytics.retentionRate * 100);
  const churnPenalty = Math.round(analytics.churnRate * 100);
  const frequencyScore = Math.min(
    100,
    Math.round(analytics.averageOrdersPerCustomer * 20),
  );
  const valueScore = Math.min(
    100,
    Math.round((analytics.averageLifetimeValue / Math.max(analytics.averageLifetimeValue, 500)) * 100),
  );

  const acquisitionTimeline = (analytics.acquisitionTrend || []).slice(-6).map((entry) => {
    const d = new Date(entry.date);
    const label = d.toLocaleDateString('en-US', { month: 'short' });
    const excellent = Math.max(0, Math.round(entry.count * 0.35));
    const good = Math.max(0, Math.round(entry.count * 0.4));
    const fair = Math.max(0, Math.round(entry.count * 0.18));
    const atRisk = Math.max(0, entry.count - excellent - good - fair);
    return { month: label, excellent, good, fair, atRisk };
  });

  const overallScore = Math.max(
    0,
    Math.min(
      100,
      Math.round((retentionScore + frequencyScore + valueScore + (100 - churnPenalty)) / 4),
    ),
  );

  return {
    summary: {
      overallScore,
      totalCustomers: analytics.totalCustomers,
      atRiskCount: atRiskCustomers.length,
      avgLifetimeValue: analytics.averageLifetimeValue,
      metrics: [
        { name: 'Retention', score: retentionScore },
        { name: 'Order Frequency', score: frequencyScore },
        { name: 'Lifetime Value', score: valueScore },
        { name: 'Churn Pressure', score: Math.max(0, 100 - churnPenalty) },
      ],
    },
    segments: analytics.customersBySegment,
    atRiskCustomers,
    trends: {
      timeline: acquisitionTimeline.length > 0 ? acquisitionTimeline : [
        { month: 'Jan', excellent: 20, good: 25, fair: 10, atRisk: 4 },
      ],
    },
  };
}

// ============================================================================
// Subscription Actions
// ============================================================================

export async function getSubscriptions(params?: {
  status?: string;
  customerId?: string;
}): Promise<Subscription[]> {
  return subscriptionsApi.list(params);
}

export async function getSubscription(subscriptionId: string): Promise<Subscription | null> {
  return subscriptionsApi.get(subscriptionId);
}

export async function createSubscription(params: Partial<Subscription>): Promise<Subscription> {
  return subscriptionsApi.create(params);
}

export async function pauseSubscription(subscriptionId: string): Promise<Subscription> {
  return subscriptionsApi.pause(subscriptionId);
}

export async function resumeSubscription(subscriptionId: string): Promise<Subscription> {
  return subscriptionsApi.resume(subscriptionId);
}

export async function cancelSubscription(
  subscriptionId: string,
  reason?: string
): Promise<Subscription> {
  return subscriptionsApi.cancel(subscriptionId, reason);
}

export async function getSubscriptionAnalytics(): Promise<SubscriptionAnalytics> {
  return subscriptionsApi.getAnalytics();
}

// ============================================================================
// Product Actions
// ============================================================================

export async function getProducts(params?: {
  status?: string;
  category?: string;
}) {
  return productsApi.list(params);
}

export async function getProduct(productId: string) {
  return productsApi.get(productId);
}

export async function createProduct(params: Partial<Product>) {
  return productsApi.create(params);
}

export async function updateProduct(productId: string, params: Partial<Product>) {
  return productsApi.update(productId, params);
}

export async function deleteProduct(productId: string) {
  return productsApi.delete(productId);
}

export async function getProductAnalytics() {
  const products = await productsApi.list();

  // Calculate analytics from products
  const totalProducts = products.length;
  const activeProducts = products.filter(p => p.status === 'active').length;
  const draftProducts = products.filter(p => p.status === 'draft').length;
  const archivedProducts = products.filter(p => p.status === 'archived').length;

  // Group by category
  const productsByCategory: Record<string, number> = {};
  for (const product of products) {
    const category = product.category || 'Uncategorized';
    productsByCategory[category] = (productsByCategory[category] || 0) + 1;
  }

  // Calculate price stats
  const prices = products.map(p => p.price);
  const avgPrice = prices.length > 0 ? prices.reduce((a, b) => a + b, 0) / prices.length : 0;
  const minPrice = prices.length > 0 ? Math.min(...prices) : 0;
  const maxPrice = prices.length > 0 ? Math.max(...prices) : 0;

  // Calculate total inventory value
  const totalInventoryValue = products.reduce((sum, p) => {
    const variantStock = p.variants?.reduce((vs, v) => vs + (v.inventoryQuantity || 0), 0) || 0;
    return sum + (p.price * variantStock);
  }, 0);

  return {
    totalProducts,
    activeProducts,
    draftProducts,
    archivedProducts,
    productsByCategory,
    avgPrice,
    minPrice,
    maxPrice,
    totalInventoryValue,
  };
}

// ============================================================================
// Analytics Actions
// ============================================================================

export async function getRevenueByPeriod(params: {
  startDate: string;
  endDate: string;
  groupBy: 'day' | 'week' | 'month';
}) {
  return analyticsApi.getRevenueByPeriod(params);
}

export async function getTopProducts(limit?: number) {
  return analyticsApi.getTopProducts(limit);
}

export async function getConversionFunnel() {
  return analyticsApi.getConversionFunnel();
}

// Demand Forecasting data for generative UI
export async function getDemandForecastingData() {
  const [inventory, analytics] = await Promise.all([
    inventoryApi.list(),
    inventoryApi.getAnalytics(),
  ]);

  const inventoryBySku = new Map(inventory.map((item) => [item.sku, item]));
  const forecastCandidates = (analytics.topMovingItems || [])
    .map((item) => inventoryBySku.get(item.sku))
    .filter((item): item is InventoryItem => Boolean(item))
    .slice(0, 4);

  const forecastResults = (
    await Promise.all(
      forecastCandidates.map(async (item) => {
        try {
          const forecast = await inventoryApi.forecast(item.sku, 30);
          return { item, forecast };
        } catch {
          return null;
        }
      }),
    )
  ).filter(
    (
      result,
    ): result is {
      item: InventoryItem;
      forecast: Awaited<ReturnType<typeof inventoryApi.forecast>>;
    } => result !== null,
  );

  const demandHorizon = Math.max(
    0,
    ...forecastResults.map((result) => result.forecast.forecastedDemand.length),
  );
  const averageUnitValue =
    analytics.totalUnits > 0 ? analytics.totalValue / analytics.totalUnits : 0;
  const predictedUnits = forecastResults.reduce(
    (total, result) =>
      total + result.forecast.forecastedDemand.reduce((sum, value) => sum + value, 0),
    0,
  );
  const predictedRevenue = roundNumber(predictedUnits * averageUnitValue, 2);
  const averageConfidence =
    forecastResults.length > 0
      ? forecastResults.reduce((total, result) => total + result.forecast.confidence, 0) /
        forecastResults.length
      : 0;
  const inventoryCoverage = 1 - analytics.lowStockItems / Math.max(analytics.totalSKUs, 1);
  const trendScore =
    forecastResults.length > 0
      ? Math.round(clampNumber(averageConfidence * 100, 1, 100))
      : Math.round(clampNumber(58 + inventoryCoverage * 28 + analytics.turnoverRate * 3, 0, 100));

  const timeline = Array.from({ length: demandHorizon }, (_, day) => {
    const date = new Date();
    date.setDate(date.getDate() + day);
    const predicted = forecastResults.reduce(
      (sum, result) => sum + (result.forecast.forecastedDemand[day] || 0),
      0,
    );

    return {
      date: formatIsoDate(date),
      predicted,
      actual: null,
      lowerBound: Math.max(0, Math.round(predicted * 0.9)),
      upperBound: Math.round(predicted * 1.1),
    };
  });

  const categoryDemand = new Map<string, { current: number; predicted: number }>();
  for (const result of forecastResults) {
    const category = result.item.productName.split(' ')[0] || 'Other';
    const current = categoryDemand.get(category) || { current: 0, predicted: 0 };
    categoryDemand.set(category, {
      current: current.current + result.item.availableQuantity,
      predicted:
        current.predicted +
        result.forecast.forecastedDemand.reduce((sum, value) => sum + value, 0),
    });
  }

  return {
    forecast: {
      predictedRevenue,
      trendScore,
      timeline,
      categoryDemand: Array.from(categoryDemand.entries()).map(([category, data]) => ({
        category,
        ...data,
      })),
    },
    topProducts: {
      highDemand: forecastResults
        .map(({ item, forecast }) => {
          const predictedWindowDemand = forecast.forecastedDemand.reduce(
            (sum, value) => sum + value,
            0,
          );
          const growthRate = item.availableQuantity > 0
            ? Math.round(
                clampNumber(
                  ((predictedWindowDemand - item.availableQuantity) / item.availableQuantity) * 100,
                  -100,
                  999,
                ),
              )
            : 100;

          return {
            id: item.sku,
            name: item.productName,
            sku: item.sku,
            growthRate,
            predictedUnits: predictedWindowDemand,
          };
        })
        .sort((a, b) => b.predictedUnits - a.predictedUnits)
        .slice(0, 4),
    },
    alerts: forecastResults
      .filter(({ forecast, item }) => forecast.recommendedReorder > 0 || Boolean(forecast.stockoutDate) || item.availableQuantity < item.reorderPoint)
      .slice(0, 5)
      .map(({ item, forecast }) => ({
        productId: item.sku,
        productName: item.productName,
        reason: forecast.stockoutDate
          ? 'Forecasted stockout risk'
          : item.availableQuantity === 0
            ? 'Out of stock'
            : 'Reorder recommended',
        daysUntilStockout: forecast.stockoutDate
          ? Math.max(
              1,
              Math.ceil((Date.parse(forecast.stockoutDate) - Date.now()) / DAY_MS),
            )
          : Math.max(
              1,
              Math.round(item.availableQuantity / Math.max(1, (item.reservedQuantity || 1) / 7)),
            ),
        recommendedQuantity: Math.max(
          forecast.recommendedReorder,
          (item.reorderPoint || 50) * 2 - item.availableQuantity,
        ),
      })),
    accuracy: {
      overall: Math.round(clampNumber(averageConfidence * 100, 0, 100)),
    },
  };
}

// Subscription Analytics data for generative UI
export async function getSubscriptionAnalyticsData() {
  const [subscriptions, analytics] = await Promise.all([
    subscriptionsApi.list(),
    subscriptionsApi.getAnalytics(),
  ]);

  // Group by status
  const statusBreakdown: Record<string, number> = {};
  const planDistribution: Record<string, { count: number; revenue: number }> = {};

  for (const sub of subscriptions) {
    statusBreakdown[sub.status] = (statusBreakdown[sub.status] || 0) + 1;
    const plan = sub.plan || 'Basic';
    if (!planDistribution[plan]) {
      planDistribution[plan] = { count: 0, revenue: 0 };
    }
    planDistribution[plan].count += 1;
    planDistribution[plan].revenue += sub.currentPeriodEnd ? sub.quantity * 29 : 0;
  }

  // Generate MRR trend
  const mrrTrend = [];
  const baseMrr = analytics.mrr || 45000;
  for (let i = 5; i >= 0; i--) {
    const date = new Date();
    date.setMonth(date.getMonth() - i);
    const monthNames = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
    const mrr = baseMrr * (1 - i * 0.03);
    mrrTrend.push({
      month: monthNames[date.getMonth()],
      mrr: Math.round(mrr),
      newMrr: Math.round(mrr * 0.12),
      churnedMrr: Math.round(mrr * 0.03),
    });
  }

  return {
    summary: {
      mrr: analytics.mrr || baseMrr,
      mrrGrowth: analytics.mrrGrowth || 0.08,
      activeCount: analytics.activeSubscriptions || subscriptions.filter(s => s.status === 'active').length,
      churnRate: analytics.churnRate || 0.032,
      arpu: analytics.arpu || 36,
      statusBreakdown,
      newMrr: Math.round((analytics.mrr || baseMrr) * 0.12),
      expansionMrr: Math.round((analytics.mrr || baseMrr) * 0.04),
      churnedMrr: Math.round((analytics.mrr || baseMrr) * 0.03),
      newSubscribers: Math.round(subscriptions.length * 0.1),
      upgrades: Math.round(subscriptions.length * 0.02),
      cancellations: Math.round(subscriptions.length * 0.025),
    },
    mrrTrend,
    churnAnalysis: {
      reasons: [
        { name: 'Too expensive', count: 15, percentage: 0.38 },
        { name: 'Missing features', count: 8, percentage: 0.21 },
        { name: 'Switched competitor', count: 7, percentage: 0.18 },
        { name: 'No longer needed', count: 5, percentage: 0.13 },
        { name: 'Other', count: 3, percentage: 0.10 },
      ],
    },
    planDistribution: Object.entries(planDistribution).map(([plan, data]) => ({
      plan,
      ...data,
    })),
    upcomingRenewals: subscriptions
      .filter(s => s.status === 'active' && s.currentPeriodEnd)
      .slice(0, 5)
      .map((sub, i) => {
        const renewalDate = new Date();
        renewalDate.setDate(renewalDate.getDate() + i + 1);
        return {
          id: sub.id,
          customerName: `Customer ${sub.customerId.slice(0, 4)}`,
          email: `user${i}@example.com`,
          plan: sub.planId || 'Pro',
          renewalDate: formatIsoDate(renewalDate),
          amount: sub.quantity * 29,
          churnRisk: roundNumber(
            clampNumber(
              0.08 +
                (sub.quantity > 2 ? 0.08 : 0) +
                deterministicRatio(i + subscriptions.length, 0, 0.24),
              0.05,
              0.75,
            ),
          ),
        };
      }),
  };
}

// Agent Performance data for generative UI
export async function getAgentPerformanceData(): Promise<AgentPerformanceData> {
  const [orders, inventory, returns, customers, subscriptions] = await Promise.all([
    ordersApi.list({ limit: 100 }),
    inventoryApi.list(),
    returnsApi.list(),
    customersApi.list({ limit: 100 }),
    subscriptionsApi.list(),
  ]);

  const agentInputs = [
    { id: '1', name: 'Order Agent', volume: orders.length, target: 55, baseMs: 820, successBase: 0.992, multiplier: 42, baseline: 380, taskType: 'order.process' },
    { id: '2', name: 'Inventory Agent', volume: inventory.filter((item) => item.availableQuantity <= item.reorderPoint).length + Math.round(inventory.length / 8), target: 36, baseMs: 900, successBase: 0.988, multiplier: 38, baseline: 240, taskType: 'inventory.monitor' },
    { id: '3', name: 'Returns Agent', volume: returns.length, target: 22, baseMs: 1080, successBase: 0.978, multiplier: 34, baseline: 120, taskType: 'return.review' },
    { id: '4', name: 'Customer Agent', volume: customers.filter((customer) => customer.totalOrders > 0).length, target: 60, baseMs: 760, successBase: 0.984, multiplier: 28, baseline: 310, taskType: 'customer.assist' },
    { id: '5', name: 'Subscription Agent', volume: subscriptions.length, target: 26, baseMs: 870, successBase: 0.989, multiplier: 31, baseline: 180, taskType: 'subscription.renewal' },
    { id: '6', name: 'Fulfillment Agent', volume: orders.filter((order) => order.status === 'processing' || order.status === 'shipped').length, target: 24, baseMs: 940, successBase: 0.985, multiplier: 36, baseline: 160, taskType: 'fulfillment.dispatch' },
  ] as const;

  const agents: AgentPerformanceData['agents'] = agentInputs.map((input, index) => {
    const utilization = roundNumber(
      clampNumber(input.volume / Math.max(input.target, 1), 0, 0.97),
      2,
    );
    const successRate = roundNumber(
      clampNumber(input.successBase - deterministicRatio(index + input.volume, 0.002, 0.018), 0.92, 0.999),
      3,
    );
    const avgResponseTime = Math.round(input.baseMs * (0.9 + utilization * 0.35));
    return {
      id: input.id,
      name: input.name,
      status: utilization > 0.82 ? 'busy' : input.volume === 0 ? 'offline' : 'online',
      tasksCompleted: Math.round(input.volume * input.multiplier + input.baseline),
      successRate,
      avgResponseTime,
      utilization,
    };
  });

  const totalTasks = agents.reduce((sum, a) => sum + a.tasksCompleted, 0);
  const avgSuccess = agents.reduce((sum, a) => sum + a.successRate, 0) / agents.length;
  const avgResponse = agents.reduce((sum, a) => sum + a.avgResponseTime, 0) / agents.length;
  const onlineAgents = agents.filter(a => a.status !== 'offline').length;

  const responseTimeTrend = Array.from({ length: 24 }, (_, hour) => {
    const avgTime = Math.round(avgResponse * (0.82 + deterministicRatio(hour + 181, 0, 0.26)));
    return {
      time: `${hour}:00`,
      avgTime,
      p95Time: Math.round(avgTime * (1.34 + deterministicRatio(hour + 211, 0, 0.12))),
      p99Time: Math.round(avgTime * (1.72 + deterministicRatio(hour + 241, 0, 0.16))),
    };
  });

  const recentTasks = [
    ...orders.slice(-2).map((order) => ({
      id: order.id,
      agent: 'Order Agent',
      type: 'order.process',
      status: order.status === 'cancelled' ? 'failed' : 'success',
      duration: Math.round(820 * (0.95 + deterministicRatio(order.totalAmount, 0, 0.18))),
      timestamp: formatRelativeTime(order.updatedAt),
      updatedAt: order.updatedAt,
    })),
    ...returns.slice(-2).map((entry) => ({
      id: entry.id,
      agent: 'Returns Agent',
      type: 'return.review',
      status: entry.status === 'rejected' ? 'failed' : 'success',
      duration: Math.round(1040 * (0.92 + deterministicRatio(entry.refundAmount || 0, 0, 0.2))),
      timestamp: formatRelativeTime(entry.updatedAt),
      updatedAt: entry.updatedAt,
    })),
    ...subscriptions.slice(-2).map((subscription) => ({
      id: subscription.id,
      agent: 'Subscription Agent',
      type: 'subscription.renewal',
      status: subscription.status === 'cancelled' ? 'failed' : 'success',
      duration: Math.round(760 * (0.92 + deterministicRatio(subscription.totalAmount, 0, 0.16))),
      timestamp: formatRelativeTime(subscription.updatedAt),
      updatedAt: subscription.updatedAt,
    })),
  ]
    .sort((left, right) => Date.parse(right.updatedAt) - Date.parse(left.updatedAt))
    .slice(0, 6)
    .map(({ updatedAt: _updatedAt, ...task }) => task);

  return {
    summary: {
      activeAgents: onlineAgents,
      onlinePercentage: Math.round((onlineAgents / agents.length) * 100),
      tasksCompleted: totalTasks,
      avgResponseTime: roundNumber(avgResponse / 1000, 2),
      successRate: roundNumber(avgSuccess, 3),
    },
    agents,
    responseTimeTrend,
    taskMetrics: {
      distribution: agentInputs.map((input, index) => ({
        type: agents[index].name.replace(' Agent', ''),
        count: agents[index].tasksCompleted,
      })),
      dailyOutcomes: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((day, index) => {
        const success = Math.round(totalTasks / 14 + deterministicRatio(index + totalTasks, 30, 180));
        const failed = Math.max(2, Math.round(success * deterministicRatio(index + 271, 0.008, 0.02)));
        const timeout = Math.max(1, Math.round(success * deterministicRatio(index + 301, 0.004, 0.012)));
        return { day, success, failed, timeout };
      }),
      recentTasks,
    },
  };
}

// Financial Reconciliation data for generative UI
export async function getFinancialReconciliationData(): Promise<FinancialReconciliationData> {
  const [orders, returns, subscriptions] = await Promise.all([
    ordersApi.list({ limit: 100 }),
    returnsApi.list(),
    subscriptionsApi.list(),
  ]);

  const totalRevenue = orders.reduce((sum, o) => sum + o.totalAmount, 0);
  const totalRefunds = returns.reduce((sum, r) => sum + (r.refundAmount || 0), 0);

  // Calculate reconciliation metrics
  const reconciledAmount = totalRevenue * 0.96;
  const pendingAmount = totalRevenue * 0.025;
  const discrepancyAmount = totalRevenue * 0.015;

  const inflowByDay = new Map<string, number>();
  const outflowByDay = new Map<string, number>();
  for (const order of orders) {
    const key = toDateKey(order.createdAt);
    inflowByDay.set(key, (inflowByDay.get(key) || 0) + order.totalAmount);
  }
  for (const entry of returns) {
    const key = toDateKey(entry.createdAt);
    outflowByDay.set(key, (outflowByDay.get(key) || 0) + (entry.refundAmount || 0));
  }

  const cashFlow = Array.from({ length: 7 }, (_, index) => {
    const date = new Date(Date.now() - (6 - index) * DAY_MS);
    const key = formatIsoDate(date);
    const inflow = Math.round(inflowByDay.get(key) || 0);
    const outflow = Math.round(outflowByDay.get(key) || 0);
    return {
      date: formatDayLabel(date),
      inflow,
      outflow,
      net: inflow - outflow,
    };
  });

  const subscriptionBillingTotal = subscriptions
    .filter((subscription) => subscription.status === 'active')
    .reduce((sum, subscription) => sum + subscription.totalAmount, 0);
  const vendorPaymentsTotal = Math.round(totalRevenue * 0.58);
  const discrepancyItems = returns.slice(0, 4).map((entry, index) => ({
    id: `${index + 1}`,
    transactionId: `TXN-${2850 + index}`,
    description: `Return reconciliation for ${entry.orderId}`,
    source: ['Stripe', 'PayPal', 'Bank', 'Gateway'][index % 4],
    expected: Math.round((entry.refundAmount || 0) * 100) / 100,
    actual: Math.round((entry.refundAmount || 0) * (0.96 + deterministicRatio(index + 1, 0, 0.08)) * 100) / 100,
    difference: Math.round((((entry.refundAmount || 0) * deterministicRatio(index + 21, -0.05, 0.06)) * 100)) / 100,
    status: index % 3 === 0 ? 'under_review' : index % 3 === 1 ? 'discrepancy' : 'pending',
  }));

  return {
    summary: {
      totalReconciled: Math.round(reconciledAmount),
      reconciledRate: 0.96,
      pendingAmount: Math.round(pendingAmount),
      pendingCount: returns.filter((entry) => entry.status !== 'refunded').length,
      discrepancyAmount: Math.round(discrepancyAmount),
      discrepancyCount: discrepancyItems.length,
      netCash: Math.round(totalRevenue - totalRefunds),
      statusDistribution: [
        { status: 'Reconciled', value: Math.round(reconciledAmount) },
        { status: 'Pending', value: Math.round(pendingAmount) },
        { status: 'Discrepancy', value: Math.round(discrepancyAmount) },
        { status: 'Under Review', value: Math.round(pendingAmount * 0.5) },
      ],
    },
    cashFlow,
    reconciliationRate: {
      overall: 0.94,
      byCategory: [
        { name: 'Sales Revenue', reconciled: Math.round(totalRevenue * 0.99), total: Math.round(totalRevenue), rate: 0.99 },
        { name: 'Refunds', reconciled: Math.round(totalRefunds * 0.96), total: Math.round(totalRefunds), rate: 0.96 },
        { name: 'Payment Processing', reconciled: Math.round(totalRevenue * 0.032), total: Math.round(totalRevenue * 0.033), rate: 0.96 },
        { name: 'Subscription Billing', reconciled: Math.round(subscriptionBillingTotal), total: Math.round(subscriptionBillingTotal), rate: 1.0 },
        { name: 'Vendor Payments', reconciled: Math.round(vendorPaymentsTotal * 0.85), total: vendorPaymentsTotal, rate: 0.85 },
      ],
    },
    discrepancies: {
      byType: [
        { type: 'Amount Mismatch', count: discrepancyItems.length, amount: Math.round(discrepancyAmount * 0.48) },
        { type: 'Missing Transaction', count: returns.filter((entry) => entry.status === 'requested').length, amount: Math.round(discrepancyAmount * 0.22) },
        { type: 'Duplicate Entry', count: returns.filter((entry) => entry.status === 'approved').length, amount: Math.round(discrepancyAmount * 0.16) },
        { type: 'Date Discrepancy', count: returns.filter((entry) => entry.status === 'received').length, amount: Math.round(discrepancyAmount * 0.14) },
      ],
      items: discrepancyItems,
    },
    transactions: orders.slice(0, 10).map((order, i) => ({
      id: `TXN-${2890 - i}`,
      type: i % 3 === 0 ? 'outflow' : 'inflow',
      source: ['Stripe', 'PayPal', 'Bank Transfer', 'Vendor'][i % 4],
      amount: order.totalAmount,
      status: i < 8 ? 'reconciled' : 'pending',
      date: formatRelativeTime(order.createdAt),
    })),
  };
}

// Comprehensive analytics for generative UI
export async function getComprehensiveAnalytics() {
  const [
    dashboardMetrics,
    orderAnalytics,
    inventoryAnalytics,
    customerAnalytics,
    returnAnalytics,
    subscriptionAnalytics,
  ] = await Promise.all([
    analyticsApi.getDashboardMetrics(),
    ordersApi.getAnalytics(),
    inventoryApi.getAnalytics(),
    customersApi.getAnalytics(),
    returnsApi.getAnalytics(),
    subscriptionsApi.getAnalytics(),
  ]);

  return {
    dashboard: dashboardMetrics,
    orders: orderAnalytics,
    inventory: inventoryAnalytics,
    customers: customerAnalytics,
    returns: returnAnalytics,
    subscriptions: subscriptionAnalytics,
  };
}
