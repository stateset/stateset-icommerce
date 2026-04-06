'use client';

import { useState } from 'react';
import { Card, Title, Text, Grid } from '@tremor/react';
import { ServerStackIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { ErrorBoundary } from '@/components/ui/error-boundary';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import {
  getGatewayHealth,
  getGatewayMetrics,
  getGatewayReadiness,
} from '@/lib/gateway-client';
import type {
  GatewayHealth,
  GatewayMetrics,
  GatewayReadiness,
} from '@/lib/types/gateway';

import { MetricsSummary } from './metrics-summary';
import { SubsystemPanel } from './subsystem-panel';
import { ChannelStatusCard } from './channel-status-card';
import { ChannelMetricsChart } from './channel-metrics-chart';
import { CommandUsageTable } from './command-usage-table';
import { GatewayConnectionStatus } from './connection-status';
import { ChannelDetail } from './channel-detail';

export default function GatewayOverview() {
  const [selectedChannel, setSelectedChannel] = useState<string | null>(null);

  const { data: health, isLoading: loadingHealth } =
    useEmbeddedData<GatewayHealth>(getGatewayHealth, { refreshInterval: 10_000 });

  const { data: metrics, isLoading: loadingMetrics } =
    useEmbeddedData<GatewayMetrics>(getGatewayMetrics, { refreshInterval: 15_000 });

  const { data: readiness } = useEmbeddedData<GatewayReadiness>(
    getGatewayReadiness,
    { refreshInterval: 30_000 }
  );

  const isLoading = loadingHealth || loadingMetrics;

  if (isLoading && !health && !metrics) {
    return <LoadingSkeleton type="chart" count={4} />;
  }

  if (!health || !metrics) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-center">
        <ServerStackIcon className="w-12 h-12 text-gray-300 mb-4" />
        <Text className="text-gray-500">Unable to connect to gateway</Text>
        <Text className="text-xs text-gray-400 mt-1">
          Ensure the CLI gateway is running on the configured URL
        </Text>
      </div>
    );
  }

  if (selectedChannel && metrics.channels[selectedChannel]) {
    return (
      <ChannelDetail
        channelName={selectedChannel}
        onBack={() => setSelectedChannel(null)}
      />
    );
  }

  return (
    <ErrorBoundary>
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
        className="space-y-6"
      >
        {/* Header */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center space-x-3">
              <ServerStackIcon className="w-8 h-8 text-indigo-600" />
              <Title className="text-2xl">Gateway</Title>
            </div>
            <GatewayConnectionStatus />
          </div>
          <Text className="text-gray-500">
            Multi-channel messaging gateway status and metrics
          </Text>
        </div>

        {/* KPI Row */}
        <MetricsSummary metrics={metrics} />

        {/* Subsystems */}
        <div>
          <Title className="text-lg mb-3">Subsystems</Title>
          <SubsystemPanel subsystems={health.subsystems} />
        </div>

        {/* Readiness Checks */}
        {readiness && (
          <Card>
            <div className="flex items-center justify-between">
              <Title className="text-lg">Readiness</Title>
              <Text
                className={
                  readiness.status === 'ready'
                    ? 'text-emerald-600 font-medium'
                    : 'text-red-600 font-medium'
                }
              >
                {readiness.status.toUpperCase()}
              </Text>
            </div>
            <div className="flex space-x-4 mt-3">
              {Object.entries(readiness.checks).map(([name, status]) => (
                <div key={name} className="flex items-center space-x-2">
                  <div
                    className={`w-2 h-2 rounded-full ${
                      status === 'ok' || status === 'configured'
                        ? 'bg-emerald-500'
                        : 'bg-red-500'
                    }`}
                  />
                  <Text className="text-sm capitalize">{name}</Text>
                </div>
              ))}
            </div>
          </Card>
        )}

        {/* Channels Grid */}
        <div>
          <Title className="text-lg mb-3">Channels</Title>
          {Object.keys(metrics.channels).length > 0 ? (
            <Grid numItems={1} numItemsSm={2} numItemsLg={3} className="gap-4">
              {Object.entries(metrics.channels).map(([name, stats]) => (
                <ChannelStatusCard
                  key={name}
                  name={name}
                  stats={stats}
                  onClick={() => setSelectedChannel(name)}
                />
              ))}
            </Grid>
          ) : (
            <Card className="p-8 text-center">
              <Text className="text-gray-400">
                No channels have received messages yet
              </Text>
            </Card>
          )}
        </div>

        {/* Charts */}
        <ChannelMetricsChart metrics={metrics} />

        {/* Command Usage */}
        <CommandUsageTable commandUsage={metrics.commandUsage} />
      </motion.div>
    </ErrorBoundary>
  );
}
