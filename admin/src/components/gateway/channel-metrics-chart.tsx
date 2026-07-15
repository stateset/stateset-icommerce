'use client';

import { BarChart, DonutChart } from '@tremor/react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@stateset/design';
import type { GatewayMetrics } from '@/lib/types/gateway';

interface ChannelMetricsChartProps {
  metrics: GatewayMetrics;
}

export function ChannelMetricsChart({ metrics }: ChannelMetricsChartProps) {
  const channelData = Object.entries(metrics.channels).map(([name, stats]) => ({
    channel: name,
    'Messages Received': stats.messagesReceived,
    'Responses Sent': stats.responsesSent,
    Errors: stats.errors,
  }));

  const trafficDistribution = Object.entries(metrics.channels)
    .map(([name, stats]) => ({ name, value: stats.messagesReceived }))
    .filter((d) => d.value > 0);

  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Messages by Channel</CardTitle>
          <CardDescription>Received vs Sent per channel</CardDescription>
        </CardHeader>
        <CardContent>
          {channelData.length > 0 ? (
            <BarChart
              className="h-72"
              data={channelData}
              index="channel"
              categories={['Messages Received', 'Responses Sent', 'Errors']}
              colors={['indigo', 'emerald', 'violet']}
              showAnimation
            />
          ) : (
            <div className="h-72 flex items-center justify-center">
              <p className="text-sm text-ds-muted-foreground">No channel data available</p>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Traffic Distribution</CardTitle>
          <CardDescription>Share of messages by channel</CardDescription>
        </CardHeader>
        <CardContent>
          {trafficDistribution.length > 0 ? (
            <DonutChart
              className="h-72"
              data={trafficDistribution}
              category="value"
              index="name"
              colors={['indigo', 'emerald', 'violet', 'amber', 'cyan']}
              showAnimation
            />
          ) : (
            <div className="h-72 flex items-center justify-center">
              <p className="text-sm text-ds-muted-foreground">No traffic data available</p>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
