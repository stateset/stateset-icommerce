'use client';

import { BarChart } from '@tremor/react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@stateset/design';

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
        <CardHeader>
          <CardTitle>Command Usage</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-48 flex items-center justify-center">
            <p className="text-sm text-ds-muted-foreground">No commands executed yet</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Top Commands</CardTitle>
        <CardDescription>Most used commands (top 20)</CardDescription>
      </CardHeader>
      <CardContent>
        <BarChart
          className="h-72"
          data={data}
          index="command"
          categories={['count']}
          colors={['indigo']}
          layout="vertical"
          showAnimation
        />
      </CardContent>
    </Card>
  );
}
