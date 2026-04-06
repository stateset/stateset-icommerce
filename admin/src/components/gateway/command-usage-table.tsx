'use client';

import { Card, Title, Text, BarChart } from '@tremor/react';

interface CommandUsageTableProps {
  commandUsage: Record<string, number>;
}

export function CommandUsageTable({ commandUsage }: CommandUsageTableProps) {
  const data = Object.entries(commandUsage)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 20)
    .map(([command, count]) => ({ command, count }));

  if (data.length === 0) {
    return (
      <Card>
        <Title>Command Usage</Title>
        <div className="h-48 flex items-center justify-center">
          <Text className="text-gray-400">No commands executed yet</Text>
        </div>
      </Card>
    );
  }

  return (
    <Card>
      <Title>Top Commands</Title>
      <Text className="text-gray-500 mb-4">Most used commands (top 20)</Text>
      <BarChart
        className="h-72"
        data={data}
        index="command"
        categories={['count']}
        colors={['indigo']}
        layout="vertical"
        showAnimation
      />
    </Card>
  );
}
