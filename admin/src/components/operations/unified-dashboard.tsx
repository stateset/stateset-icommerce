'use client';

import { Card, Title, Text, Badge, Grid, Col, Metric, AreaChart, ProgressBar } from '@tremor/react';
import { ChartPieIcon, ArrowTrendingUpIcon, ArrowTrendingDownIcon, SparklesIcon, ExclamationTriangleIcon, CheckCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useState, useEffect, memo } from 'react';
import { ErrorBoundary } from '../ui/error-boundary';
import LoadingSkeleton from '../ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getDashboardMetrics, getHourlyActivity, getSystemHealth } from '@/app/actions/commerce';
import type { DashboardMetrics, HourlyActivity, SystemHealth } from '@/lib/types';
import { formatCurrency, formatCompactNumber, formatPercentage, formatRelativeTime, getTrendColor } from '@/lib/utils';

// Types
interface KPI {
  name: string;
  value: number;
  change: number;
  trend: 'up' | 'down' | 'stable';
  unit?: string;
  target?: number;
}

interface Alert {
  id: string;
  severity: 'critical' | 'high' | 'medium' | 'low';
  message: string;
  time: string;
}

interface AIInsight {
  insight: string;
  impact: 'positive' | 'negative' | 'warning';
  action: string;
}

interface DataCoverage {
  name: string;
  status: 'connected' | 'unavailable';
  detail: string;
  coverage: number;
}

// Animation variants
const containerVariants = {
  exit: { opacity: 0, transition: { duration: 0.5, ease: [0.175, 0.85, 0.42, 0.96] } },
  enter: {
    opacity: 1,
    transition: { delay: 0.1, duration: 0.5, ease: [0.175, 0.85, 0.42, 0.96] }
  }
};

const getTrendIcon = (trend: string) => {
  return trend === 'up' ? ArrowTrendingUpIcon : trend === 'down' ? ArrowTrendingDownIcon : null;
};

const formatValue = (value: number, unit?: string): string => {
  if (unit === '$') return formatCurrency(value);
  if (unit === '%') return formatPercentage(value);
  return formatCompactNumber(value);
};

function buildDataCoverage(
  metrics: DashboardMetrics | null,
  hourlyActivity: HourlyActivity[] | null,
  systemHealth: SystemHealth | null,
): DataCoverage[] {
  return [
    {
      name: 'Dashboard Metrics',
      status: metrics ? 'connected' : 'unavailable',
      detail: metrics
        ? `${formatCompactNumber(metrics.ordersToday)} orders tracked today`
        : 'No dashboard metrics response',
      coverage: metrics ? 100 : 0,
    },
    {
      name: 'Hourly Activity',
      status: hourlyActivity ? 'connected' : 'unavailable',
      detail: hourlyActivity
        ? hourlyActivity.length > 0
          ? `${hourlyActivity.length} hourly buckets loaded`
          : 'Feed returned no hourly buckets for the current window'
        : 'No hourly activity response',
      coverage: hourlyActivity ? 100 : 0,
    },
    {
      name: 'System Health',
      status: systemHealth ? 'connected' : 'unavailable',
      detail: systemHealth
        ? `Latency ${systemHealth.databaseLatency}ms, error rate ${systemHealth.errorRate}%`
        : 'No system health snapshot',
      coverage: systemHealth ? 100 : 0,
    },
  ];
}

