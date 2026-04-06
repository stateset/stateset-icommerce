'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, AreaChart, BarChart, DonutChart, ProgressBar } from '@tremor/react';
import { CpuChipIcon, CheckCircleIcon, XCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getAgentPerformanceData } from '@/app/actions/commerce';
import { formatNumber, formatPercentage } from '@/lib/utils';
import type {
  AgentPerformanceData,
  Agent,
  DailyOutcomeEntry,
  RecentTask,
  TaskDistributionEntry,
  TremorColor,
} from '@/lib/types/dashboard-data';

interface AgentPerformanceProps {
  data?: AgentPerformanceData;
}

const statusColors: Record<string, string> = {
  online: 'emerald',
  busy: 'amber',
  offline: 'gray',
  error: 'red',
};

function AgentPerformanceInner({ data: propData }: AgentPerformanceProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getAgentPerformanceData(),
    { initialData: propData, refreshInterval: 15000 }
  );

  if (isLoading && !data) {
    return (
      <Card>
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-gray-200 rounded w-48" />
          <div className="h-64 bg-gray-200 rounded" />
        </div>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-red-200">
        <Text className="text-red-600">Failed to load agent performance data</Text>
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
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <Text>Active Agents</Text>
          <Metric>{summary?.activeAgents || 8}</Metric>
          <Text className="text-xs text-emerald-600 mt-1">
            {summary?.onlinePercentage || 95}% online
          </Text>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Tasks Completed</Text>
          <Metric>{formatNumber(summary?.tasksCompleted || 12450)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Avg Response Time</Text>
          <Metric>{summary?.avgResponseTime || 1.2}s</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Success Rate</Text>
          <Metric>{formatPercentage(summary?.successRate || 0.984)}</Metric>
        </Card>
      </Grid>

      {/* Response Time Trend */}
      <Card>
        <Title>Response Time Trend</Title>
        <Text className="text-gray-500 mb-4">Agent response times over the last 24 hours</Text>
        <AreaChart
          className="h-64"
          data={responseTimeTrend || generateDemoResponseTimeTrend()}
          index="time"
          categories={['avgTime', 'p95Time', 'p99Time']}
          colors={['emerald', 'amber', 'red']}
          showAnimation
          curveType="monotone"
          valueFormatter={(value) => `${value}ms`}
        />
      </Card>

      {/* Agent Status Grid */}
      <Card>
        <Title>Agent Status</Title>
        <Text className="text-gray-500 mb-4">Real-time status of all AI agents</Text>
        <Grid numItems={1} numItemsSm={2} numItemsLg={4} className="gap-4">
          {(agents || generateDemoAgents()).map((agent: Agent, index: number) => (
            <motion.div
              key={agent.id || index}
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: index * 0.05 }}
              className="p-4 border rounded-lg dark:border-gray-700"
            >
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center space-x-2">
                  <CpuChipIcon className="w-5 h-5 text-indigo-600" />
                  <Text className="font-medium">{agent.name}</Text>
                </div>
                <Badge color={statusColors[agent.status] as TremorColor || 'gray'} size="xs">
                  {agent.status}
                </Badge>
              </div>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <Text className="text-gray-500">Tasks</Text>
                  <Text>{formatNumber(agent.tasksCompleted)}</Text>
                </div>
                <div className="flex justify-between text-sm">
                  <Text className="text-gray-500">Success</Text>
                  <Text>{formatPercentage(agent.successRate)}</Text>
                </div>
                <div className="flex justify-between text-sm">
                  <Text className="text-gray-500">Avg Time</Text>
                  <Text>{agent.avgResponseTime}ms</Text>
                </div>
                <ProgressBar
                  value={agent.utilization * 100}
                  color={agent.utilization > 0.8 ? 'red' : agent.utilization > 0.6 ? 'amber' : 'emerald'}
                />
                <Text className="text-xs text-gray-500 text-center">
                  {formatPercentage(agent.utilization)} utilization
                </Text>
              </div>
            </motion.div>
          ))}
        </Grid>
      </Card>

      {/* Task Metrics */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        {/* Task Distribution */}
        <Card>
          <Title>Task Distribution</Title>
          <Text className="text-gray-500 mb-4">Tasks by type</Text>
          <DonutChart
            className="h-64"
            data={taskMetrics?.distribution || generateDemoTaskDistribution()}
            category="count"
            index="type"
            colors={['blue', 'emerald', 'amber', 'purple', 'red']}
            showAnimation
          />
        </Card>

        {/* Task Success/Failure */}
        <Card>
          <Title>Task Outcomes</Title>
          <Text className="text-gray-500 mb-4">Last 7 days performance</Text>
          <BarChart
            className="h-64"
            data={taskMetrics?.dailyOutcomes || generateDemoTaskOutcomes()}
            index="day"
            categories={['success', 'failed', 'timeout']}
            colors={['emerald', 'red', 'amber']}
            stack
            showAnimation
          />
        </Card>
      </Grid>

      {/* Recent Tasks */}
      <Card>
        <Title>Recent Agent Tasks</Title>
        <Text className="text-gray-500 mb-4">Latest tasks processed by AI agents</Text>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Task ID</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Agent</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Type</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Status</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Duration</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Time</th>
              </tr>
            </thead>
            <tbody>
              {(taskMetrics?.recentTasks || generateDemoRecentTasks()).map((task: RecentTask, index: number) => (
                <motion.tr
                  key={task.id || index}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: index * 0.03 }}
                  className="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                >
                  <td className="py-2 px-3 text-sm font-mono">{task.id}</td>
                  <td className="py-2 px-3">
                    <div className="flex items-center space-x-2">
                      <CpuChipIcon className="w-4 h-4 text-indigo-500" />
                      <Text className="text-sm">{task.agent}</Text>
                    </div>
                  </td>
                  <td className="py-2 px-3">
                    <Badge color="blue" size="xs">{task.type}</Badge>
                  </td>
                  <td className="py-2 px-3">
                    {task.status === 'success' ? (
                      <div className="flex items-center space-x-1">
                        <CheckCircleIcon className="w-4 h-4 text-emerald-500" />
                        <Text className="text-sm text-emerald-600">Success</Text>
                      </div>
                    ) : task.status === 'failed' ? (
                      <div className="flex items-center space-x-1">
                        <XCircleIcon className="w-4 h-4 text-red-500" />
                        <Text className="text-sm text-red-600">Failed</Text>
                      </div>
                    ) : (
                      <Badge color="amber" size="xs">{task.status}</Badge>
                    )}
                  </td>
                  <td className="py-2 px-3 text-sm text-right">{task.duration}ms</td>
                  <td className="py-2 px-3 text-sm text-gray-500">{task.timestamp}</td>
                </motion.tr>
              ))}
            </tbody>
          </table>
        </div>
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
    { id: '1', name: 'Order Agent', status: 'online', tasksCompleted: 3245, successRate: 0.992, avgResponseTime: 850, utilization: 0.72 },
    { id: '2', name: 'Inventory Agent', status: 'online', tasksCompleted: 2890, successRate: 0.988, avgResponseTime: 920, utilization: 0.65 },
    { id: '3', name: 'Returns Agent', status: 'busy', tasksCompleted: 1560, successRate: 0.975, avgResponseTime: 1100, utilization: 0.88 },
    { id: '4', name: 'Customer Agent', status: 'online', tasksCompleted: 2100, successRate: 0.981, avgResponseTime: 780, utilization: 0.55 },
    { id: '5', name: 'Analytics Agent', status: 'online', tasksCompleted: 1890, successRate: 0.995, avgResponseTime: 1250, utilization: 0.42 },
    { id: '6', name: 'Support Agent', status: 'online', tasksCompleted: 2450, successRate: 0.968, avgResponseTime: 650, utilization: 0.78 },
    { id: '7', name: 'Pricing Agent', status: 'offline', tasksCompleted: 980, successRate: 0.991, avgResponseTime: 450, utilization: 0 },
    { id: '8', name: 'Fulfillment Agent', status: 'online', tasksCompleted: 3100, successRate: 0.986, avgResponseTime: 890, utilization: 0.68 },
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
    { id: 'TSK-001', agent: 'Order Agent', type: 'order.process', status: 'success', duration: 856, timestamp: '2 min ago' },
    { id: 'TSK-002', agent: 'Customer Agent', type: 'customer.query', status: 'success', duration: 423, timestamp: '3 min ago' },
    { id: 'TSK-003', agent: 'Returns Agent', type: 'return.approve', status: 'success', duration: 1120, timestamp: '5 min ago' },
    { id: 'TSK-004', agent: 'Inventory Agent', type: 'stock.check', status: 'failed', duration: 2500, timestamp: '6 min ago' },
    { id: 'TSK-005', agent: 'Analytics Agent', type: 'report.generate', status: 'success', duration: 1890, timestamp: '8 min ago' },
    { id: 'TSK-006', agent: 'Fulfillment Agent', type: 'shipment.track', status: 'success', duration: 650, timestamp: '10 min ago' },
  ];
}

const AgentPerformance = memo(AgentPerformanceInner);
export default AgentPerformance;
