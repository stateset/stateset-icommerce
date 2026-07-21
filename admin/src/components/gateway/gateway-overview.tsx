'use client';

import { useState } from 'react';
import { Card, CardContent, StatusPill } from '@stateset/design';
import { ServerStackIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { ErrorBoundary } from '@/components/ui/error-boundary';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayHealth, getGatewayMetrics, getGatewayReadiness } from '@/lib/gateway-client';
import type { GatewayHealth, GatewayMetrics, GatewayReadiness } from '@/lib/types/gateway';

import { MetricsSummary } from './metrics-summary';
import { SubsystemPanel } from './subsystem-panel';
import { ChannelStatusCard } from './channel-status-card';
import { ChannelMetricsChart } from './channel-metrics-chart';
import { CommandUsageTable } from './command-usage-table';
import { GatewayConnectionStatus } from './connection-status';
import { ChannelDetail } from './channel-detail';

export default function GatewayOverview() {
  const [selectedChannel, setSelectedChannel] = useState<string | null>(null);

  const { data: health, isLoading: loadingHealth } = useEmbeddedData<GatewayHealth>(
    getGatewayHealth,
    { refreshInterval: 10_000 },
  );

  const { data: metrics, isLoading: loadingMetrics } = useEmbeddedData<GatewayMetrics>(
    getGatewayMetrics,
    { refreshInterval: 15_000 },
  );

  const { data: readiness } = useEmbeddedData<GatewayReadiness>(getGatewayReadiness, {
    refreshInterval: 30_000,
  });

  const isLoading = loadingHealth || loadingMetrics;

  if (isLoading && !health && !metrics) {
    return <LoadingSkeleton type="chart" count={4} />;
  }

  if (!health || !metrics) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-center">
        <ServerStackIcon className="w-12 h-12 text-ds-muted-foreground mb-4" />
        <p className="text-sm text-ds-muted-foreground">Unable to connect to gateway</p>
        <p className="text-xs text-ds-muted-foreground mt-1">
          Ensure the CLI gateway is running on the configured URL
        </p>
      </div>
    );
  }

  if (selectedChannel && metrics.channels[selectedChannel]) {
    return <ChannelDetail channelName={selectedChannel} onBack={() => setSelectedChannel(null)} />;
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
              <ServerStackIcon className="w-8 h-8 text-ds-primary" />
              <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Gateway</h3>
            </div>
            <GatewayConnectionStatus />
          </div>
          <p className="text-sm text-ds-muted-foreground">
            Multi-channel messaging gateway status and metrics
          </p>
        </div>

        {/* KPI Row */}
        <MetricsSummary metrics={metrics} />

        {/* Subsystems */}
        <div>
          <h3 className="font-ds-display text-lg font-semibold text-ds-foreground mb-3">
            Subsystems
          </h3>
          <SubsystemPanel subsystems={health.subsystems} />
        </div>

        {/* Readiness Checks */}
        {readiness && (
          <Card>
            <CardContent className="p-5">
              <div className="flex items-center justify-between">
                <h3 className="font-ds-display text-lg font-semibold text-ds-foreground">
                  Readiness
                </h3>
                <StatusPill status={readiness.status === 'ready' ? 'ok' : 'fail'}>
                  {readiness.status.toUpperCase()}
                </StatusPill>
              </div>
              <div className="flex space-x-4 mt-3">
                {Object.entries(readiness.checks).map(([name, status]) => (
                  <div key={name} className="flex items-center space-x-2">
                    <div
                      className={`w-2 h-2 rounded-full ${
                        status === 'ok' || status === 'configured'
                          ? 'bg-ds-status-ok'
                          : 'bg-ds-status-fail'
                      }`}
                    />
                    <p className="text-sm capitalize text-ds-foreground">{name}</p>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        )}

        {/* Channels Grid */}
        <div>
          <h3 className="font-ds-display text-lg font-semibold text-ds-foreground mb-3">
            Channels
          </h3>
          {Object.keys(metrics.channels).length > 0 ? (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {Object.entries(metrics.channels).map(([name, stats]) => (
                <ChannelStatusCard
                  key={name}
                  name={name}
                  stats={stats}
                  onClick={() => setSelectedChannel(name)}
                />
              ))}
            </div>
          ) : (
            <Card>
              <CardContent className="p-8 text-center">
                <p className="text-sm text-ds-muted-foreground">
                  No channels have received messages yet
                </p>
              </CardContent>
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