function buildOperationalAlerts(
  metrics: DashboardMetrics | null,
  systemHealth: SystemHealth | null,
): Alert[] {
  const alerts: Alert[] = [];

  if (systemHealth) {
    if (systemHealth.errorRate >= 1) {
      alerts.push({
        id: 'error-rate',
        severity: systemHealth.errorRate >= 2 ? 'critical' : 'high',
        message: `Error rate is ${systemHealth.errorRate}%`,
        time: 'Live snapshot',
      });
    }

    if (systemHealth.databaseLatency >= 100) {
      alerts.push({
        id: 'database-latency',
        severity: 'high',
        message: `Database latency is ${systemHealth.databaseLatency}ms`,
        time: 'Live snapshot',
      });
    }

    if (systemHealth.queueDepth >= 50) {
      alerts.push({
        id: 'queue-depth',
        severity: systemHealth.queueDepth >= 100 ? 'high' : 'medium',
        message: `Queue depth is ${systemHealth.queueDepth}`,
        time: 'Live snapshot',
      });
    }
  }

  if (metrics && metrics.inventoryHealth < 90) {
    alerts.push({
      id: 'inventory-health',
      severity: metrics.inventoryHealth < 80 ? 'high' : 'medium',
      message: `Inventory health is ${metrics.inventoryHealth}%`,
      time: 'Live snapshot',
    });
  }

  return alerts;
}

function buildOperationalInsights(
  metrics: DashboardMetrics | null,
  systemHealth: SystemHealth | null,
): AIInsight[] {
  const insights: AIInsight[] = [];

  if (metrics) {
    insights.push({
      insight:
        metrics.gmvChange >= 0
          ? `GMV is up ${Math.abs(metrics.gmvChange)}% today`
          : `GMV is down ${Math.abs(metrics.gmvChange)}% today`,
      impact: metrics.gmvChange >= 0 ? 'positive' : 'negative',
      action:
        metrics.gmvChange >= 0
          ? 'Verify fulfillment capacity against the current order pace'
          : 'Review channel mix and order acquisition sources',
    });

    insights.push({
      insight:
        metrics.conversionRate >= 3.5
          ? `Conversion is above target at ${metrics.conversionRate}%`
          : `Conversion is below target at ${metrics.conversionRate}%`,
      impact: metrics.conversionRate >= 3.5 ? 'positive' : 'warning',
      action:
        metrics.conversionRate >= 3.5
          ? 'Maintain current storefront and checkout configuration'
          : 'Inspect checkout friction and merchandising changes',
    });
  }

  if (systemHealth) {
    insights.push({
      insight:
        systemHealth.processingSpeed >= 95
          ? `Processing speed is healthy at ${systemHealth.processingSpeed}%`
          : `Processing speed is degraded at ${systemHealth.processingSpeed}%`,
      impact: systemHealth.processingSpeed >= 95 ? 'positive' : 'warning',
      action:
        systemHealth.processingSpeed >= 95
          ? 'Keep current capacity allocation'
          : 'Investigate queue pressure and database contention',
    });
  }

  return insights;
}

