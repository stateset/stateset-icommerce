'use client';

import { Card, Title, Text, Grid, Metric, AreaChart, BarChart, DonutChart, Badge } from '@tremor/react';
import { ChartBarIcon, ArrowTrendingUpIcon, CurrencyDollarIcon, ShoppingBagIcon, UsersIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { ErrorBoundary } from '@/components/ui/error-boundary';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getDashboardMetrics, getOrderAnalytics, getCustomerAnalytics, getRevenueByPeriod, getTopProducts, getConversionFunnel } from '@/app/actions/commerce';
import type { DashboardMetrics, OrderAnalytics, CustomerAnalytics } from '@/lib/types';
import { formatCurrency, formatCompactNumber, formatPercentage } from '@/lib/utils';

export default function AnalyticsDashboard() {
  const { data: metrics, isLoading: loadingMetrics } = useEmbeddedData<DashboardMetrics>(
    () => getDashboardMetrics(),
    { refreshInterval: 60000 }
  );

  const { data: orderAnalytics, isLoading: loadingOrders } = useEmbeddedData<OrderAnalytics>(
    () => getOrderAnalytics(),
    { refreshInterval: 60000 }
  );

  const { data: customerAnalytics, isLoading: loadingCustomers } = useEmbeddedData<CustomerAnalytics>(
    () => getCustomerAnalytics(),
    { refreshInterval: 60000 }
  );

  const { data: revenueData } = useEmbeddedData(
    () => getRevenueByPeriod({
      startDate: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
      endDate: new Date().toISOString(),
      groupBy: 'day'
    }),
    { refreshInterval: 300000 }
  );

  const { data: topProducts } = useEmbeddedData(
    () => getTopProducts(10),
    { refreshInterval: 300000 }
  );

  const { data: conversionFunnel } = useEmbeddedData(
    () => getConversionFunnel(),
    { refreshInterval: 300000 }
  );

  const isLoading = loadingMetrics || loadingOrders || loadingCustomers;

  if (isLoading) {
    return <LoadingSkeleton type="chart" count={4} />;
  }

  const ordersByStatusData = orderAnalytics?.ordersByStatus ?
    Object.entries(orderAnalytics.ordersByStatus).map(([status, count]) => ({
      status: status.charAt(0).toUpperCase() + status.slice(1),
      count
    })) : [];

  const customerSegmentData = customerAnalytics?.customersBySegment ?
    Object.entries(customerAnalytics.customersBySegment).map(([segment, count]) => ({
      segment: segment.replace('_', ' ').replace(/\b\w/g, l => l.toUpperCase()),
      customers: count
    })) : [];

  return (
    <ErrorBoundary>
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
      >
        <div className="mb-6">
          <div className="flex items-center space-x-2 mb-2">
            <ChartBarIcon className="w-8 h-8 text-indigo-600" />
            <Title className="text-2xl">Analytics</Title>
          </div>
          <Text className="text-gray-600">
            Comprehensive analytics powered by embedded forecasting engine
          </Text>
        </div>

        {/* Key Metrics */}
        <Grid numItems={2} numItemsLg={4} className="gap-4 mb-6">
          <Card decoration="top" decorationColor="emerald">
            <div className="flex items-center justify-between">
              <div>
                <Text>Total Revenue</Text>
                <Metric>{formatCurrency(orderAnalytics?.totalRevenue || 0)}</Metric>
              </div>
              <CurrencyDollarIcon className="w-8 h-8 text-emerald-500" />
            </div>
            <Badge color="emerald" className="mt-2">
              <ArrowTrendingUpIcon className="w-3 h-3 mr-1" />
              {metrics?.gmvChange || 0}% vs last period
            </Badge>
          </Card>

          <Card decoration="top" decorationColor="blue">
            <div className="flex items-center justify-between">
              <div>
                <Text>Total Orders</Text>
                <Metric>{formatCompactNumber(orderAnalytics?.totalOrders || 0)}</Metric>
              </div>
              <ShoppingBagIcon className="w-8 h-8 text-blue-500" />
            </div>
            <Badge color="blue" className="mt-2">
              <ArrowTrendingUpIcon className="w-3 h-3 mr-1" />
              {metrics?.ordersChange || 0}% vs last period
            </Badge>
          </Card>

          <Card decoration="top" decorationColor="indigo">
            <div className="flex items-center justify-between">
              <div>
                <Text>Avg Order Value</Text>
                <Metric>{formatCurrency(orderAnalytics?.averageOrderValue || 0)}</Metric>
              </div>
              <ChartBarIcon className="w-8 h-8 text-indigo-500" />
            </div>
            <Badge color={metrics?.aovChange && metrics.aovChange >= 0 ? 'emerald' : 'red'} className="mt-2">
              <ArrowTrendingUpIcon className="w-3 h-3 mr-1" />
              {metrics?.aovChange || 0}% vs last period
            </Badge>
          </Card>

          <Card decoration="top" decorationColor="purple">
            <div className="flex items-center justify-between">
              <div>
                <Text>Active Customers</Text>
                <Metric>{formatCompactNumber(customerAnalytics?.activeCustomers || 0)}</Metric>
              </div>
              <UsersIcon className="w-8 h-8 text-purple-500" />
            </div>
            <Text className="text-sm text-gray-500 mt-2">
              {customerAnalytics?.newCustomersThisMonth || 0} new this month
            </Text>
          </Card>
        </Grid>

        {/* Revenue Trend */}
        <Grid numItems={1} numItemsLg={2} className="gap-6 mb-6">
          <Card>
            <Title>Revenue Trend (30 Days)</Title>
            <Text className="text-gray-500 mb-4">Daily revenue and orders</Text>
            {revenueData && revenueData.length > 0 ? (
              <AreaChart
                className="h-72"
                data={revenueData}
                index="period"
                categories={['revenue']}
                colors={['emerald']}
                valueFormatter={(value) => formatCurrency(value)}
                showAnimation={true}
              />
            ) : (
              <div className="h-72 flex items-center justify-center">
                <Text className="text-gray-400">No revenue data available</Text>
              </div>
            )}
          </Card>

          <Card>
            <Title>Orders by Status</Title>
            <Text className="text-gray-500 mb-4">Current distribution</Text>
            {ordersByStatusData.length > 0 ? (
              <DonutChart
                className="h-72"
                data={ordersByStatusData}
                category="count"
                index="status"
                colors={['amber', 'blue', 'indigo', 'purple', 'emerald', 'red']}
                showAnimation={true}
              />
            ) : (
              <div className="h-72 flex items-center justify-center">
                <Text className="text-gray-400">No order data available</Text>
              </div>
            )}
          </Card>
        </Grid>

        {/* Top Products and Funnel */}
        <Grid numItems={1} numItemsLg={2} className="gap-6 mb-6">
          <Card>
            <Title>Top Products</Title>
            <Text className="text-gray-500 mb-4">By revenue</Text>
            {topProducts && topProducts.length > 0 ? (
              <BarChart
                className="h-72"
                data={topProducts.map(p => ({ name: p.name, revenue: p.revenue }))}
                index="name"
                categories={['revenue']}
                colors={['indigo']}
                valueFormatter={(value) => formatCurrency(value)}
                showAnimation={true}
                layout="vertical"
              />
            ) : (
              <div className="h-72 flex items-center justify-center">
                <Text className="text-gray-400">No product data available</Text>
              </div>
            )}
          </Card>

          <Card>
            <Title>Customer Segments</Title>
            <Text className="text-gray-500 mb-4">Distribution by health score</Text>
            {customerSegmentData.length > 0 ? (
              <BarChart
                className="h-72"
                data={customerSegmentData}
                index="segment"
                categories={['customers']}
                colors={['purple']}
                showAnimation={true}
              />
            ) : (
              <div className="h-72 flex items-center justify-center">
                <Text className="text-gray-400">No segment data available</Text>
              </div>
            )}
          </Card>
        </Grid>

        {/* Conversion Funnel */}
        <Card>
          <Title>Conversion Funnel</Title>
          <Text className="text-gray-500 mb-4">Customer journey analytics</Text>
          {conversionFunnel && conversionFunnel.length > 0 ? (
            <div className="space-y-4">
              {conversionFunnel.map((stage) => (
                <div key={stage.stage} className="relative">
                  <div className="flex items-center justify-between mb-1">
                    <Text className="font-medium">{stage.stage}</Text>
                    <div className="flex items-center space-x-2">
                      <Text>{formatCompactNumber(stage.count)}</Text>
                      <Badge color={stage.rate > 50 ? 'emerald' : stage.rate > 25 ? 'amber' : 'red'}>
                        {formatPercentage(stage.rate)}
                      </Badge>
                    </div>
                  </div>
                  <div className="h-8 bg-gray-100 dark:bg-gray-800 rounded">
                    <div
                      className="h-full bg-indigo-500 rounded transition-all duration-500"
                      style={{ width: `${stage.rate}%` }}
                    />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="h-48 flex items-center justify-center">
              <Text className="text-gray-400">No funnel data available</Text>
            </div>
          )}
        </Card>
      </motion.div>
    </ErrorBoundary>
  );
}
