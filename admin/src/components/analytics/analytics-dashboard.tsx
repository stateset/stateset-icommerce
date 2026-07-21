'use client';

import { AreaChart, BarChart, DonutChart } from '@tremor/react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  Badge,
  MetricCard,
} from '@stateset/design';
import {
  ChartBarIcon,
  CurrencyDollarIcon,
  ShoppingBagIcon,
  UsersIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { ErrorBoundary } from '@/components/ui/error-boundary';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import {
  getDashboardMetrics,
  getOrderAnalytics,
  getCustomerAnalytics,
  getRevenueByPeriod,
  getTopProducts,
  getConversionFunnel,
} from '@/app/actions/commerce';
import type { DashboardMetrics, OrderAnalytics, CustomerAnalytics } from '@/lib/types';
import { formatCurrency, formatCompactNumber, formatPercentage } from '@/lib/utils';

export default function AnalyticsDashboard() {
  const { data: metrics, isLoading: loadingMetrics } = useEmbeddedData<DashboardMetrics>(
    () => getDashboardMetrics(),
    { refreshInterval: 60000 },
  );

  const { data: orderAnalytics, isLoading: loadingOrders } = useEmbeddedData<OrderAnalytics>(
    () => getOrderAnalytics(),
    { refreshInterval: 60000 },
  );

  const { data: customerAnalytics, isLoading: loadingCustomers } =
    useEmbeddedData<CustomerAnalytics>(() => getCustomerAnalytics(), { refreshInterval: 60000 });

  const { data: revenueData } = useEmbeddedData(
    () =>
      getRevenueByPeriod({
        startDate: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
        endDate: new Date().toISOString(),
        groupBy: 'day',
      }),
    { refreshInterval: 300000 },
  );

  const { data: topProducts } = useEmbeddedData(() => getTopProducts(10), {
    refreshInterval: 300000,
  });

  const { data: conversionFunnel } = useEmbeddedData(() => getConversionFunnel(), {
    refreshInterval: 300000,
  });

  const isLoading = loadingMetrics || loadingOrders || loadingCustomers;

  if (isLoading) {
    return <LoadingSkeleton type="chart" count={4} />;
  }

  const ordersByStatusData = orderAnalytics?.ordersByStatus
    ? Object.entries(orderAnalytics.ordersByStatus).map(([status, count]) => ({
        status: status.charAt(0).toUpperCase() + status.slice(1),
        count,
      }))
    : [];

  const customerSegmentData = customerAnalytics?.customersBySegment
    ? Object.entries(customerAnalytics.customersBySegment).map(([segment, count]) => ({
        segment: segment.replace('_', ' ').replace(/\b\w/g, (l) => l.toUpperCase()),
        customers: count,
      }))
    : [];

  return (
    <ErrorBoundary>
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
      >
        <div className="mb-6">
          <div className="flex items-center space-x-2 mb-2">
            <ChartBarIcon className="w-8 h-8 text-ds-primary" />
            <h2 className="font-ds-display text-2xl font-semibold text-ds-foreground">Analytics</h2>
          </div>
          <p className="text-sm text-ds-muted-foreground">
            Comprehensive analytics powered by embedded forecasting engine
          </p>
        </div>

        {/* Key Metrics */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
          <MetricCard
            label="Total Revenue"
            value={formatCurrency(orderAnalytics?.totalRevenue || 0)}
            tone="success"
            icon={CurrencyDollarIcon}
            subtitle={`${metrics?.gmvChange || 0}% vs last period`}
          />

          <MetricCard
            label="Total Orders"
            value={formatCompactNumber(orderAnalytics?.totalOrders || 0)}
            tone="primary"
            icon={ShoppingBagIcon}
            subtitle={`${metrics?.ordersChange || 0}% vs last period`}
          />

          <MetricCard
            label="Avg Order Value"
            value={formatCurrency(orderAnalytics?.averageOrderValue || 0)}
            tone="primary"
            icon={ChartBarIcon}
            subtitle={`${metrics?.aovChange || 0}% vs last period`}
          />

          <MetricCard
            label="Active Customers"
            value={formatCompactNumber(customerAnalytics?.activeCustomers || 0)}
            tone="accent"
            icon={UsersIcon}
            subtitle={`${customerAnalytics?.newCustomersThisMonth || 0} new this month`}
          />
        </div>

        {/* Revenue Trend */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <Card>
            <CardHeader>
              <CardTitle>Revenue Trend (30 Days)</CardTitle>
              <CardDescription>Daily revenue and orders</CardDescription>
            </CardHeader>
            <CardContent>
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
                  <p className="text-sm text-ds-muted-foreground">No revenue data available</p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Orders by Status</CardTitle>
              <CardDescription>Current distribution</CardDescription>
            </CardHeader>
            <CardContent>
              {ordersByStatusData.length > 0 ? (
                <DonutChart
                  className="h-72"
                  data={ordersByStatusData}
                  category="count"
                  index="status"
                  colors={['indigo', 'emerald', 'violet', 'amber', 'cyan']}
                  showAnimation={true}
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <p className="text-sm text-ds-muted-foreground">No order data available</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Top Products and Funnel */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <Card>
            <CardHeader>
              <CardTitle>Top Products</CardTitle>
              <CardDescription>By revenue</CardDescription>
            </CardHeader>
            <CardContent>
              {topProducts && topProducts.length > 0 ? (
                <BarChart
                  className="h-72"
                  data={topProducts.map((p) => ({ name: p.name, revenue: p.revenue }))}
                  index="name"
                  categories={['revenue']}
                  colors={['indigo']}
                  valueFormatter={(value) => formatCurrency(value)}
                  showAnimation={true}
                  layout="vertical"
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <p className="text-sm text-ds-muted-foreground">No product data available</p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Customer Segments</CardTitle>
              <CardDescription>Distribution by health score</CardDescription>
            </CardHeader>
            <CardContent>
              {customerSegmentData.length > 0 ? (
                <BarChart
                  className="h-72"
                  data={customerSegmentData}
                  index="segment"
                  categories={['customers']}
                  colors={['violet']}
                  showAnimation={true}
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <p className="text-sm text-ds-muted-foreground">No segment data available</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Conversion Funnel */}
        <Card>
          <CardHeader>
            <CardTitle>Conversion Funnel</CardTitle>
            <CardDescription>Customer journey analytics</CardDescription>
          </CardHeader>
          <CardContent>
            {conversionFunnel && conversionFunnel.length > 0 ? (
              <div className="space-y-4">
                {conversionFunnel.map((stage) => (
                  <div key={stage.stage} className="relative">
                    <div className="flex items-center justify-between mb-1">
                      <p className="text-sm font-medium text-ds-foreground">{stage.stage}</p>
                      <div className="flex items-center space-x-2">
                        <p className="text-sm text-ds-muted-foreground">
                          {formatCompactNumber(stage.count)}
                        </p>
                        <Badge
                          variant={
                            stage.rate > 50 ? 'success' : stage.rate > 25 ? 'warning' : 'danger'
                          }
                        >
                          {formatPercentage(stage.rate)}
                        </Badge>
                      </div>
                    </div>
                    <div className="h-8 bg-ds-muted rounded">
                      <div
                        className="h-full bg-ds-primary rounded transition-all duration-500"
                        style={{ width: `${stage.rate}%` }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="h-48 flex items-center justify-center">
                <p className="text-sm text-ds-muted-foreground">No funnel data available</p>
              </div>
            )}
          </CardContent>
        </Card>
      </motion.div>
    </ErrorBoundary>
  );
}
