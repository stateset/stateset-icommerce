'use client';

import { Card, Title, Text, Badge, Grid, Metric } from '@tremor/react';
import { ServerStackIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayMetrics } from '@/lib/gateway-client';
import type { GatewayMetrics } from '@/lib/types/gateway';
import LoadingSkeleton from '@/components/ui/loading-skeleton';
import { formatRelativeTime } from '@/lib/utils';

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

export default function SessionsDashboard() {
  const { data: metrics, isLoading } = useEmbeddedData<GatewayMetrics>(
    getGatewayMetrics,
    { refreshInterval: 10_000 }
  );

  if (isLoading && !metrics) {
    return <LoadingSkeleton type="table" count={1} />;
  }

  if (!metrics) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-center">
        <ServerStackIcon className="w-12 h-12 text-gray-300 mb-4" />
        <Text className="text-gray-500">Unable to connect to gateway</Text>
      </div>
    );
  }

  const channels = Object.entries(metrics.channels)
    .sort(([, a], [, b]) => {
      if (a.lastMessageAt && !b.lastMessageAt) return -1;
      if (!a.lastMessageAt && b.lastMessageAt) return 1;
      if (a.lastMessageAt && b.lastMessageAt) {
        return new Date(b.lastMessageAt).getTime() - new Date(a.lastMessageAt).getTime();
      }
      return b.messagesReceived - a.messagesReceived;
    });

  const activeCount = channels.filter(([, s]) => s.lastMessageAt !== null).length;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5 }}
      className="space-y-6"
    >
      <div>
        <Title className="text-2xl">Sessions</Title>
        <Text className="text-gray-500">Active channel sessions and activity</Text>
      </div>

      <Grid numItems={2} numItemsLg={4} className="gap-4">
        <Card decoration="top" decorationColor="indigo">
          <Text>Total Channels</Text>
          <Metric className="text-xl">{channels.length}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Active</Text>
          <Metric className="text-xl">{activeCount}</Metric>
        </Card>
        <Card decoration="top" decorationColor="gray">
          <Text>Idle</Text>
          <Metric className="text-xl">{channels.length - activeCount}</Metric>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Total Messages</Text>
          <Metric className="text-xl">
            {metrics.totals.messagesReceived.toLocaleString()}
          </Metric>
        </Card>
      </Grid>

      <Card>
        <Title className="text-lg mb-4">Channel Sessions</Title>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-200 dark:border-gray-700">
                <th className="text-left py-2 px-3 text-gray-500 font-medium">Channel</th>
                <th className="text-left py-2 px-3 text-gray-500 font-medium">Status</th>
                <th className="text-right py-2 px-3 text-gray-500 font-medium">Messages</th>
                <th className="text-right py-2 px-3 text-gray-500 font-medium">Responses</th>
                <th className="text-right py-2 px-3 text-gray-500 font-medium">Errors</th>
                <th className="text-right py-2 px-3 text-gray-500 font-medium">Avg Response</th>
                <th className="text-left py-2 px-3 text-gray-500 font-medium">Last Active</th>
              </tr>
            </thead>
            <tbody>
              {channels.map(([name, stats]) => (
                <tr
                  key={name}
                  className="border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-900/50"
                >
                  <td className="py-3 px-3 font-medium">
                    {DISPLAY_NAMES[name] || name}
                  </td>
                  <td className="py-3 px-3">
                    <Badge
                      color={stats.lastMessageAt ? 'emerald' : 'gray'}
                      size="xs"
                    >
                      {stats.lastMessageAt ? 'Active' : 'Idle'}
                    </Badge>
                  </td>
                  <td className="py-3 px-3 text-right">{stats.messagesReceived}</td>
                  <td className="py-3 px-3 text-right">{stats.responsesSent}</td>
                  <td className="py-3 px-3 text-right">
                    <span className={stats.errors > 0 ? 'text-red-500' : ''}>
                      {stats.errors}
                    </span>
                  </td>
                  <td className="py-3 px-3 text-right">
                    {Math.round(stats.avgResponseMs)}ms
                  </td>
                  <td className="py-3 px-3 text-gray-500">
                    {stats.lastMessageAt
                      ? formatRelativeTime(stats.lastMessageAt)
                      : '-'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </motion.div>
  );
}
