'use client';

import { useState, useRef, useCallback } from 'react';
import { AreaChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  MetricCard,
  StatusPill,
} from '@stateset/design';
import { ArrowLeftIcon } from '@heroicons/react/24/outline';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayMetrics } from '@/lib/gateway-client';
import type { GatewayMetrics, ChannelStats, MetricsSnapshot } from '@/lib/types/gateway';
import { formatRelativeTime } from '@/lib/utils';

interface ChannelDetailProps {
  channelName: string;
  onBack?: () => void;
}

const DISPLAY_NAMES: Record<string, string> = {
  discord: 'Discord',
  slack: 'Slack',
  telegram: 'Telegram',
  whatsapp: 'WhatsApp',
  signal: 'Signal',
  imessage: 'iMessage',
  teams: 'Teams',
  matrix: 'Matrix',
  'google-chat': 'Google Chat',
  webchat: 'Webchat',
  http: 'HTTP API',
};

const MAX_HISTORY = 60;

export function ChannelDetail({ channelName, onBack }: ChannelDetailProps) {
  const [history, setHistory] = useState<MetricsSnapshot[]>([]);
  const prevRef = useRef<ChannelStats | null>(null);

  const handleMetrics = useCallback(
    (metrics: GatewayMetrics) => {
      const stats = metrics.channels[channelName];
      if (!stats) return;

      const prev = prevRef.current;
      prevRef.current = stats;

      const snapshot: MetricsSnapshot = {
        timestamp: new Date().toLocaleTimeString(),
        messagesReceived: prev ? stats.messagesReceived - prev.messagesReceived : 0,
        responsesSent: prev ? stats.responsesSent - prev.responsesSent : 0,
        errors: prev ? stats.errors - prev.errors : 0,
        avgResponseMs: stats.avgResponseMs,
      };

      setHistory((h) => [...h.slice(-(MAX_HISTORY - 1)), snapshot]);
    },
    [channelName],
  );

  const { data: metrics } = useEmbeddedData<GatewayMetrics>(getGatewayMetrics, {
    refreshInterval: 10_000,
  });

  // Feed metrics into history accumulator
  if (metrics && prevRef.current !== metrics.channels[channelName]) {
    handleMetrics(metrics);
  }

  const stats = metrics?.channels[channelName];

  if (!stats) {
    return (
      <div className="space-y-4">
        {onBack && (
          <button
            onClick={onBack}
            className="flex items-center space-x-1 text-sm text-ds-muted-foreground hover:text-ds-foreground transition-colors"
          >
            <ArrowLeftIcon className="w-4 h-4" />
            <span>Back to channels</span>
          </button>
        )}
        <Card>
          <CardContent className="p-8 text-center">
            <p className="text-sm text-ds-muted-foreground">
              No data available for channel: {channelName}
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  const displayName = DISPLAY_NAMES[channelName] || channelName;
  const errorRate = stats.messagesReceived > 0 ? (stats.errors / stats.messagesReceived) * 100 : 0;

  return (
    <div className="space-y-6">
      {onBack && (
        <button
          onClick={onBack}
          className="flex items-center space-x-1 text-sm text-ds-muted-foreground hover:text-ds-foreground transition-colors"
        >
          <ArrowLeftIcon className="w-4 h-4" />
          <span>Back to channels</span>
        </button>
      )}

      <div className="flex items-center justify-between">
        <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">{displayName}</h3>
        <StatusPill status={stats.lastMessageAt ? 'ok' : 'idle'}>
          {stats.lastMessageAt ? 'Online' : 'Idle'}
        </StatusPill>
      </div>

      {/* KPI Row */}
      <div className="grid grid-cols-2 lg:grid-cols-5 gap-4">
        <MetricCard
          label="Messages"
          value={stats.messagesReceived}
          format="number"
          tone="primary"
        />
        <MetricCard label="Responses" value={stats.responsesSent} format="number" tone="accent" />
        <MetricCard
          label="Errors"
          value={stats.errors}
          tone={stats.errors > 0 ? 'danger' : 'success'}
        />
        <MetricCard label="Blocked" value={stats.blocked} tone="warning" />
        <MetricCard
          label="Avg Response"
          value={`${Math.round(stats.avgResponseMs)}ms`}
          tone="success"
        />
      </div>

      {/* Error Rate */}
      <Card>
        <CardContent className="p-5">
          <div className="flex items-center justify-between mb-2">
            <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
              Error Rate
            </h3>
            <p className="text-sm font-semibold text-ds-foreground">{errorRate.toFixed(2)}%</p>
          </div>
          <ProgressBar
            value={Math.min(errorRate, 100)}
            color={errorRate > 5 ? 'red' : errorRate > 1 ? 'amber' : 'emerald'}
          />
        </CardContent>
      </Card>

      {/* Message Volume Chart */}
      <Card>
        <CardHeader>
          <CardTitle>Message Volume (per interval)</CardTitle>
          <CardDescription>New messages per 10-second polling interval</CardDescription>
        </CardHeader>
        <CardContent>
          {history.length > 1 ? (
            <AreaChart
              className="h-72"
              data={history}
              index="timestamp"
              categories={['messagesReceived', 'responsesSent', 'errors']}
              colors={['indigo', 'emerald', 'violet']}
              showAnimation
            />
          ) : (
            <div className="h-72 flex items-center justify-center">
              <p className="text-sm text-ds-muted-foreground">Accumulating data...</p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Last Activity */}
      {stats.lastMessageAt && (
        <Card>
          <CardContent className="p-5">
            <p className="text-sm text-ds-muted-foreground">
              Last activity: {formatRelativeTime(stats.lastMessageAt)}
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
