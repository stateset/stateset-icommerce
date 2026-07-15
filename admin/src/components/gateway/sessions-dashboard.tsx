'use client';

import {
  Card,
  CardContent,
  MetricCard,
  StatusPill,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@stateset/design';
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
        <ServerStackIcon className="w-12 h-12 text-ds-muted-foreground mb-4" />
        <p className="text-sm text-ds-muted-foreground">Unable to connect to gateway</p>
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
        <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Sessions</h3>
        <p className="text-sm text-ds-muted-foreground">Active channel sessions and activity</p>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard label="Total Channels" value={channels.length} tone="primary" format="number" />
        <MetricCard label="Active" value={activeCount} tone="success" format="number" />
        <MetricCard label="Idle" value={channels.length - activeCount} format="number" />
        <MetricCard
          label="Total Messages"
          value={metrics.totals.messagesReceived}
          tone="accent"
          format="number"
        />
      </div>

      <Card>
        <CardContent>
          <h3 className="font-ds-display text-lg font-semibold text-ds-foreground mb-4">
            Channel Sessions
          </h3>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Channel</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right">Messages</TableHead>
                  <TableHead className="text-right">Responses</TableHead>
                  <TableHead className="text-right">Errors</TableHead>
                  <TableHead className="text-right">Avg Response</TableHead>
                  <TableHead>Last Active</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {channels.map(([name, stats]) => (
                  <TableRow key={name}>
                    <TableCell className="font-medium text-ds-foreground">
                      {DISPLAY_NAMES[name] || name}
                    </TableCell>
                    <TableCell>
                      <StatusPill status={stats.lastMessageAt ? 'ok' : 'idle'}>
                        {stats.lastMessageAt ? 'Active' : 'Idle'}
                      </StatusPill>
                    </TableCell>
                    <TableCell tone="numeric">{stats.messagesReceived}</TableCell>
                    <TableCell tone="numeric">{stats.responsesSent}</TableCell>
                    <TableCell tone="numeric">
                      <span className={stats.errors > 0 ? 'text-ds-status-fail' : ''}>
                        {stats.errors}
                      </span>
                    </TableCell>
                    <TableCell tone="numeric">
                      {Math.round(stats.avgResponseMs)}ms
                    </TableCell>
                    <TableCell className="text-ds-muted-foreground">
                      {stats.lastMessageAt
                        ? formatRelativeTime(stats.lastMessageAt)
                        : '-'}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}