function UnifiedDashboardInner() {
  const [isLoading, setIsLoading] = useState(true);
  const [lastUpdated, setLastUpdated] = useState(new Date());

  // Fetch real-time data from embedded backend
  const { data: metrics } = useEmbeddedData<DashboardMetrics>(
    () => getDashboardMetrics(),
    { refreshInterval: 30000 }
  );

  const { data: hourlyActivity } = useEmbeddedData<HourlyActivity[]>(
    () => getHourlyActivity(),
    { refreshInterval: 60000 }
  );

  const { data: systemHealth } = useEmbeddedData<SystemHealth>(
    () => getSystemHealth(),
    { refreshInterval: 10000 }
  );

  // Convert metrics to KPIs for display
  const keyMetrics: KPI[] = metrics ? [
    { name: 'GMV Today', value: metrics.gmvToday, change: metrics.gmvChange, trend: metrics.gmvChange >= 0 ? 'up' : 'down', unit: '$' },
    { name: 'Orders Processed', value: metrics.ordersToday, change: metrics.ordersChange, trend: metrics.ordersChange >= 0 ? 'up' : 'down' },
    { name: 'Avg Order Value', value: metrics.averageOrderValue, change: metrics.aovChange, trend: metrics.aovChange >= 0 ? 'up' : 'down', unit: '$' },
    { name: 'Conversion Rate', value: metrics.conversionRate, change: metrics.conversionChange, trend: metrics.conversionChange >= 0 ? 'up' : 'down', unit: '%', target: 3.5 },
    { name: 'Active Customers', value: metrics.activeCustomers, change: 0, trend: 'stable' },
    { name: 'Inventory Health', value: metrics.inventoryHealth, change: 0, trend: 'stable', unit: '%' }
  ] : [];

  const dataCoverage = buildDataCoverage(metrics || null, hourlyActivity || null, systemHealth || null);
  const criticalAlerts = buildOperationalAlerts(metrics || null, systemHealth || null);
  const aiInsights = buildOperationalInsights(metrics || null, systemHealth || null);

  useEffect(() => {
    const timer = setTimeout(() => setIsLoading(false), 1000);
    return () => clearTimeout(timer);
  }, []);

  // Update timestamp on data refresh
  useEffect(() => {
    if (metrics) {
      setLastUpdated(new Date());
    }
  }, [metrics]);

  const totalRevenue = hourlyActivity?.reduce((sum, hour) => sum + hour.revenue, 0) || 0;
  const totalOrders = hourlyActivity?.reduce((sum, hour) => sum + hour.orders, 0) || 0;

  if (isLoading) {
    return <LoadingSkeleton type="chart" count={3} />;
  }

  return (
    <ErrorBoundary>
      <motion.div initial="exit" animate="enter" exit="exit" variants={containerVariants}>
        <div className="mb-6">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center space-x-2 mb-2">
                <ChartPieIcon className="w-8 h-8 text-indigo-600" />
                <Title className="text-2xl">Executive Operations Dashboard</Title>
              </div>
              <Text className="text-gray-600">
                Real-time unified view powered by embedded commerce engine
              </Text>
            </div>
            <div className="text-right">
              <Text className="text-sm text-gray-500">Last updated</Text>
              <Text className="font-medium">{formatRelativeTime(lastUpdated)}</Text>
            </div>
          </div>
        </div>

        {/* Key Metrics Grid */}
        <Grid numItems={2} numItemsSm={3} numItemsLg={6} className="gap-4 mb-6">
          {keyMetrics.map((metric) => {
            const TrendIcon = metric.trend ? getTrendIcon(metric.trend) : null;
            const isPositiveMetric = metric.name !== 'Avg Order Value' || metric.trend === 'up';

            return (
              <Card key={metric.name} decoration="top" decorationColor="indigo">
                <Text>{metric.name}</Text>
                <div className="flex items-baseline space-x-2">
                  <Metric>
                    {formatValue(metric.value, metric.unit)}
                  </Metric>
                  {TrendIcon && metric.trend && metric.change !== undefined && (
                    <div className={`flex items-center space-x-1 text-sm ${getTrendColor(metric.trend, isPositiveMetric)}`}>
                      <TrendIcon className="w-4 h-4" />
                      <span>{Math.abs(metric.change)}%</span>
                    </div>
                  )}
                </div>
                {metric.target && typeof metric.value === 'number' && (
                  <div className="mt-2">
                    <div className="flex justify-between text-xs mb-1">
                      <Text>Target: {formatValue(metric.target, metric.unit)}</Text>
                      <Text>{((metric.value / metric.target) * 100).toFixed(1)}%</Text>
                    </div>
                    <ProgressBar
                      value={(metric.value / metric.target) * 100}
                      color={(metric.value >= metric.target) ? 'emerald' : 'amber'}
                    />
                  </div>
                )}
              </Card>
            );
          })}
        </Grid>

        <Grid numItems={1} numItemsLg={3} className="gap-6 mb-6">
          {/* Hourly Performance */}
          <Col numColSpan={1} numColSpanLg={2}>
            <Card>
              <Title>Today's Performance</Title>
              <Text className="text-gray-500 mb-4">Orders and revenue by hour</Text>

              {hourlyActivity && hourlyActivity.length > 0 ? (
                <>
                  <AreaChart
                    className="h-72"
                    data={hourlyActivity}
                    index="hour"
                    categories={['orders', 'revenue']}
                    colors={['indigo', 'emerald']}
                    valueFormatter={(value) => {
                      if (value > 1000) return `$${(value / 1000).toFixed(0)}k`;
                      return value.toString();
                    }}
                    showAnimation={true}
                  />

                  <div className="mt-4 grid grid-cols-3 gap-4 text-center">
                    <div>
                      <Text className="text-sm text-gray-500">Total Orders</Text>
                      <Text className="font-medium text-lg">{totalOrders}</Text>
                    </div>
                    <div>
                      <Text className="text-sm text-gray-500">Total Revenue</Text>
                      <Text className="font-medium text-lg">${(totalRevenue / 1000).toFixed(1)}k</Text>
                    </div>
                    <div>
                      <Text className="text-sm text-gray-500">Avg Order Value</Text>
                      <Text className="font-medium text-lg">${totalOrders > 0 ? (totalRevenue / totalOrders).toFixed(2) : '0.00'}</Text>
                    </div>
                  </div>
                </>
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <Text className="text-gray-400">No data available</Text>
                </div>
              )}
            </Card>
          </Col>

          {/* Live Data Coverage */}
          <Card>
            <Title>Live Data Coverage</Title>
            <Text className="text-gray-500 mb-4">Current telemetry available to the dashboard</Text>
            <div className="space-y-3">
              {dataCoverage.map((feed) => (
                <div key={feed.name}>
                  <div className="flex justify-between items-center mb-1">
                    <Text className="text-sm font-medium">{feed.name}</Text>
                    <div className="flex items-center space-x-2">
                      <Badge color={feed.status === 'connected' ? 'emerald' : 'amber'} size="xs">
                        {feed.status}
                      </Badge>
                      <Text className="text-sm">{feed.coverage}%</Text>
                    </div>
                  </div>
                  <Text className="text-xs text-gray-500 mb-2">{feed.detail}</Text>
                  <ProgressBar
                    value={feed.coverage}
                    color={feed.status === 'connected' ? 'emerald' : 'amber'}
                  />
                </div>
              ))}
            </div>

            <div className="mt-4 p-3 bg-indigo-50 dark:bg-indigo-900/20 rounded">
              <div className="flex items-center space-x-2">
                <SparklesIcon className="w-5 h-5 text-indigo-600" />
                <Text className="text-sm font-medium">Embedded Engine Active</Text>
              </div>
              <Text className="text-xs text-gray-600 dark:text-gray-400 mt-1">
                All operations running on local embedded database
              </Text>
            </div>
          </Card>
        </Grid>

        <Grid numItems={1} numItemsLg={2} className="gap-6 mb-6">
          {/* Critical Alerts */}
          <Card>
            <div className="flex items-center justify-between mb-4">
              <Title>Critical Alerts</Title>
              <Badge color="red" icon={ExclamationTriangleIcon}>
                {criticalAlerts.length} active
              </Badge>
            </div>

            {criticalAlerts.length > 0 ? (
              <div className="space-y-2">
                {criticalAlerts.map((alert) => (
                  <motion.div
                    key={alert.id}
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    className="flex items-start justify-between p-3 border rounded-lg dark:border-gray-700"
                  >
                    <div className="flex items-start space-x-3">
                      <div className={`w-2 h-2 rounded-full mt-1.5 ${
                        alert.severity === 'critical' ? 'bg-red-500' :
                        alert.severity === 'high' ? 'bg-orange-500' : 'bg-yellow-500'
                      }`} />
                      <div>
                        <Text className="text-sm font-medium">{alert.message}</Text>
                        <Text className="text-xs text-gray-500">{alert.time}</Text>
                      </div>
                    </div>
                    <Badge color={
                      alert.severity === 'critical' ? 'red' :
                      alert.severity === 'high' ? 'orange' : 'yellow'
                    } size="xs">
                      {alert.severity}
                    </Badge>
                  </motion.div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
                <Text className="text-gray-500">No active operational alerts in the current snapshot.</Text>
              </div>
            )}
          </Card>

          {/* Operational Insights */}
          <Card>
            <div className="flex items-center justify-between mb-4">
              <Title>Operational Insights</Title>
              <Badge color="purple" icon={SparklesIcon}>
                Derived Signals
              </Badge>
            </div>

            {aiInsights.length > 0 ? (
              <div className="space-y-3">
                {aiInsights.map((insight, index) => (
                  <motion.div
                    key={index}
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: index * 0.1 }}
                    className="border rounded-lg p-3 dark:border-gray-700"
                  >
                    <div className="flex items-start space-x-2 mb-2">
                      {insight.impact === 'positive' && <CheckCircleIcon className="w-5 h-5 text-emerald-500 flex-shrink-0" />}
                      {insight.impact === 'negative' && <ExclamationTriangleIcon className="w-5 h-5 text-red-500 flex-shrink-0" />}
                      {insight.impact === 'warning' && <ExclamationTriangleIcon className="w-5 h-5 text-amber-500 flex-shrink-0" />}
                      <Text className="text-sm">{insight.insight}</Text>
                    </div>
                    <div className="ml-7">
                      <Badge color="indigo" size="xs">
                        Suggested: {insight.action}
                      </Badge>
                    </div>
                  </motion.div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
                <Text className="text-gray-500">
                  No live operational insights are available until metrics feeds are connected.
                </Text>
              </div>
            )}
          </Card>
        </Grid>

        {/* System Health */}
        <Card>
          <Title>System Health Monitor</Title>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-4 mt-4">
            <div className="text-center">
              <Text className="text-sm text-gray-500">Database Latency</Text>
              <Metric className="text-lg">{systemHealth?.databaseLatency || 0}ms</Metric>
              <Badge color={(systemHealth?.databaseLatency || 0) < 100 ? 'emerald' : 'amber'} size="xs">
                {(systemHealth?.databaseLatency || 0) < 100 ? 'Optimal' : 'Slow'}
              </Badge>
            </div>
            <div className="text-center">
              <Text className="text-sm text-gray-500">Error Rate</Text>
              <Metric className="text-lg">{systemHealth?.errorRate || 0}%</Metric>
              <Badge color={(systemHealth?.errorRate || 0) < 0.1 ? 'emerald' : 'red'} size="xs">
                {(systemHealth?.errorRate || 0) < 0.1 ? 'Healthy' : 'High'}
              </Badge>
            </div>
            <div className="text-center">
              <Text className="text-sm text-gray-500">Active Connections</Text>
              <Metric className="text-lg">{systemHealth?.activeConnections || 0}</Metric>
              <Badge color="blue" size="xs">
                Running
              </Badge>
            </div>
            <div className="text-center">
              <Text className="text-sm text-gray-500">Queue Depth</Text>
              <Metric className="text-lg">{systemHealth?.queueDepth || 0}</Metric>
              <Badge color={(systemHealth?.queueDepth || 0) < 50 ? 'emerald' : 'amber'} size="xs">
                {(systemHealth?.queueDepth || 0) < 50 ? 'Normal' : 'Elevated'}
              </Badge>
            </div>
            <div className="text-center">
              <Text className="text-sm text-gray-500">Processing Speed</Text>
              <Metric className="text-lg">{systemHealth?.processingSpeed || 0}%</Metric>
              <Badge color={(systemHealth?.processingSpeed || 0) > 95 ? 'emerald' : 'amber'} size="xs">
                {(systemHealth?.processingSpeed || 0) > 95 ? 'Fast' : 'Degraded'}
              </Badge>
            </div>
          </div>
        </Card>
      </motion.div>
    </ErrorBoundary>
  );
}

const UnifiedDashboard = memo(UnifiedDashboardInner);
export default UnifiedDashboard;
