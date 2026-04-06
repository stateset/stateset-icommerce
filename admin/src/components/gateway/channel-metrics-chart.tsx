'use client';

import { Card, Title, Text, BarChart, DonutChart, Grid } from '@tremor/react';
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
    <Grid numItems={1} numItemsLg={2} className="gap-6">
      <Card>
        <Title>Messages by Channel</Title>
        <Text className="text-gray-500 mb-4">Received vs Sent per channel</Text>
        {channelData.length > 0 ? (
          <BarChart
            className="h-72"
            data={channelData}
            index="channel"
            categories={['Messages Received', 'Responses Sent', 'Errors']}
            colors={['indigo', 'blue', 'red']}
            showAnimation
          />
        ) : (
          <div className="h-72 flex items-center justify-center">
            <Text className="text-gray-400">No channel data available</Text>
          </div>
        )}
      </Card>

      <Card>
        <Title>Traffic Distribution</Title>
        <Text className="text-gray-500 mb-4">Share of messages by channel</Text>
        {trafficDistribution.length > 0 ? (
          <DonutChart
            className="h-72"
            data={trafficDistribution}
            category="value"
            index="name"
            colors={[
              'indigo',
              'blue',
              'purple',
              'emerald',
              'amber',
              'red',
              'pink',
              'teal',
              'cyan',
              'sky',
              'orange',
            ]}
            showAnimation
          />
        ) : (
          <div className="h-72 flex items-center justify-center">
            <Text className="text-gray-400">No traffic data available</Text>
          </div>
        )}
      </Card>
    </Grid>
  );
}
