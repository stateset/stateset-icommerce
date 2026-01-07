#!/usr/bin/env node
/**
 * StateSet iCommerce - Analytics & Forecasting Example
 *
 * This example demonstrates the analytics and forecasting capabilities:
 * - Sales summaries and revenue by period
 * - Product performance analysis
 * - Customer metrics and top customers
 * - Inventory health and movement tracking
 * - Demand forecasting
 * - Revenue forecasting
 * - Order status breakdown and fulfillment metrics
 *
 * Run with: node 03_analytics_and_forecasting.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Analytics & Forecasting ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup: Create sample data
  // ============================================

  console.log('[Setup] Creating sample data...');
  await createSampleData(commerce);
  console.log('    Sample data created\n');

  // ============================================
  // 1. Sales Summary
  // ============================================

  console.log('[1] Sales Summary...');

  // Get sales summary for different periods
  const todaySales = await commerce.analytics.salesSummary({ period: 'today' });
  console.log('    Today:');
  console.log(`      Revenue: $${todaySales.totalRevenue}`);
  console.log(`      Orders: ${todaySales.orderCount}`);
  console.log(`      Items Sold: ${todaySales.itemsSold}`);
  console.log(`      Average Order Value: $${todaySales.averageOrderValue.toFixed(2)}`);
  console.log(`      Unique Customers: ${todaySales.uniqueCustomers}`);

  const allTimeSales = await commerce.analytics.salesSummary({ period: 'all_time' });
  console.log('\n    All Time:');
  console.log(`      Revenue: $${allTimeSales.totalRevenue}`);
  console.log(`      Orders: ${allTimeSales.orderCount}`);
  console.log(`      Average Order Value: $${allTimeSales.averageOrderValue.toFixed(2)}\n`);

  // ============================================
  // 2. Revenue by Period
  // ============================================

  console.log('[2] Revenue by Period...');

  const revenueByDay = await commerce.analytics.revenueByPeriod({
    period: 'last7days',
    granularity: 'day'
  });

  console.log('    Revenue breakdown (last 7 days):');
  for (const entry of revenueByDay.slice(0, 5)) {
    console.log(`      ${entry.period}: $${entry.revenue} (${entry.orderCount} orders)`);
  }
  console.log('');

  // ============================================
  // 3. Top Products
  // ============================================

  console.log('[3] Top Products...');

  const topProducts = await commerce.analytics.topProducts({
    period: 'all_time',
    limit: 5
  });

  console.log('    Top selling products:');
  for (const product of topProducts) {
    console.log(`      ${product.name} (${product.sku})`);
    console.log(`        Units Sold: ${product.unitsSold}, Revenue: $${product.revenue}`);
  }
  console.log('');

  // ============================================
  // 4. Product Performance
  // ============================================

  console.log('[4] Product Performance...');

  const productPerf = await commerce.analytics.productPerformance({
    period: 'all_time'
  });

  console.log('    Product performance comparison:');
  for (const product of productPerf.slice(0, 3)) {
    console.log(`      ${product.name}:`);
    console.log(`        Current: ${product.unitsSold} units, $${product.revenue} revenue`);
    console.log(`        Previous: ${product.previousUnitsSold} units, $${product.previousRevenue} revenue`);
    console.log(`        Growth: ${product.revenueGrowthPercent.toFixed(1)}%`);
  }
  console.log('');

  // ============================================
  // 5. Customer Metrics
  // ============================================

  console.log('[5] Customer Metrics...');

  const customerMetrics = await commerce.analytics.customerMetrics({
    period: 'all_time'
  });

  console.log('    Customer metrics:');
  console.log(`      Total Customers: ${customerMetrics.totalCustomers}`);
  console.log(`      New Customers: ${customerMetrics.newCustomers}`);
  console.log(`      Returning Customers: ${customerMetrics.returningCustomers}`);
  console.log(`      Avg Lifetime Value: $${customerMetrics.averageLifetimeValue.toFixed(2)}`);
  console.log(`      Avg Orders per Customer: ${customerMetrics.averageOrdersPerCustomer.toFixed(1)}\n`);

  // ============================================
  // 6. Top Customers
  // ============================================

  console.log('[6] Top Customers...');

  const topCustomers = await commerce.analytics.topCustomers({
    period: 'all_time',
    limit: 3
  });

  console.log('    Top customers by spend:');
  for (const customer of topCustomers) {
    console.log(`      ${customer.name} (${customer.email})`);
    console.log(`        Total Spent: $${customer.totalSpent}, Orders: ${customer.orderCount}`);
    console.log(`        Avg Order Value: $${customer.averageOrderValue.toFixed(2)}`);
  }
  console.log('');

  // ============================================
  // 7. Inventory Health
  // ============================================

  console.log('[7] Inventory Health...');

  const invHealth = await commerce.analytics.inventoryHealth();

  console.log('    Inventory health summary:');
  console.log(`      Total SKUs: ${invHealth.totalSkus}`);
  console.log(`      In Stock: ${invHealth.inStockSkus}`);
  console.log(`      Low Stock: ${invHealth.lowStockSkus}`);
  console.log(`      Out of Stock: ${invHealth.outOfStockSkus}`);
  console.log(`      Total Value: $${invHealth.totalValue.toFixed(2)}\n`);

  // ============================================
  // 8. Low Stock Items
  // ============================================

  console.log('[8] Low Stock Items...');

  const lowStockItems = await commerce.analytics.lowStockItems(20); // Threshold of 20

  console.log('    Items below reorder point:');
  for (const item of lowStockItems.slice(0, 5)) {
    console.log(`      ${item.name} (${item.sku})`);
    console.log(`        On Hand: ${item.onHand}, Available: ${item.available}`);
    if (item.reorderPoint) {
      console.log(`        Reorder Point: ${item.reorderPoint}`);
    }
    if (item.daysOfStock) {
      console.log(`        Days of Stock: ${item.daysOfStock}`);
    }
  }
  console.log('');

  // ============================================
  // 9. Inventory Movement
  // ============================================

  console.log('[9] Inventory Movement...');

  const invMovement = await commerce.analytics.inventoryMovement({
    period: 'last30days'
  });

  console.log('    Inventory movement (last 30 days):');
  for (const item of invMovement.slice(0, 3)) {
    console.log(`      ${item.name} (${item.sku})`);
    console.log(`        Sold: ${item.unitsSold}, Received: ${item.unitsReceived}`);
    console.log(`        Returned: ${item.unitsReturned}, Adjusted: ${item.unitsAdjusted}`);
    console.log(`        Net Change: ${item.netChange}`);
  }
  console.log('');

  // ============================================
  // 10. Demand Forecasting
  // ============================================

  console.log('[10] Demand Forecasting...');

  const demandForecast = await commerce.analytics.demandForecast(
    ['LAPTOP-PRO', 'MOUSE-001', 'TSHIRT-M'], // SKUs to forecast
    30 // Days ahead
  );

  console.log('    30-day demand forecast:');
  for (const forecast of demandForecast) {
    console.log(`      ${forecast.name} (${forecast.sku})`);
    console.log(`        Avg Daily Demand: ${forecast.averageDailyDemand.toFixed(2)}`);
    console.log(`        Forecasted Demand: ${forecast.forecastedDemand.toFixed(0)} units`);
    console.log(`        Confidence: ${(forecast.confidence * 100).toFixed(0)}%`);
    console.log(`        Current Stock: ${forecast.currentStock}`);
    console.log(`        Trend: ${forecast.trend}`);
    if (forecast.daysUntilStockout) {
      console.log(`        Days Until Stockout: ${forecast.daysUntilStockout}`);
    }
    if (forecast.recommendedReorderQty) {
      console.log(`        Recommended Reorder: ${forecast.recommendedReorderQty} units`);
    }
  }
  console.log('');

  // ============================================
  // 11. Revenue Forecasting
  // ============================================

  console.log('[11] Revenue Forecasting...');

  const revenueForecast = await commerce.analytics.revenueForecast(
    3, // Periods ahead
    'month' // Granularity
  );

  console.log('    3-month revenue forecast:');
  for (const forecast of revenueForecast) {
    console.log(`      ${forecast.period}:`);
    console.log(`        Forecasted: $${forecast.forecastedRevenue.toFixed(2)}`);
    console.log(`        Range: $${forecast.lowerBound.toFixed(2)} - $${forecast.upperBound.toFixed(2)}`);
    console.log(`        Confidence: ${(forecast.confidenceLevel * 100).toFixed(0)}%`);
  }
  console.log('');

  // ============================================
  // 12. Order Status Breakdown
  // ============================================

  console.log('[12] Order Status Breakdown...');

  const statusBreakdown = await commerce.analytics.orderStatusBreakdown({
    period: 'all_time'
  });

  console.log('    Order status distribution:');
  console.log(`      Pending: ${statusBreakdown.pending}`);
  console.log(`      Confirmed: ${statusBreakdown.confirmed}`);
  console.log(`      Processing: ${statusBreakdown.processing}`);
  console.log(`      Shipped: ${statusBreakdown.shipped}`);
  console.log(`      Delivered: ${statusBreakdown.delivered}`);
  console.log(`      Cancelled: ${statusBreakdown.cancelled}`);
  console.log(`      Refunded: ${statusBreakdown.refunded}\n`);

  // ============================================
  // 13. Fulfillment Metrics
  // ============================================

  console.log('[13] Fulfillment Metrics...');

  const fulfillment = await commerce.analytics.fulfillmentMetrics({
    period: 'last30days'
  });

  console.log('    Fulfillment performance:');
  console.log(`      Shipped Today: ${fulfillment.shippedToday}`);
  console.log(`      Awaiting Shipment: ${fulfillment.awaitingShipment}`);
  if (fulfillment.avgTimeToShipHours) {
    console.log(`      Avg Time to Ship: ${fulfillment.avgTimeToShipHours.toFixed(1)} hours`);
  }
  if (fulfillment.avgTimeToDeliverHours) {
    console.log(`      Avg Time to Deliver: ${fulfillment.avgTimeToDeliverHours.toFixed(1)} hours`);
  }
  if (fulfillment.onTimeShippingPercent) {
    console.log(`      On-Time Shipping: ${fulfillment.onTimeShippingPercent.toFixed(1)}%`);
  }
  console.log('');

  // ============================================
  // 14. Return Metrics
  // ============================================

  console.log('[14] Return Metrics...');

  const returnMetrics = await commerce.analytics.returnMetrics({
    period: 'all_time'
  });

  console.log('    Return metrics:');
  console.log(`      Total Returns: ${returnMetrics.totalReturns}`);
  console.log(`      Return Rate: ${returnMetrics.returnRatePercent.toFixed(2)}%`);
  console.log(`      Total Refunded: $${returnMetrics.totalRefunded.toFixed(2)}`);

  console.log('\n=== Analytics & Forecasting Example Complete ===');
}

// Helper function to create sample data for analytics
async function createSampleData(commerce) {
  // Create customers
  const customers = [];
  const customerData = [
    { email: 'alice@example.com', firstName: 'Alice', lastName: 'Smith' },
    { email: 'bob@example.com', firstName: 'Bob', lastName: 'Johnson' },
    { email: 'carol@example.com', firstName: 'Carol', lastName: 'Williams' },
    { email: 'david@example.com', firstName: 'David', lastName: 'Brown' }
  ];

  for (const data of customerData) {
    customers.push(await commerce.customers.create(data));
  }

  // Create products
  const laptop = await commerce.products.create({
    name: 'Pro Laptop',
    variants: [{ sku: 'LAPTOP-PRO', name: 'Pro Model', price: 1299.99 }]
  });

  const mouse = await commerce.products.create({
    name: 'Wireless Mouse',
    variants: [{ sku: 'MOUSE-001', name: 'Standard', price: 49.99 }]
  });

  const tshirt = await commerce.products.create({
    name: 'T-Shirt',
    variants: [
      { sku: 'TSHIRT-S', name: 'Small', price: 24.99 },
      { sku: 'TSHIRT-M', name: 'Medium', price: 24.99 },
      { sku: 'TSHIRT-L', name: 'Large', price: 24.99 }
    ]
  });

  // Create inventory
  await commerce.inventory.createItem({ sku: 'LAPTOP-PRO', name: 'Laptop Pro', initialQuantity: 25, reorderPoint: 5 });
  await commerce.inventory.createItem({ sku: 'MOUSE-001', name: 'Mouse', initialQuantity: 100, reorderPoint: 20 });
  await commerce.inventory.createItem({ sku: 'TSHIRT-S', name: 'T-Shirt S', initialQuantity: 8, reorderPoint: 10 }); // Low stock
  await commerce.inventory.createItem({ sku: 'TSHIRT-M', name: 'T-Shirt M', initialQuantity: 50, reorderPoint: 15 });
  await commerce.inventory.createItem({ sku: 'TSHIRT-L', name: 'T-Shirt L', initialQuantity: 40, reorderPoint: 12 });

  // Create orders
  const orders = [];

  // Order 1: Alice - Laptop purchase
  orders.push(await commerce.orders.create({
    customerId: customers[0].id,
    items: [
      { sku: 'LAPTOP-PRO', name: 'Pro Laptop', quantity: 1, unitPrice: 1299.99 },
      { sku: 'MOUSE-001', name: 'Wireless Mouse', quantity: 1, unitPrice: 49.99 }
    ],
    currency: 'USD'
  }));

  // Order 2: Bob - T-shirts
  orders.push(await commerce.orders.create({
    customerId: customers[1].id,
    items: [
      { sku: 'TSHIRT-M', name: 'T-Shirt Medium', quantity: 3, unitPrice: 24.99 },
      { sku: 'TSHIRT-L', name: 'T-Shirt Large', quantity: 2, unitPrice: 24.99 }
    ],
    currency: 'USD'
  }));

  // Order 3: Carol - Mixed order
  orders.push(await commerce.orders.create({
    customerId: customers[2].id,
    items: [
      { sku: 'MOUSE-001', name: 'Wireless Mouse', quantity: 5, unitPrice: 49.99 },
      { sku: 'TSHIRT-S', name: 'T-Shirt Small', quantity: 2, unitPrice: 24.99 }
    ],
    currency: 'USD'
  }));

  // Order 4: David - Laptop
  orders.push(await commerce.orders.create({
    customerId: customers[3].id,
    items: [
      { sku: 'LAPTOP-PRO', name: 'Pro Laptop', quantity: 1, unitPrice: 1299.99 }
    ],
    currency: 'USD'
  }));

  // Order 5: Alice again (returning customer)
  orders.push(await commerce.orders.create({
    customerId: customers[0].id,
    items: [
      { sku: 'MOUSE-001', name: 'Wireless Mouse', quantity: 2, unitPrice: 49.99 }
    ],
    currency: 'USD'
  }));

  // Process some orders
  await commerce.orders.updateStatus(orders[0].id, 'confirmed');
  await commerce.orders.ship(orders[0].id, 'TRACK001');

  await commerce.orders.updateStatus(orders[1].id, 'confirmed');
  await commerce.orders.ship(orders[1].id, 'TRACK002');

  await commerce.orders.updateStatus(orders[2].id, 'confirmed');

  // Adjust inventory
  await commerce.inventory.adjust('LAPTOP-PRO', -2, 'Orders fulfilled');
  await commerce.inventory.adjust('MOUSE-001', -8, 'Orders fulfilled');
  await commerce.inventory.adjust('TSHIRT-M', -3, 'Orders fulfilled');
  await commerce.inventory.adjust('TSHIRT-L', -2, 'Orders fulfilled');
  await commerce.inventory.adjust('TSHIRT-S', -2, 'Orders fulfilled');

  // Create a return
  await commerce.returns.create({
    orderId: orders[1].id,
    reason: 'wrong_size',
    reasonDetails: 'Customer ordered wrong size',
    items: [{ orderItemId: orders[1].items[0].id, quantity: 1 }]
  });
}

main().catch(console.error);
