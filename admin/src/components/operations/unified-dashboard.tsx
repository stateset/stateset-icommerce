'use client';

import { AreaChart } from '@tremor/react';
import {
  CurrencyDollarIcon,
  ShoppingCartIcon,
  ReceiptPercentIcon,
  ArrowTrendingUpIcon,
  UsersIcon,
  CubeIcon,
  SparklesIcon,
} from '@heroicons/react/24/outline';
import { useState, useEffect, memo } from 'react';
import {
  DashboardSectionHeader,
  MetricCard,
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  StatusPill,
  EmptyState,
  Reveal,
  type MetricTone,
  type StatusTone,
} from '@stateset/design';
import { ErrorBoundary } from '../ui/error-boundary';
import LoadingSkeleton from '../ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getDashboardMetrics, getHourlyActivity, getSystemHealth } from '@/app/actions/commerce';
import type { DashboardMetrics, HourlyActivity, SystemHealth } from '@/lib/types';
import { formatCurrency, formatCompactNumber, formatPercentage, formatRelativeTime } from '@/lib/utils';

// ─── Types ──────────────────────────────────────────────────────────────────
interface KPI {
  name: string;
  value: number;
  change: number;
  unit?: '$' | '%';
  target?: number;
  icon: React.ComponentType<{ className?: string }>;
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

// ─── Formatting / mapping helpers ─────────────────────────────────────────────
function formatValue(value: number, unit?: '$' | '%'): string {
  if (unit === '$') return formatCurrency(value);
  if (unit === '%') return formatPercentage(value);
  return formatCompactNumber(value);
}

/** A signed change becomes a brand tone + an arrowed trend caption. */
function changeTone(change: number): MetricTone {
  if (change > 0) return 'success';
  if (change < 0) return 'danger';
  return 'primary';
}

function trendCaption(change: number): string {
  if (change === 0) return 'Flat vs. yesterday';
  const arrow = change > 0 ? '▲' : '▼';
  return `${arrow} ${Math.abs(change)}% vs. yesterday`;
}

const SEVERITY_STATUS: Record<Alert['severity'], StatusTone> = {
  critical: 'fail',
  high: 'fail',
  medium: 'warn',
  low: 'review',
};

const IMPACT_STATUS: Record<AIInsight['impact'], StatusTone> = {
  positive: 'ok',
  negative: 'fail',
  warning: 'warn',
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

// ─── Small presentational helpers ─────────────────────────────────────────────
function CoverageBar({ value, ok }: { value: number; ok: boolean }) {
  return (
    <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-ds-muted">
      <div
        className={ok ? 'h-full rounded-full bg-ds-primary' : 'h-full rounded-full bg-ds-status-warn'}
        style={{ width: `${value}%` }}
      />
    </div>
  );
}

function UnifiedDashboardInner() {
  const [isLoading, setIsLoading] = useState(true);
  const [lastUpdated, setLastUpdated] = useState(new Date());

  // Real-time data from the embedded backend.
  const { data: metrics } = useEmbeddedData<DashboardMetrics>(() => getDashboardMetrics(), {
    refreshInterval: 30000,
  });
  const { data: hourlyActivity } = useEmbeddedData<HourlyActivity[]>(() => getHourlyActivity(), {
    refreshInterval: 60000,
  });
  const { data: systemHealth } = useEmbeddedData<SystemHealth>(() => getSystemHealth(), {
    refreshInterval: 10000,
  });

  const keyMetrics: KPI[] = metrics
    ? [
        { name: 'GMV Today', value: metrics.gmvToday, change: metrics.gmvChange, unit: '$', icon: CurrencyDollarIcon },
        { name: 'Orders Processed', value: metrics.ordersToday, change: metrics.ordersChange, icon: ShoppingCartIcon },
        { name: 'Avg Order Value', value: metrics.averageOrderValue, change: metrics.aovChange, unit: '$', icon: ReceiptPercentIcon },
        { name: 'Conversion Rate', value: metrics.conversionRate, change: metrics.conversionChange, unit: '%', target: 3.5, icon: ArrowTrendingUpIcon },
        { name: 'Active Customers', value: metrics.activeCustomers, change: 0, icon: UsersIcon },
        { name: 'Inventory Health', value: metrics.inventoryHealth, change: 0, unit: '%', icon: CubeIcon },
      ]
    : [];

  const dataCoverage = buildDataCoverage(metrics || null, hourlyActivity || null, systemHealth || null);
  const criticalAlerts = buildOperationalAlerts(metrics || null, systemHealth || null);
  const aiInsights = buildOperationalInsights(metrics || null, systemHealth || null);

  useEffect(() => {
    const timer = setTimeout(() => setIsLoading(false), 1000);
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    if (metrics) setLastUpdated(new Date());
  }, [metrics]);

  const totalRevenue = hourlyActivity?.reduce((sum, hour) => sum + hour.revenue, 0) || 0;
  const totalOrders = hourlyActivity?.reduce((sum, hour) => sum + hour.orders, 0) || 0;

  if (isLoading) {
    return <LoadingSkeleton type="chart" count={3} />;
  }

  return (
    <ErrorBoundary>
      <div className="space-y-6">
        <DashboardSectionHeader
          eyebrow="Intelligent Commerce"
          title="Executive Operations"
          description="Real-time unified view powered by the embedded commerce engine."
          actions={
            <div className="text-right">
              <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-ds-muted-foreground">
                Last updated
              </p>
              <p className="ds-instrument-number mt-1 text-sm text-ds-foreground">
                {formatRelativeTime(lastUpdated)}
              </p>
            </div>
          }
        />

        {/* Key metrics */}
        <Reveal className="grid grid-cols-2 gap-4 lg:grid-cols-3 xl:grid-cols-6">
          {keyMetrics.map((metric) => (
            <MetricCard
              key={metric.name}
              label={metric.name}
              value={formatValue(metric.value, metric.unit)}
              icon={metric.icon}
              tone={changeTone(metric.change)}
              trend={metric.change !== 0 ? trendCaption(metric.change) : ''}
              subtitle={
                metric.target
                  ? `Target ${formatValue(metric.target, metric.unit)} · ${((metric.value / metric.target) * 100).toFixed(0)}% attained`
                  : ''
              }
            />
          ))}
        </Reveal>

        {/* Performance + live coverage */}
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>Today&apos;s Performance</CardTitle>
              <CardDescription>Orders and revenue by hour</CardDescription>
            </CardHeader>
            <CardContent>
              {hourlyActivity && hourlyActivity.length > 0 ? (
                <>
                  <AreaChart
                    className="h-72"
                    data={hourlyActivity}
                    index="hour"
                    categories={['orders', 'revenue']}
                    colors={['indigo', 'emerald']}
                    valueFormatter={(value) => (value > 1000 ? `$${(value / 1000).toFixed(0)}k` : value.toString())}
                    showAnimation
                  />
                  <div className="mt-4 grid grid-cols-3 gap-4 border-t border-ds-enterprise-line/70 pt-4 text-center">
                    <div>
                      <p className="text-xs text-ds-muted-foreground">Total Orders</p>
                      <p className="ds-instrument-number mt-1 text-lg text-ds-foreground">{totalOrders}</p>
                    </div>
                    <div>
                      <p className="text-xs text-ds-muted-foreground">Total Revenue</p>
                      <p className="ds-instrument-number mt-1 text-lg text-ds-foreground">
                        ${(totalRevenue / 1000).toFixed(1)}k
                      </p>
                    </div>
                    <div>
                      <p className="text-xs text-ds-muted-foreground">Avg Order Value</p>
                      <p className="ds-instrument-number mt-1 text-lg text-ds-foreground">
                        ${totalOrders > 0 ? (totalRevenue / totalOrders).toFixed(2) : '0.00'}
                      </p>
                    </div>
                  </div>
                </>
              ) : (
                <div className="flex h-72 items-center justify-center">
                  <p className="text-sm text-ds-muted-foreground">No data available</p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Live Data Coverage</CardTitle>
              <CardDescription>Current telemetry available to the dashboard</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {dataCoverage.map((feed) => {
                const ok = feed.status === 'connected';
                return (
                  <div key={feed.name}>
                    <div className="mb-1 flex items-center justify-between gap-2">
                      <p className="text-sm font-medium text-ds-foreground">{feed.name}</p>
                      <StatusPill status={ok ? 'ok' : 'warn'}>{ok ? 'Connected' : 'Unavailable'}</StatusPill>
                    </div>
                    <p className="text-xs text-ds-muted-foreground">{feed.detail}</p>
                    <CoverageBar value={feed.coverage} ok={ok} />
                  </div>
                );
              })}

              <div className="ds-agent-panel rounded-lg border border-ds-brand-200 bg-ds-brand-50 p-3 dark:border-ds-brand-700 dark:bg-ds-brand-950/30">
                <div className="flex items-center gap-2">
                  <SparklesIcon className="h-5 w-5 text-ds-primary" />
                  <p className="text-sm font-medium text-ds-foreground">Embedded Engine Active</p>
                </div>
                <p className="mt-1 text-xs text-ds-muted-foreground">
                  All operations running on the local embedded database.
                </p>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Alerts + insights */}
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <Card>
            <CardHeader className="flex-row items-center justify-between">
              <CardTitle>Critical Alerts</CardTitle>
              <StatusPill status={criticalAlerts.length > 0 ? 'fail' : 'ok'} pulse={criticalAlerts.length > 0}>
                {criticalAlerts.length} active
              </StatusPill>
            </CardHeader>
            <CardContent>
              {criticalAlerts.length > 0 ? (
                <div className="space-y-2">
                  {criticalAlerts.map((alert) => (
                    <div
                      key={alert.id}
                      className="flex items-start justify-between gap-3 rounded-lg border border-ds-enterprise-line/70 p-3"
                    >
                      <div>
                        <p className="text-sm font-medium text-ds-foreground">{alert.message}</p>
                        <p className="text-xs text-ds-muted-foreground">{alert.time}</p>
                      </div>
                      <StatusPill status={SEVERITY_STATUS[alert.severity]}>{alert.severity}</StatusPill>
                    </div>
                  ))}
                </div>
              ) : (
                <EmptyState
                  title="All clear"
                  description="No active operational alerts in the current snapshot."
                />
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="flex-row items-center justify-between">
              <CardTitle>Operational Insights</CardTitle>
              <StatusPill status="review">Derived Signals</StatusPill>
            </CardHeader>
            <CardContent>
              {aiInsights.length > 0 ? (
                <div className="space-y-3">
                  {aiInsights.map((insight, index) => (
                    <div key={index} className="rounded-lg border border-ds-enterprise-line/70 p-3">
                      <div className="flex items-start gap-2">
                        <StatusPill status={IMPACT_STATUS[insight.impact]} />
                        <p className="text-sm text-ds-foreground">{insight.insight}</p>
                      </div>
                      <p className="mt-2 pl-1 text-xs text-ds-muted-foreground">
                        <span className="font-semibold text-ds-primary">Suggested:</span> {insight.action}
                      </p>
                    </div>
                  ))}
                </div>
              ) : (
                <EmptyState
                  title="No insights yet"
                  description="No live operational insights are available until metrics feeds are connected."
                />
              )}
            </CardContent>
          </Card>
        </div>

        {/* System health */}
        <Card>
          <CardHeader>
            <CardTitle>System Health Monitor</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4 md:grid-cols-5">
              <HealthStat
                label="Database Latency"
                value={`${systemHealth?.databaseLatency ?? 0}ms`}
                status={(systemHealth?.databaseLatency ?? 0) < 100 ? 'ok' : 'warn'}
                caption={(systemHealth?.databaseLatency ?? 0) < 100 ? 'Optimal' : 'Slow'}
              />
              <HealthStat
                label="Error Rate"
                value={`${systemHealth?.errorRate ?? 0}%`}
                status={(systemHealth?.errorRate ?? 0) < 0.1 ? 'ok' : 'fail'}
                caption={(systemHealth?.errorRate ?? 0) < 0.1 ? 'Healthy' : 'High'}
              />
              <HealthStat
                label="Active Connections"
                value={`${systemHealth?.activeConnections ?? 0}`}
                status="run"
                caption="Running"
              />
              <HealthStat
                label="Queue Depth"
                value={`${systemHealth?.queueDepth ?? 0}`}
                status={(systemHealth?.queueDepth ?? 0) < 50 ? 'ok' : 'warn'}
                caption={(systemHealth?.queueDepth ?? 0) < 50 ? 'Normal' : 'Elevated'}
              />
              <HealthStat
                label="Processing Speed"
                value={`${systemHealth?.processingSpeed ?? 0}%`}
                status={(systemHealth?.processingSpeed ?? 0) > 95 ? 'ok' : 'warn'}
                caption={(systemHealth?.processingSpeed ?? 0) > 95 ? 'Fast' : 'Degraded'}
              />
            </div>
          </CardContent>
        </Card>
      </div>
    </ErrorBoundary>
  );
}

function HealthStat({
  label,
  value,
  status,
  caption,
}: {
  label: string;
  value: string;
  status: StatusTone;
  caption: string;
}) {
  return (
    <div className="flex flex-col items-center gap-2 text-center">
      <p className="text-xs text-ds-muted-foreground">{label}</p>
      <p className="ds-instrument-number text-lg text-ds-foreground">{value}</p>
      <StatusPill status={status}>{caption}</StatusPill>
    </div>
  );
}

const UnifiedDashboard = memo(UnifiedDashboardInner);
export default UnifiedDashboard;
