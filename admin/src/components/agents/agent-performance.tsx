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
import { formatNumber, formatPercentage } from '@/lib/utils';
import type {
  AgentPerformanceData,
  Agent,
  DailyOutcomeEntry,
  RecentTask,
  TaskDistributionEntry,
} from '@/lib/types/dashboard-data';

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
        <MetricCard
          label="Active Agents"
          value={summary?.activeAgents || 8}
          tone="success"
          subtitle={`${summary?.onlinePercentage || 95}% online`}
        />
        <MetricCard
          label="Tasks Completed"
          value={formatNumber(summary?.tasksCompleted || 12450)}
          tone="primary"
        />
        <MetricCard
          label="Avg Response Time"
          value={`${summary?.avgResponseTime || 1.2}s`}
          tone="warning"
        />
        <MetricCard
          label="Success Rate"
          value={formatPercentage(summary?.successRate || 0.984)}
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
            data={responseTimeTrend || generateDemoResponseTimeTrend()}
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
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {(agents || generateDemoAgents()).map((agent: Agent, index: number) => (
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
                    <p className="text-sm text-ds-foreground">
                      {formatPercentage(agent.successRate)}
                    </p>
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
                    {formatPercentage(agent.utilization)} utilization
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
              data={taskMetrics?.distribution || generateDemoTaskDistribution()}
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
              data={taskMetrics?.dailyOutcomes || generateDemoTaskOutcomes()}
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
                {(taskMetrics?.recentTasks || generateDemoRecentTasks()).map(
                  (task: RecentTask, index: number) => (
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
                      <td className="py-2 px-3 text-sm text-ds-muted-foreground">
                        {task.timestamp}
                      </td>
                    </motion.tr>
                  ),
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

// Demo data generators
function generateDemoResponseTimeTrend(): NonNullable<AgentPerformanceData['responseTimeTrend']> {
  const data = [];
  for (let i = 0; i < 24; i++) {
    data.push({
      time: `${i}:00`,
      avgTime: 800 + Math.random() * 400,
      p95Time: 1200 + Math.random() * 600,
      p99Time: 1800 + Math.random() * 800,
    });
  }
  return data;
}

function generateDemoAgents(): Agent[] {
  return [
    {
      id: '1',
      name: 'Order Agent',
      status: 'online',
      tasksCompleted: 3245,
      successRate: 0.992,
      avgResponseTime: 850,
      utilization: 0.72,
    },
    {
      id: '2',
      name: 'Inventory Agent',
      status: 'online',
      tasksCompleted: 2890,
      successRate: 0.988,
      avgResponseTime: 920,
      utilization: 0.65,
    },
    {
      id: '3',
      name: 'Returns Agent',
      status: 'busy',
      tasksCompleted: 1560,
      successRate: 0.975,
      avgResponseTime: 1100,
      utilization: 0.88,
    },
    {
      id: '4',
      name: 'Customer Agent',
      status: 'online',
      tasksCompleted: 2100,
      successRate: 0.981,
      avgResponseTime: 780,
      utilization: 0.55,
    },
    {
      id: '5',
      name: 'Analytics Agent',
      status: 'online',
      tasksCompleted: 1890,
      successRate: 0.995,
      avgResponseTime: 1250,
      utilization: 0.42,
    },
    {
      id: '6',
      name: 'Support Agent',
      status: 'online',
      tasksCompleted: 2450,
      successRate: 0.968,
      avgResponseTime: 650,
      utilization: 0.78,
    },
    {
      id: '7',
      name: 'Pricing Agent',
      status: 'offline',
      tasksCompleted: 980,
      successRate: 0.991,
      avgResponseTime: 450,
      utilization: 0,
    },
    {
      id: '8',
      name: 'Fulfillment Agent',
      status: 'online',
      tasksCompleted: 3100,
      successRate: 0.986,
      avgResponseTime: 890,
      utilization: 0.68,
    },
  ];
}

function generateDemoTaskDistribution(): TaskDistributionEntry[] {
  return [
    { type: 'Order Processing', count: 3245 },
    { type: 'Inventory Check', count: 2890 },
    { type: 'Customer Query', count: 2100 },
    { type: 'Returns Processing', count: 1560 },
    { type: 'Analytics', count: 1890 },
  ];
}

function generateDemoTaskOutcomes(): DailyOutcomeEntry[] {
  return [
    { day: 'Mon', success: 1850, failed: 32, timeout: 15 },
    { day: 'Tue', success: 1920, failed: 28, timeout: 12 },
    { day: 'Wed', success: 1780, failed: 35, timeout: 18 },
    { day: 'Thu', success: 2100, failed: 22, timeout: 10 },
    { day: 'Fri', success: 2250, failed: 18, timeout: 8 },
    { day: 'Sat', success: 1450, failed: 25, timeout: 14 },
    { day: 'Sun', success: 1200, failed: 20, timeout: 11 },
  ];
}

function generateDemoRecentTasks(): RecentTask[] {
  return [
    {
      id: 'TSK-001',
      agent: 'Order Agent',
      type: 'order.process',
      status: 'success',
      duration: 856,
      timestamp: '2 min ago',
    },
    {
      id: 'TSK-002',
      agent: 'Customer Agent',
      type: 'customer.query',
      status: 'success',
      duration: 423,
      timestamp: '3 min ago',
    },
    {
      id: 'TSK-003',
      agent: 'Returns Agent',
      type: 'return.approve',
      status: 'success',
      duration: 1120,
      timestamp: '5 min ago',
    },
    {
      id: 'TSK-004',
      agent: 'Inventory Agent',
      type: 'stock.check',
      status: 'failed',
      duration: 2500,
      timestamp: '6 min ago',
    },
    {
      id: 'TSK-005',
      agent: 'Analytics Agent',
      type: 'report.generate',
      status: 'success',
      duration: 1890,
      timestamp: '8 min ago',
    },
    {
      id: 'TSK-006',
      agent: 'Fulfillment Agent',
      type: 'shipment.track',
      status: 'success',
      duration: 650,
      timestamp: '10 min ago',
    },
  ];
}

const AgentPerformance = memo(AgentPerformanceInner);
export default AgentPerformance;
