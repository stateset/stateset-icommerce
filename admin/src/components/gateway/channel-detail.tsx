'use client';

import { useState, useRef, useCallback } from 'react';
import {
  Card,
  Title,
  Text,
  Metric,
  Grid,
  AreaChart,
  ProgressBar,
  Badge,
} from '@tremor/react';
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
    [channelName]
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
            className="flex items-center space-x-1 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-colors"
          >
            <ArrowLeftIcon className="w-4 h-4" />
            <span>Back to channels</span>
          </button>
        )}
        <Card className="p-8 text-center">
          <Text className="text-gray-400">
            No data available for channel: {channelName}
          </Text>
        </Card>
      </div>
    );
  }

  const displayName = DISPLAY_NAMES[channelName] || channelName;
  const errorRate =
    stats.messagesReceived > 0
      ? (stats.errors / stats.messagesReceived) * 100
      : 0;

  return (
    <div className="space-y-6">
      {onBack && (
        <button
          onClick={onBack}
          className="flex items-center space-x-1 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-colors"
        >
          <ArrowLeftIcon className="w-4 h-4" />
          <span>Back to channels</span>
        </button>
      )}

      <div className="flex items-center justify-between">
        <Title className="text-2xl">{displayName}</Title>
        <Badge color={stats.lastMessageAt ? 'emerald' : 'gray'} size="sm">
          {stats.lastMessageAt ? 'Online' : 'Idle'}
        </Badge>
      </div>

      {/* KPI Row */}
      <Grid numItems={2} numItemsLg={5} className="gap-4">
        <Card decoration="top" decorationColor="indigo">
          <Text>Messages</Text>
          <Metric className="text-xl">{stats.messagesReceived.toLocaleString()}</Metric>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Responses</Text>
          <Metric className="text-xl">{stats.responsesSent.toLocaleString()}</Metric>
        </Card>
        <Card decoration="top" decorationColor={stats.errors > 0 ? 'red' : 'emerald'}>
          <Text>Errors</Text>
          <Metric className="text-xl">{stats.errors}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Blocked</Text>
          <Metric className="text-xl">{stats.blocked}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Avg Response</Text>
          <Metric className="text-xl">{Math.round(stats.avgResponseMs)}ms</Metric>
        </Card>
      </Grid>

      {/* Error Rate */}
      <Card>
        <div className="flex items-center justify-between mb-2">
          <Title className="text-lg">Error Rate</Title>
          <Text className="font-semibold">{errorRate.toFixed(2)}%</Text>
        </div>
        <ProgressBar
          value={Math.min(errorRate, 100)}
          color={errorRate > 5 ? 'red' : errorRate > 1 ? 'amber' : 'emerald'}
        />
      </Card>

      {/* Message Volume Chart */}
      <Card>
        <Title>Message Volume (per interval)</Title>
        <Text className="text-gray-500 mb-4">
          New messages per 10-second polling interval
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

      {/* Last Activity */}
      {stats.lastMessageAt && (
        <Card>
          <Text className="text-gray-500">
            Last activity: {formatRelativeTime(stats.lastMessageAt)}
          </Text>
        </Card>
      )}
    </div>
  );
}
