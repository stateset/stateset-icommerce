'use client';

import { AreaChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  MetricCard,
  StatusPill,
  type StatusTone,
} from '@stateset/design';
import {
  ServerIcon,
  CpuChipIcon,
  CircleStackIcon,
  BoltIcon,
  CheckCircleIcon,
  ExclamationCircleIcon,
  MagnifyingGlassIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSystemHealthData } from '@/app/actions/commerce';
import { formatNumber } from '@/lib/utils';
import type { SystemHealthData, SystemService, SystemEvent } from '@/lib/types/dashboard-data';

interface SystemHealthProps {
  data?: SystemHealthData;
}

const serviceStatusMap: Record<string, StatusTone> = {
  healthy: 'ok',
  degraded: 'warn',
  critical: 'fail',
  unknown: 'idle',
};

export default function SystemHealth({ data: propData }: SystemHealthProps) {
  const { data, isLoading, error } = useEmbeddedData<SystemHealthData>(
    () => getSystemHealthData(),
    { initialData: propData, refreshInterval: 10000 },
  );

  const healthData = data || propData;

  if (isLoading && !data) {
    return (
      <Card className="p-5">
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-ds-muted rounded w-48" />
          <div className="h-64 bg-ds-muted rounded" />
        </div>
      </Card>
    );
  }

  if (error && !propData) {
    return (
      <Card className="border-ds-status-fail/30 p-5">
        <p className="text-sm text-ds-status-fail">Failed to load system health</p>
      </Card>
    );
  }

  if (!healthData) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>System Health</CardTitle>
          <CardDescription>
            No live system health data is available from the embedded engine yet.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const { summary, services, performance, database, vectorSearch, recentEvents } = healthData;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Overall Health Status */}
      <Card
        className={`p-5 ${
          summary.overallStatus === 'healthy'
            ? 'bg-ds-status-ok/10'
            : summary.overallStatus === 'degraded'
              ? 'bg-ds-status-warn/10'
              : 'bg-ds-status-fail/10'
        }`}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-4">
            <div
              className={`w-12 h-12 rounded-full flex items-center justify-center ${
                summary.overallStatus === 'healthy'
                  ? 'bg-ds-status-ok/15'
                  : summary.overallStatus === 'degraded'
                    ? 'bg-ds-status-warn/15'
                    : 'bg-ds-status-fail/15'
              }`}
            >
              {summary.overallStatus === 'healthy' ? (
                <CheckCircleIcon className="w-6 h-6 text-ds-status-ok" />
              ) : (
                <ExclamationCircleIcon
                  className={`w-6 h-6 ${
                    summary.overallStatus === 'degraded'
                      ? 'text-ds-status-warn'
                      : 'text-ds-status-fail'
                  }`}
                />
              )}
            </div>
            <div>
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                System Status: {summary.overallStatus.toUpperCase()}
              </h3>
              <p className="text-sm text-ds-muted-foreground">
                {summary.healthyServices}/{summary.totalServices} services operational
              </p>
            </div>
          </div>
          <div className="text-right">
            <p className="text-sm text-ds-muted-foreground">Uptime</p>
            <p className="ds-instrument-number text-3xl text-ds-status-ok">{summary.uptime}%</p>
          </div>
        </div>
      </Card>

      {/* Key Metrics */}
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <MetricCard
          label="Database Latency"
          value={`${database.latency}ms`}
          subtitle="SQLite embedded"
          icon={CircleStackIcon}
          tone="primary"
        />
        <MetricCard
          label="Requests/sec"
          value={formatNumber(performance.requestsPerSecond)}
          icon={BoltIcon}
          tone="success"
        />
        <MetricCard
          label="CPU Usage"
          value={`${performance.cpuUsage}%`}
          icon={CpuChipIcon}
          tone="accent"
        />
        <MetricCard
          label="Memory Usage"
          value={`${performance.memoryUsage}%`}
          icon={ServerIcon}
          tone="warning"
        />
      </div>

      {/* Performance Chart */}
      <Card>
        <CardHeader>
          <CardTitle>System Performance</CardTitle>
          <CardDescription>Resource utilization over the last hour</CardDescription>
        </CardHeader>
        <CardContent>
          {performance.timeline.length > 0 ? (
            <AreaChart
              className="h-64"
              data={performance.timeline}
              index="time"
              categories={['cpu', 'memory', 'latency']}
              colors={['violet', 'amber', 'indigo']}
              showAnimation
              curveType="monotone"
            />
          ) : (
            <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-ds-enterprise-line">
              <p className="text-sm text-ds-muted-foreground">
                Performance history is not available from the embedded engine yet.
              </p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Service Status */}
      <Card>
        <CardHeader>
          <CardTitle>Service Status</CardTitle>
          <CardDescription>Individual service health monitoring</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {services.map((service: SystemService, index: number) => (
              <motion.div
                key={service.name || index}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 border border-ds-enterprise-line rounded-lg"
              >
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center space-x-2">
                    <div
                      className={`w-2 h-2 rounded-full ${
                        service.status === 'healthy'
                          ? 'bg-ds-status-ok animate-pulse'
                          : service.status === 'degraded'
                            ? 'bg-ds-status-warn'
                            : 'bg-ds-status-fail'
                      }`}
                    />
                    <p className="text-sm font-medium text-ds-foreground">{service.name}</p>
                  </div>
                  <StatusPill status={serviceStatusMap[service.status] || 'idle'}>
                    {service.status}
                  </StatusPill>
                </div>
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <p className="text-ds-muted-foreground">Latency</p>
                    <p className="text-ds-foreground">{service.latency}ms</p>
                  </div>
                  <div className="flex justify-between text-sm">
                    <p className="text-ds-muted-foreground">Success Rate</p>
                    <p
                      className={
                        service.successRate >= 99 ? 'text-ds-status-ok' : 'text-ds-status-warn'
                      }
                    >
                      {service.successRate}%
                    </p>
                  </div>
                  <ProgressBar
                    value={service.successRate}
                    color={
                      service.successRate >= 99
                        ? 'emerald'
                        : service.successRate >= 95
                          ? 'amber'
                          : 'red'
                    }
                  />
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Database Health */}
      <Card>
        <CardHeader>
          <CardTitle>Database Health</CardTitle>
          <CardDescription>Embedded SQLite performance metrics</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div className="p-4 bg-ds-muted rounded-lg">
              <p className="text-sm text-ds-muted-foreground">Connection Pool</p>
              <p className="ds-instrument-number text-3xl text-ds-foreground">
                {database.connections}/{database.maxConnections}
              </p>
              <ProgressBar
                value={(database.connections / database.maxConnections) * 100}
                color="indigo"
                className="mt-2"
              />
            </div>
            <div className="p-4 bg-ds-muted rounded-lg">
              <p className="text-sm text-ds-muted-foreground">Query Time (avg)</p>
              <p className="ds-instrument-number text-3xl text-ds-foreground">
                {database.avgQueryTime}ms
              </p>
            </div>
            <div className="p-4 bg-ds-muted rounded-lg">
              <p className="text-sm text-ds-muted-foreground">Queries/sec</p>
              <p className="ds-instrument-number text-3xl text-ds-foreground">
                {formatNumber(database.queriesPerSecond)}
              </p>
            </div>
            <div className="p-4 bg-ds-muted rounded-lg">
              <p className="text-sm text-ds-muted-foreground">Database Size</p>
              <p className="ds-instrument-number text-3xl text-ds-foreground">{database.size}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Vector Search / Embedding Stats */}
      {vectorSearch && (
        <Card>
          <CardHeader>
            <div className="flex items-center space-x-2">
              <MagnifyingGlassIcon className="w-5 h-5 text-ds-primary" />
              <CardTitle>Vector Search</CardTitle>
            </div>
            <CardDescription>
              Embedding index status ({vectorSearch.model}, {vectorSearch.dimensions}d)
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Products</p>
                <p className="ds-instrument-number text-3xl text-ds-foreground">
                  {formatNumber(vectorSearch.counts.products)}
                </p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Customers</p>
                <p className="ds-instrument-number text-3xl text-ds-foreground">
                  {formatNumber(vectorSearch.counts.customers)}
                </p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Orders</p>
                <p className="ds-instrument-number text-3xl text-ds-foreground">
                  {formatNumber(vectorSearch.counts.orders)}
                </p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Inventory</p>
                <p className="ds-instrument-number text-3xl text-ds-foreground">
                  {formatNumber(vectorSearch.counts.inventory)}
                </p>
              </div>
            </div>
            <div className="mt-4 p-3 bg-ds-brand-50 rounded-lg">
              <div className="flex justify-between items-center">
                <p className="text-sm font-medium text-ds-primary">
                  Total Indexed: {formatNumber(vectorSearch.total)}
                </p>
                <StatusPill status={vectorSearch.total > 0 ? 'ok' : 'idle'}>
                  {vectorSearch.total > 0 ? 'Active' : 'Empty'}
                </StatusPill>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Recent Events */}
      <Card>
        <CardHeader>
          <CardTitle>Recent System Events</CardTitle>
          <CardDescription>Latest system activity and alerts</CardDescription>
        </CardHeader>
        <CardContent>
          {recentEvents.length > 0 ? (
            <div className="space-y-3">
              {recentEvents.map((event: SystemEvent, index: number) => (
                <motion.div
                  key={index}
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: index * 0.03 }}
                  className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg"
                >
                  <div className="flex items-center space-x-3">
                    <div
                      className={`w-8 h-8 rounded-full flex items-center justify-center ${
                        event.type === 'success'
                          ? 'bg-ds-status-ok/15'
                          : event.type === 'warning'
                            ? 'bg-ds-status-warn/15'
                            : event.type === 'error'
                              ? 'bg-ds-status-fail/15'
                              : 'bg-ds-status-run/15'
                      }`}
                    >
                      {event.type === 'success' ? (
                        <CheckCircleIcon className="w-4 h-4 text-ds-status-ok" />
                      ) : event.type === 'warning' ? (
                        <ExclamationCircleIcon className="w-4 h-4 text-ds-status-warn" />
                      ) : event.type === 'error' ? (
                        <ExclamationCircleIcon className="w-4 h-4 text-ds-status-fail" />
                      ) : (
                        <ServerIcon className="w-4 h-4 text-ds-status-run" />
                      )}
                    </div>
                    <div>
                      <p className="text-sm font-medium text-ds-foreground">{event.message}</p>
                      <p className="text-xs text-ds-muted-foreground">{event.service}</p>
                    </div>
                  </div>
                  <p className="text-sm text-ds-muted-foreground">{event.timestamp}</p>
                </motion.div>
              ))}
            </div>
          ) : (
            <div className="rounded-lg border border-dashed border-ds-enterprise-line p-6 text-center">
              <p className="text-sm text-ds-muted-foreground">
                No recent system events are available from the embedded engine.
              </p>
            </div>
          )}
        </CardContent>
      </Card>
    </motion.div>
  );
}
