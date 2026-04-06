'use client';

import { Card, Title, Text, Badge, Grid, Metric, AreaChart, ProgressBar } from '@tremor/react';
import { ServerIcon, CpuChipIcon, CircleStackIcon, BoltIcon, CheckCircleIcon, ExclamationCircleIcon, MagnifyingGlassIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSystemHealthData } from '@/app/actions/commerce';
import { formatNumber } from '@/lib/utils';
import type { SystemHealthData, SystemService, SystemEvent, TremorColor } from '@/lib/types/dashboard-data';

interface SystemHealthProps {
  data?: SystemHealthData;
}

const statusColors: Record<string, string> = {
  healthy: 'emerald',
  degraded: 'amber',
  critical: 'red',
  unknown: 'gray',
};

export default function SystemHealth({ data: propData }: SystemHealthProps) {
  const { data, isLoading, error } = useEmbeddedData<SystemHealthData>(
    () => getSystemHealthData(),
    { initialData: propData, refreshInterval: 10000 }
  );

  const healthData = data || propData;

  if (isLoading && !data) {
    return (
      <Card>
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-gray-200 rounded w-48" />
          <div className="h-64 bg-gray-200 rounded" />
        </div>
      </Card>
    );
  }

  if (error && !propData) {
    return (
      <Card className="border-red-200">
        <Text className="text-red-600">Failed to load system health</Text>
      </Card>
    );
  }

  if (!healthData) {
    return (
      <Card>
        <Title>System Health</Title>
        <Text className="mt-2 text-gray-500">
          No live system health data is available from the embedded engine yet.
        </Text>
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
      <Card className={`${
        summary.overallStatus === 'healthy' ? 'bg-emerald-50 dark:bg-emerald-900/20' :
        summary.overallStatus === 'degraded' ? 'bg-amber-50 dark:bg-amber-900/20' :
        'bg-red-50 dark:bg-red-900/20'
      }`}>
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-4">
            <div className={`w-12 h-12 rounded-full flex items-center justify-center ${
              summary.overallStatus === 'healthy' ? 'bg-emerald-100 dark:bg-emerald-900/30' :
              summary.overallStatus === 'degraded' ? 'bg-amber-100 dark:bg-amber-900/30' :
              'bg-red-100 dark:bg-red-900/30'
            }`}>
              {summary.overallStatus === 'healthy' ? (
                <CheckCircleIcon className="w-6 h-6 text-emerald-600" />
              ) : (
                <ExclamationCircleIcon className={`w-6 h-6 ${
                  summary.overallStatus === 'degraded' ? 'text-amber-600' : 'text-red-600'
                }`} />
              )}
            </div>
            <div>
              <Title>System Status: {summary.overallStatus.toUpperCase()}</Title>
              <Text className="text-gray-600">
                {summary.healthyServices}/{summary.totalServices} services operational
              </Text>
            </div>
          </div>
          <div className="text-right">
            <Text className="text-sm text-gray-500">Uptime</Text>
            <Metric className="text-emerald-600">{summary.uptime}%</Metric>
          </div>
        </div>
      </Card>

      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <div className="flex items-center space-x-2">
            <CircleStackIcon className="w-5 h-5 text-blue-600" />
            <Text>Database Latency</Text>
          </div>
          <Metric>{database.latency}ms</Metric>
          <Text className="text-xs text-blue-600 mt-1">SQLite embedded</Text>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <div className="flex items-center space-x-2">
            <BoltIcon className="w-5 h-5 text-emerald-600" />
            <Text>Requests/sec</Text>
          </div>
          <Metric>{formatNumber(performance.requestsPerSecond)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <div className="flex items-center space-x-2">
            <CpuChipIcon className="w-5 h-5 text-purple-600" />
            <Text>CPU Usage</Text>
          </div>
          <Metric>{performance.cpuUsage}%</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <div className="flex items-center space-x-2">
            <ServerIcon className="w-5 h-5 text-amber-600" />
            <Text>Memory Usage</Text>
          </div>
          <Metric>{performance.memoryUsage}%</Metric>
        </Card>
      </Grid>

      {/* Performance Chart */}
      <Card>
        <Title>System Performance</Title>
        <Text className="text-gray-500 mb-4">Resource utilization over the last hour</Text>
        {performance.timeline.length > 0 ? (
          <AreaChart
            className="h-64"
            data={performance.timeline}
            index="time"
            categories={['cpu', 'memory', 'latency']}
            colors={['purple', 'amber', 'blue']}
            showAnimation
            curveType="monotone"
          />
        ) : (
          <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-gray-200 dark:border-gray-700">
            <Text className="text-gray-500">
              Performance history is not available from the embedded engine yet.
            </Text>
          </div>
        )}
      </Card>

      {/* Service Status */}
      <Card>
        <Title>Service Status</Title>
        <Text className="text-gray-500 mb-4">Individual service health monitoring</Text>
        <Grid numItems={1} numItemsSm={2} numItemsLg={3} className="gap-4">
          {services.map((service: SystemService, index: number) => (
            <motion.div
              key={service.name || index}
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: index * 0.05 }}
              className="p-4 border rounded-lg dark:border-gray-700"
            >
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center space-x-2">
                  <div className={`w-2 h-2 rounded-full ${
                    service.status === 'healthy' ? 'bg-emerald-500 animate-pulse' :
                    service.status === 'degraded' ? 'bg-amber-500' :
                    'bg-red-500'
                  }`} />
                  <Text className="font-medium">{service.name}</Text>
                </div>
                <Badge color={statusColors[service.status] as TremorColor || 'gray'} size="xs">
                  {service.status}
                </Badge>
              </div>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <Text className="text-gray-500">Latency</Text>
                  <Text>{service.latency}ms</Text>
                </div>
                <div className="flex justify-between text-sm">
                  <Text className="text-gray-500">Success Rate</Text>
                  <Text className={service.successRate >= 99 ? 'text-emerald-600' : 'text-amber-600'}>
                    {service.successRate}%
                  </Text>
                </div>
                <ProgressBar
                  value={service.successRate}
                  color={service.successRate >= 99 ? 'emerald' : service.successRate >= 95 ? 'amber' : 'red'}
                />
              </div>
            </motion.div>
          ))}
        </Grid>
      </Card>

      {/* Database Health */}
      <Card>
        <Title>Database Health</Title>
        <Text className="text-gray-500 mb-4">Embedded SQLite performance metrics</Text>
        <Grid numItems={2} numItemsSm={4} className="gap-4">
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <Text className="text-sm text-gray-500">Connection Pool</Text>
            <Metric>{database.connections}/{database.maxConnections}</Metric>
            <ProgressBar
              value={(database.connections / database.maxConnections) * 100}
              color="blue"
              className="mt-2"
            />
          </div>
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <Text className="text-sm text-gray-500">Query Time (avg)</Text>
            <Metric>{database.avgQueryTime}ms</Metric>
          </div>
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <Text className="text-sm text-gray-500">Queries/sec</Text>
            <Metric>{formatNumber(database.queriesPerSecond)}</Metric>
          </div>
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <Text className="text-sm text-gray-500">Database Size</Text>
            <Metric>{database.size}</Metric>
          </div>
        </Grid>
      </Card>

      {/* Vector Search / Embedding Stats */}
      {vectorSearch && (
        <Card>
          <div className="flex items-center space-x-2 mb-4">
            <MagnifyingGlassIcon className="w-5 h-5 text-indigo-600" />
            <Title>Vector Search</Title>
          </div>
          <Text className="text-gray-500 mb-4">
            Embedding index status ({vectorSearch.model}, {vectorSearch.dimensions}d)
          </Text>
          <Grid numItems={2} numItemsSm={4} className="gap-4">
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Products</Text>
              <Metric>{formatNumber(vectorSearch.counts.products)}</Metric>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Customers</Text>
              <Metric>{formatNumber(vectorSearch.counts.customers)}</Metric>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Orders</Text>
              <Metric>{formatNumber(vectorSearch.counts.orders)}</Metric>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Inventory</Text>
              <Metric>{formatNumber(vectorSearch.counts.inventory)}</Metric>
            </div>
          </Grid>
          <div className="mt-4 p-3 bg-indigo-50 dark:bg-indigo-900/20 rounded-lg">
            <div className="flex justify-between items-center">
              <Text className="text-sm font-medium text-indigo-700 dark:text-indigo-300">
                Total Indexed: {formatNumber(vectorSearch.total)}
              </Text>
              <Badge color="indigo" size="xs">
                {vectorSearch.total > 0 ? 'Active' : 'Empty'}
              </Badge>
            </div>
          </div>
        </Card>
      )}

      {/* Recent Events */}
      <Card>
        <Title>Recent System Events</Title>
        <Text className="text-gray-500 mb-4">Latest system activity and alerts</Text>
        {recentEvents.length > 0 ? (
          <div className="space-y-3">
            {recentEvents.map((event: SystemEvent, index: number) => (
              <motion.div
                key={index}
                initial={{ opacity: 0, x: -10 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.03 }}
                className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700"
              >
                <div className="flex items-center space-x-3">
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center ${
                    event.type === 'success' ? 'bg-emerald-100 dark:bg-emerald-900/30' :
                    event.type === 'warning' ? 'bg-amber-100 dark:bg-amber-900/30' :
                    event.type === 'error' ? 'bg-red-100 dark:bg-red-900/30' :
                    'bg-blue-100 dark:bg-blue-900/30'
                  }`}>
                    {event.type === 'success' ? (
                      <CheckCircleIcon className="w-4 h-4 text-emerald-600" />
                    ) : event.type === 'warning' ? (
                      <ExclamationCircleIcon className="w-4 h-4 text-amber-600" />
                    ) : event.type === 'error' ? (
                      <ExclamationCircleIcon className="w-4 h-4 text-red-600" />
                    ) : (
                      <ServerIcon className="w-4 h-4 text-blue-600" />
                    )}
                  </div>
                  <div>
                    <Text className="font-medium">{event.message}</Text>
                    <Text className="text-xs text-gray-500">{event.service}</Text>
                  </div>
                </div>
                <Text className="text-sm text-gray-500">{event.timestamp}</Text>
              </motion.div>
            ))}
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
            <Text className="text-gray-500">
              No recent system events are available from the embedded engine.
            </Text>
          </div>
        )}
      </Card>
    </motion.div>
  );
}
