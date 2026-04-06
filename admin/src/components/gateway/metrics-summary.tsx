'use client';

import { Card, Metric, Text, Grid } from '@tremor/react';
import {
  ChatBubbleLeftEllipsisIcon,
  ArrowPathIcon,
  ExclamationTriangleIcon,
  ClockIcon,
  ServerIcon,
} from '@heroicons/react/24/outline';
import type { GatewayMetrics } from '@/lib/types/gateway';

interface MetricsSummaryProps {
  metrics: GatewayMetrics;
}

export function MetricsSummary({ metrics }: MetricsSummaryProps) {
  const { totals, uptime } = metrics;
  const errorRate =
    totals.messagesReceived > 0
      ? ((totals.errors / totals.messagesReceived) * 100).toFixed(2)
      : '0.00';

  const kpis = [
    {
      label: 'Messages Received',
      value: totals.messagesReceived.toLocaleString(),
      icon: ChatBubbleLeftEllipsisIcon,
      color: 'indigo' as const,
    },
    {
      label: 'Responses Sent',
      value: totals.responsesSent.toLocaleString(),
      icon: ArrowPathIcon,
      color: 'blue' as const,
    },
    {
      label: 'Errors',
      value: `${totals.errors.toLocaleString()} (${errorRate}%)`,
      icon: ExclamationTriangleIcon,
      color: (totals.errors > 0 ? 'red' : 'emerald') as 'red' | 'emerald',
    },
    {
      label: 'Avg Response',
      value: `${Math.round(totals.avgResponseMs)}ms`,
      icon: ClockIcon,
      color: (totals.avgResponseMs > 2000 ? 'amber' : 'emerald') as 'amber' | 'emerald',
    },
    {
      label: 'Uptime',
      value: uptime,
      icon: ServerIcon,
      color: 'emerald' as const,
    },
  ];

  return (
    <Grid numItems={2} numItemsLg={5} className="gap-4">
      {kpis.map((kpi) => (
        <Card key={kpi.label} decoration="top" decorationColor={kpi.color}>
          <div className="flex items-center justify-between">
            <div>
              <Text>{kpi.label}</Text>
              <Metric className="text-xl">{kpi.value}</Metric>
            </div>
            <kpi.icon className="w-8 h-8 text-gray-400" />
          </div>
        </Card>
      ))}
    </Grid>
  );
}
