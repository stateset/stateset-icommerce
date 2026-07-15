'use client';

import { useState, useRef, useCallback, useEffect } from 'react';
import { AreaChart } from '@tremor/react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  StatusPill,
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@stateset/design';
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
        <ServerStackIcon className="w-12 h-12 text-ds-muted-foreground mb-4" />
        <p className="text-sm text-ds-muted-foreground">Unable to connect to gateway</p>
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
        <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Metrics</h3>
        <p className="text-sm text-ds-muted-foreground">
          Detailed gateway performance metrics and analytics
        </p>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="channels">Channels</TabsTrigger>
          <TabsTrigger value="commands">Commands</TabsTrigger>
          <TabsTrigger value="response-times">Response Times</TabsTrigger>
        </TabsList>

        {/* Overview */}
        <TabsContent value="overview" className="mt-6 space-y-6">
          <MetricsSummary metrics={metrics} />
          <ChannelMetricsChart metrics={metrics} />
        </TabsContent>

        {/* Channels */}
        <TabsContent value="channels" className="mt-6 space-y-4">
          {channelEntries.length === 0 ? (
            <Card className="p-8 text-center">
              <p className="text-sm text-ds-muted-foreground">No channels active</p>
            </Card>
          ) : (
            channelEntries.map(([name, stats]) => (
              <Card key={name} className="p-5">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="font-ds-display text-lg font-semibold capitalize text-ds-foreground">
                    {name}
                  </h3>
                  <StatusPill status={stats.lastMessageAt ? 'ok' : 'idle'}>
                    {stats.lastMessageAt ? 'Active' : 'Idle'}
                  </StatusPill>
                </div>
                <div className="grid grid-cols-2 lg:grid-cols-5 gap-4">
                  <div>
                    <p className="text-xs text-ds-muted-foreground">Messages In</p>
                    <p className="ds-instrument-number text-lg text-ds-foreground">
                      {stats.messagesReceived.toLocaleString()}
                    </p>
                  </div>
                  <div>
                    <p className="text-xs text-ds-muted-foreground">Responses</p>
                    <p className="ds-instrument-number text-lg text-ds-foreground">
                      {stats.responsesSent.toLocaleString()}
                    </p>
                  </div>
                  <div>
                    <p className="text-xs text-ds-muted-foreground">Errors</p>
                    <p
                      className={`ds-instrument-number text-lg ${stats.errors > 0 ? 'text-ds-status-fail' : 'text-ds-foreground'}`}
                    >
                      {stats.errors}
                    </p>
                  </div>
                  <div>
                    <p className="text-xs text-ds-muted-foreground">Blocked</p>
                    <p className="ds-instrument-number text-lg text-ds-foreground">{stats.blocked}</p>
                  </div>
                  <div>
                    <p className="text-xs text-ds-muted-foreground">Avg Response</p>
                    <p className="ds-instrument-number text-lg text-ds-foreground">
                      {Math.round(stats.avgResponseMs)}ms
                    </p>
                  </div>
                </div>
              </Card>
            ))
          )}
        </TabsContent>

        {/* Commands */}
        <TabsContent value="commands" className="mt-6">
          <CommandUsageTable commandUsage={metrics.commandUsage} />
        </TabsContent>

        {/* Response Times */}
        <TabsContent value="response-times" className="mt-6 space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Response Time Trend</CardTitle>
              <CardDescription>
                Average response time per polling interval (10s)
              </CardDescription>
            </CardHeader>
            <CardContent>
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
                  <p className="text-sm text-ds-muted-foreground">Accumulating data...</p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Throughput Trend</CardTitle>
              <CardDescription>
                Messages and responses per interval
              </CardDescription>
            </CardHeader>
            <CardContent>
              {history.length > 1 ? (
                <AreaChart
                  className="h-72"
                  data={history}
                  index="timestamp"
                  categories={['messagesReceived', 'responsesSent', 'errors']}
                  colors={['indigo', 'emerald', 'amber']}
                  showAnimation
                />
              ) : (
                <div className="h-72 flex items-center justify-center">
                  <p className="text-sm text-ds-muted-foreground">Accumulating data...</p>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </motion.div>
  );
}
