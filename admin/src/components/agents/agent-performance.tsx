'use client';

import { memo } from 'react';
import { AreaChart, BarChart, DonutChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  Badge,
  StatusPill,
  MetricCard,
} from '@stateset/design';
import { CpuChipIcon, CheckCircleIcon, XCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getAgentPerformanceData } from '@/app/actions/commerce';
import { SimulatedDataBadge } from '@/components/shared/simulated-data-badge';
import { formatMeasured, formatNumber, formatRatio } from '@/lib/utils';
import type { AgentPerformanceData, Agent, RecentTask } from '@/lib/types/dashboard-data';

interface AgentPerformanceProps {
  data?: AgentPerformanceData;
}

type AgentStatusKind = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';

const statusPillMap: Record<string, AgentStatusKind> = {
  online: 'ok',
  busy: 'warn',
  offline: 'idle',
  error: 'fail',
};

function AgentPerformanceInner({ data: propData }: AgentPerformanceProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getAgentPerformanceData(), {
    initialData: propData,
    refreshInterval: 15000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-64 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/25">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load agent performance data</p>
        </CardContent>
      </Card>
    );
  }

  const { summary, agents, taskMetrics, responseTimeTrend } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        {/* A missing figure reads as unavailable, never as a stand-in number. */}
        <MetricCard
          label="Active Agents"
          value={formatMeasured(summary?.activeAgents, String)}
          tone="success"
          subtitle={
            summary?.onlinePercentage === undefined ? '' : `${summary.onlinePercentage}% online`
          }
        />
        <MetricCard
          label="Tasks Completed"
          value={formatMeasured(summary?.tasksCompleted, formatNumber)}
          tone="primary"
        />
        <MetricCard
          label="Avg Response Time"
          value={formatMeasured(summary?.avgResponseTime, (seconds) => `${seconds}s`)}
          tone="warning"
        />
        <MetricCard
          label="Success Rate"
          value={formatMeasured(summary?.successRate, (rate) => formatRatio(rate))}
          tone="accent"
        />
      </div>

      {/* Response Time Trend.
          The hourly avg/p95/p99 series is generated deterministically in
          getAgentPerformanceData — there is no real latency telemetry yet,
          so this chart must carry the simulated-data badge. */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Response Time Trend</CardTitle>
            <SimulatedDataBadge />
          </div>
          <CardDescription>Agent response times over the last 24 hours</CardDescription>
        </CardHeader>
        <CardContent>
          <AreaChart
            className="h-64"
            data={responseTimeTrend ?? []}
            index="time"
            categories={['avgTime', 'p95Time', 'p99Time']}
            colors={['emerald', 'amber', 'indigo']}
            showAnimation
            curveType="monotone"
            valueFormatter={(value) => `${value}ms`}
          />
        </CardContent>
      </Card>

      {/* Agent Status Grid */}
      <Card>
        <CardHeader>
          <CardTitle>Agent Status</CardTitle>
          <CardDescription>Real-time status of all AI agents</CardDescription>
        </CardHeader>
        <CardContent>
          {(agents ?? []).length === 0 && (
            <p className="text-sm text-ds-muted-foreground">No agent activity reported</p>
          )}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {(agents ?? []).map((agent: Agent, index: number) => (
              <motion.div
                key={agent.id || index}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 border border-ds-enterprise-line rounded-lg"
              >
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center space-x-2">
                    <CpuChipIcon className="w-5 h-5 text-ds-primary" />
                    <p className="text-sm font-medium text-ds-foreground">{agent.name}</p>
                  </div>
                  <StatusPill status={statusPillMap[agent.status] || 'idle'}>
                    {agent.status}
                  </StatusPill>
                </div>
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <p className="text-sm text-ds-muted-foreground">Tasks</p>
                    <p className="text-sm text-ds-foreground">
                      {formatNumber(agent.tasksCompleted)}
                    </p>
                  </div>
                  <div className="flex justify-between text-sm">
                    <p className="text-sm text-ds-muted-foreground">Success</p>
                    <p className="text-sm text-ds-foreground">{formatRatio(agent.successRate)}</p>
                  </div>
                  <div className="flex justify-between text-sm">
                    <p className="text-sm text-ds-muted-foreground">Avg Time</p>
                    <p className="text-sm text-ds-foreground">{agent.avgResponseTime}ms</p>
                  </div>
                  <ProgressBar
                    value={agent.utilization * 100}
                    color={
                      agent.utilization > 0.8
                        ? 'red'
                        : agent.utilization > 0.6
                          ? 'amber'
                          : 'emerald'
                    }
                  />
                  <p className="text-xs text-ds-muted-foreground text-center">
                    {formatRatio(agent.utilization)} utilization
                  </p>
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Task Metrics */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Task Distribution */}
        <Card>
          <CardHeader>
            <CardTitle>Task Distribution</CardTitle>
            <CardDescription>Tasks by type</CardDescription>
          </CardHeader>
          <CardContent>
            <DonutChart
              className="h-64"
              data={taskMetrics?.distribution ?? []}
              category="count"
              index="type"
              colors={['indigo', 'emerald', 'violet', 'amber', 'cyan']}
              showAnimation
            />
          </CardContent>
        </Card>

        {/* Task Success/Failure.
            Daily success/failed/timeout counts are deterministic demo values
            (no per-task outcome history exists in the engine yet). */}
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle>Task Outcomes</CardTitle>
              <SimulatedDataBadge />
            </div>
            <CardDescription>Last 7 days performance</CardDescription>
          </CardHeader>
          <CardContent>
            <BarChart
              className="h-64"
              data={taskMetrics?.dailyOutcomes ?? []}
              index="day"
              categories={['success', 'failed', 'timeout']}
              colors={['emerald', 'indigo', 'amber']}
              stack
              showAnimation
            />
          </CardContent>
        </Card>
      </div>

      {/* Recent Tasks */}
      <Card>
        <CardHeader>
          <CardTitle>Recent Agent Tasks</CardTitle>
          <CardDescription>Latest tasks processed by AI agents</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-ds-enterprise-line">
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Task ID
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Agent
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Type
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Status
                  </th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Duration
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Time
                  </th>
                </tr>
              </thead>
              <tbody>
                {(taskMetrics?.recentTasks ?? []).length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-4 px-3 text-sm text-ds-muted-foreground">
                      No recent agent tasks
                    </td>
                  </tr>
                )}
                {(taskMetrics?.recentTasks ?? []).map((task: RecentTask, index: number) => (
                  <motion.tr
                    key={task.id || index}
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: index * 0.03 }}
                    className="border-b border-ds-enterprise-line hover:bg-ds-muted"
                  >
                    <td className="py-2 px-3 text-sm font-mono text-ds-foreground">{task.id}</td>
                    <td className="py-2 px-3">
                      <div className="flex items-center space-x-2">
                        <CpuChipIcon className="w-4 h-4 text-ds-primary" />
                        <p className="text-sm text-ds-foreground">{task.agent}</p>
                      </div>
                    </td>
                    <td className="py-2 px-3">
                      <Badge variant="primary">{task.type}</Badge>
                    </td>
                    <td className="py-2 px-3">
                      {task.status === 'success' ? (
                        <div className="flex items-center space-x-1">
                          <CheckCircleIcon className="w-4 h-4 text-ds-status-ok" />
                          <p className="text-sm text-ds-status-ok">Success</p>
                        </div>
                      ) : task.status === 'failed' ? (
                        <div className="flex items-center space-x-1">
                          <XCircleIcon className="w-4 h-4 text-ds-status-fail" />
                          <p className="text-sm text-ds-status-fail">Failed</p>
                        </div>
                      ) : (
                        <Badge variant="warning">{task.status}</Badge>
                      )}
                    </td>
                    <td className="py-2 px-3 text-sm text-right text-ds-foreground">
                      {task.duration}ms
                    </td>
                    <td className="py-2 px-3 text-sm text-ds-muted-foreground">{task.timestamp}</td>
                  </motion.tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

const AgentPerformance = memo(AgentPerformanceInner);
export default AgentPerformance;
