'use client';

import { useState, useRef, useCallback, useEffect } from 'react';
import {
  Card,
  Title,
  Text,
  Tab,
  TabGroup,
  TabList,
  TabPanel,
  TabPanels,
  AreaChart,
  Grid,
  Metric,
  Badge,
} from '@tremor/react';
import { ServerStackIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayMetrics } from '@/lib/gateway-client';
import type { GatewayMetrics, MetricsSnapshot } from '@/lib/types/gateway';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { MetricsSummary } from './metrics-summary';
import { ChannelMetricsChart } from './channel-metrics-chart';
import { CommandUsageTable } from './command-usage-table';

const MAX_HISTORY = 120;

export default function MetricsDashboard() {
  const [history, setHistory] = useState<MetricsSnapshot[]>([]);
  const prevTotalsRef = useRef<GatewayMetrics['totals'] | null>(null);

  const accumulate = useCallback((metrics: GatewayMetrics) => {
    const prev = prevTotalsRef.current;
    prevTotalsRef.current = { ...metrics.totals };

    const snapshot: MetricsSnapshot = {
      timestamp: new Date().toLocaleTimeString(),
      messagesReceived: prev
        ? metrics.totals.messagesReceived - prev.messagesReceived
        : 0,
      responsesSent: prev
        ? metrics.totals.responsesSent - prev.responsesSent
        : 0,
      errors: prev ? metrics.totals.errors - prev.errors : 0,
      avgResponseMs: metrics.totals.avgResponseMs,
    };

    setHistory((h) => [...h.slice(-(MAX_HISTORY - 1)), snapshot]);
  }, []);

  const { data: metrics, isLoading } = useEmbeddedData<GatewayMetrics>(
    getGatewayMetrics,
    { refreshInterval: 10_000 }
  );

  useEffect(() => {
    if (!metrics) return;
    accumulate(metrics);
  }, [metrics, accumulate]);

  if (isLoading && !metrics) {
    return <LoadingSkeleton type="chart" count={3} />;
  }

  if (!metrics) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-center">
        <ServerStackIcon className="w-12 h-12 text-gray-300 mb-4" />
        <Text className="text-gray-500">Unable to connect to gateway</Text>
      </div>
    );
  }

  const channelEntries = Object.entries(metrics.channels);

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5 }}
      className="space-y-6"
    >
      <div>
        <Title className="text-2xl">Metrics</Title>
        <Text className="text-gray-500">
          Detailed gateway performance metrics and analytics
        </Text>
      </div>

      <TabGroup>
        <TabList>
          <Tab>Overview</Tab>
          <Tab>Channels</Tab>
          <Tab>Commands</Tab>
          <Tab>Response Times</Tab>
        </TabList>

        <TabPanels>
          {/* Overview */}
          <TabPanel className="mt-6 space-y-6">
            <MetricsSummary metrics={metrics} />
            <ChannelMetricsChart metrics={metrics} />
          </TabPanel>

          {/* Channels */}
          <TabPanel className="mt-6 space-y-4">
            {channelEntries.length === 0 ? (
              <Card className="p-8 text-center">
                <Text className="text-gray-400">No channels active</Text>
              </Card>
            ) : (
              channelEntries.map(([name, stats]) => (
                <Card key={name}>
                  <div className="flex items-center justify-between mb-3">
                    <Title className="text-lg capitalize">{name}</Title>
                    <Badge
                      color={stats.lastMessageAt ? 'emerald' : 'gray'}
                      size="xs"
                    >
                      {stats.lastMessageAt ? 'Active' : 'Idle'}
                    </Badge>
                  </div>
                  <Grid numItems={2} numItemsLg={5} className="gap-4">
                    <div>
                      <Text className="text-xs text-gray-400">Messages In</Text>
                      <Metric className="text-lg">
                        {stats.messagesReceived.toLocaleString()}
                      </Metric>
                    </div>
                    <div>
                      <Text className="text-xs text-gray-400">Responses</Text>
                      <Metric className="text-lg">
                        {stats.responsesSent.toLocaleString()}
                      </Metric>
                    </div>
                    <div>
                      <Text className="text-xs text-gray-400">Errors</Text>
                      <Metric
                        className={`text-lg ${stats.errors > 0 ? 'text-red-500' : ''}`}
                      >
                        {stats.errors}
                      </Metric>
                    </div>
                    <div>
                      <Text className="text-xs text-gray-400">Blocked</Text>
                      <Metric className="text-lg">{stats.blocked}</Metric>
                    </div>
                    <div>
                      <Text className="text-xs text-gray-400">Avg Response</Text>
                      <Metric className="text-lg">
                        {Math.round(stats.avgResponseMs)}ms
                      </Metric>
                    </div>
                  </Grid>
                </Card>
              ))
            )}
          </TabPanel>

          {/* Commands */}
          <TabPanel className="mt-6">
            <CommandUsageTable commandUsage={metrics.commandUsage} />
          </TabPanel>

          {/* Response Times */}
          <TabPanel className="mt-6 space-y-6">
            <Card>
              <Title>Response Time Trend</Title>
              <Text className="text-gray-500 mb-4">
                Average response time per polling interval (10s)
              </Text>
              {history.length > 1 ? (
                <AreaChart
                  className="h-72"
                  data={history}
                  index="timestamp"
                  categories={['avgResponseMs']}
                  colors={['indigo']}
                  valueFormatter={(v) => `${Math.round(v)}ms`}
                  showAnimation
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <Text className="text-gray-400">Accumulating data...</Text>
                </div>
              )}
            </Card>

            <Card>
              <Title>Throughput Trend</Title>
              <Text className="text-gray-500 mb-4">
                Messages and responses per interval
              </Text>
              {history.length > 1 ? (
                <AreaChart
                  className="h-72"
                  data={history}
                  index="timestamp"
                  categories={['messagesReceived', 'responsesSent', 'errors']}
                  colors={['indigo', 'blue', 'red']}
                  showAnimation
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <Text className="text-gray-400">Accumulating data...</Text>
                </div>
              )}
            </Card>
          </TabPanel>
        </TabPanels>
      </TabGroup>
    </motion.div>
  );
}
