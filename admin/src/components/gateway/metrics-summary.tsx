'use client';

import { MetricCard } from '@stateset/design';
import {
  ChatBubbleLeftEllipsisIcon,
  ArrowPathIcon,
  ExclamationTriangleIcon,
  ClockIcon,
  ServerIcon,
} from '@heroicons/react/24/outline';
import type { GatewayMetrics } from '@/lib/types/gateway';

type MetricTone = 'primary' | 'accent' | 'success' | 'warning' | 'danger';

interface MetricsSummaryProps {
  metrics: GatewayMetrics;
}

export function MetricsSummary({ metrics }: MetricsSummaryProps) {
  const { totals, uptime } = metrics;
  const errorRate =
    totals.messagesReceived > 0
      ? ((totals.errors / totals.messagesReceived) * 100).toFixed(2)
      : '0.00';

  const kpis: {
    label: string;
    value: string;
    icon: typeof ChatBubbleLeftEllipsisIcon;
    tone: MetricTone;
  }[] = [
    {
      label: 'Messages Received',
      value: totals.messagesReceived.toLocaleString(),
      icon: ChatBubbleLeftEllipsisIcon,
      tone: 'primary',
    },
    {
      label: 'Responses Sent',
      value: totals.responsesSent.toLocaleString(),
      icon: ArrowPathIcon,
      tone: 'accent',
    },
    {
      label: 'Errors',
      value: `${totals.errors.toLocaleString()} (${errorRate}%)`,
      icon: ExclamationTriangleIcon,
      tone: totals.errors > 0 ? 'danger' : 'success',
    },
    {
      label: 'Avg Response',
      value: `${Math.round(totals.avgResponseMs)}ms`,
      icon: ClockIcon,
      tone: totals.avgResponseMs > 2000 ? 'warning' : 'success',
    },
    {
      label: 'Uptime',
      value: uptime,
      icon: ServerIcon,
      tone: 'success',
    },
  ];

  return (
    <div className="grid grid-cols-2 lg:grid-cols-5 gap-4">
      {kpis.map((kpi) => (
        <MetricCard
          key={kpi.label}
          label={kpi.label}
          value={kpi.value}
          tone={kpi.tone}
          icon={kpi.icon}
        />
      ))}
    </div>
  );
}
