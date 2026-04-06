'use client';

import { Card, Title, Text, Badge, Grid, Metric, ProgressBar } from '@tremor/react';
import { ExclamationTriangleIcon, BoltIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import type { ExceptionManagementData, ExceptionItem, ExceptionResolution, TremorColor } from '@/lib/types/dashboard-data';

interface ExceptionManagementProps {
  data?: ExceptionManagementData;
}

const severityColors: Record<string, string> = {
  critical: 'red',
  high: 'amber',
  medium: 'blue',
  low: 'gray',
};

const statusColors: Record<string, string> = {
  open: 'red',
  investigating: 'amber',
  resolved: 'emerald',
  dismissed: 'gray',
};

export default function ExceptionManagement({ data: propData }: ExceptionManagementProps) {
  // Demo data - in production this would come from embedded API
  const data: ExceptionManagementData = propData || generateDemoData();

  const { summary, exceptions, recentResolutions } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="red">
          <Text>Open Exceptions</Text>
          <Metric>{summary.openCount}</Metric>
          <Text className="text-xs text-red-600 mt-1">
            {summary.criticalCount} critical
          </Text>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Investigating</Text>
          <Metric>{summary.investigatingCount}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Resolved Today</Text>
          <Metric>{summary.resolvedToday}</Metric>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Auto-Resolved</Text>
          <Metric>{summary.autoResolvedPercent}%</Metric>
        </Card>
      </Grid>

      {/* Severity Distribution */}
      <Card>
        <Title>Exception Severity Distribution</Title>
        <div className="mt-4 space-y-3">
          {Object.entries(summary.bySeverity || {}).map(([severity, count]) => {
            const total = summary.openCount || 1;
            const percentage = ((count as number) / total) * 100;
            return (
              <div key={severity}>
                <div className="flex justify-between mb-1">
                  <Text className="font-medium capitalize">{severity}</Text>
                  <div className="flex items-center space-x-2">
                    <Badge color={severityColors[severity] as TremorColor || 'gray'} size="xs">
                      {count as number}
                    </Badge>
                    <Text className="text-sm">{percentage.toFixed(1)}%</Text>
                  </div>
                </div>
                <ProgressBar
                  value={percentage}
                  color={severityColors[severity] as TremorColor || 'gray'}
                />
              </div>
            );
          })}
        </div>
      </Card>

      {/* Active Exceptions */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Active Exceptions</Title>
            <Text className="text-gray-500">Issues requiring attention</Text>
          </div>
          <Badge color="red" size="lg">
            {summary.openCount} open
          </Badge>
        </div>

        <div className="space-y-3">
          {exceptions.filter((e: ExceptionItem) => e.status !== 'resolved').slice(0, 8).map((exception: ExceptionItem, index: number) => (
            <motion.div
              key={exception.id || index}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: index * 0.05 }}
              className={`p-4 border rounded-lg dark:border-gray-700 ${
                exception.severity === 'critical' ? 'border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-900/10' :
                exception.severity === 'high' ? 'border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/10' :
                ''
              }`}
            >
              <div className="flex items-start justify-between">
                <div className="flex items-start space-x-3">
                  <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
                    exception.severity === 'critical' ? 'bg-red-100 dark:bg-red-900/30' :
                    exception.severity === 'high' ? 'bg-amber-100 dark:bg-amber-900/30' :
                    'bg-blue-100 dark:bg-blue-900/30'
                  }`}>
                    <ExclamationTriangleIcon className={`w-5 h-5 ${
                      exception.severity === 'critical' ? 'text-red-600' :
                      exception.severity === 'high' ? 'text-amber-600' :
                      'text-blue-600'
                    }`} />
                  </div>
                  <div>
                    <Text className="font-medium">{exception.title}</Text>
                    <Text className="text-sm text-gray-500">{exception.description}</Text>
                    <div className="flex items-center space-x-3 mt-2">
                      <Badge color={severityColors[exception.severity] as TremorColor || 'gray'} size="xs">
                        {exception.severity}
                      </Badge>
                      <Badge color={statusColors[exception.status] as TremorColor || 'gray'} size="xs">
                        {exception.status}
                      </Badge>
                      <Text className="text-xs text-gray-500">
                        {exception.category}
                      </Text>
                    </div>
                  </div>
                </div>
                <div className="text-right">
                  <Text className="text-xs text-gray-500">
                    {exception.timestamp}
                  </Text>
                  {exception.suggestedAction && (
                    <div className="mt-2 flex items-center space-x-1 text-indigo-600">
                      <BoltIcon className="w-3 h-3" />
                      <Text className="text-xs">Auto-fix available</Text>
                    </div>
                  )}
                </div>
              </div>
            </motion.div>
          ))}
        </div>
      </Card>

      {/* Recent Resolutions */}
      <Card>
        <Title>Recent Resolutions</Title>
        <Text className="text-gray-500 mb-4">Successfully resolved exceptions</Text>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Exception</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Resolution</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Method</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Time to Resolve</th>
              </tr>
            </thead>
            <tbody>
              {recentResolutions.map((resolution: ExceptionResolution, index: number) => (
                <motion.tr
                  key={resolution.id || index}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ delay: index * 0.03 }}
                  className="border-b dark:border-gray-700"
                >
                  <td className="py-2 px-3">
                    <Text className="font-medium">{resolution.title}</Text>
                  </td>
                  <td className="py-2 px-3 text-sm">{resolution.resolution}</td>
                  <td className="py-2 px-3">
                    <Badge color={resolution.method === 'auto' ? 'emerald' : 'blue'} size="xs">
                      {resolution.method === 'auto' ? 'Auto-resolved' : 'Manual'}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-sm text-right">{resolution.timeToResolve}</td>
                </motion.tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </motion.div>
  );
}

function generateDemoData(): ExceptionManagementData {
  return {
    summary: {
      openCount: 12,
      criticalCount: 2,
      investigatingCount: 4,
      resolvedToday: 18,
      autoResolvedPercent: 65,
      bySeverity: {
        critical: 2,
        high: 4,
        medium: 4,
        low: 2,
      },
    },
    exceptions: [
      { id: '1', title: 'Payment Processing Failed', description: 'Multiple payment attempts failing for Stripe gateway', severity: 'critical', status: 'investigating', category: 'Payments', timestamp: '5 min ago', suggestedAction: true },
      { id: '2', title: 'Inventory Sync Delayed', description: 'Warehouse inventory sync exceeded 15 minute threshold', severity: 'high', status: 'open', category: 'Inventory', timestamp: '12 min ago', suggestedAction: true },
      { id: '3', title: 'Order Fulfillment Stuck', description: '3 orders stuck in processing for over 2 hours', severity: 'high', status: 'investigating', category: 'Orders', timestamp: '25 min ago', suggestedAction: false },
      { id: '4', title: 'High Return Rate Alert', description: 'Product SKU-1234 return rate exceeds 15%', severity: 'medium', status: 'open', category: 'Returns', timestamp: '1 hour ago', suggestedAction: true },
      { id: '5', title: 'Customer Complaint Spike', description: 'Unusual increase in customer complaints detected', severity: 'medium', status: 'open', category: 'Support', timestamp: '2 hours ago', suggestedAction: false },
      { id: '6', title: 'Low Stock Warning', description: '5 popular items below safety stock levels', severity: 'low', status: 'open', category: 'Inventory', timestamp: '3 hours ago', suggestedAction: true },
    ],
    recentResolutions: [
      { id: '1', title: 'Database Connection Pool Exhausted', resolution: 'Increased pool size from 20 to 50', method: 'manual', timeToResolve: '15 min' },
      { id: '2', title: 'Shipping Label Generation Failed', resolution: 'Retried with backup carrier API', method: 'auto', timeToResolve: '2 min' },
      { id: '3', title: 'Duplicate Order Detection', resolution: 'Merged duplicate orders automatically', method: 'auto', timeToResolve: '30 sec' },
      { id: '4', title: 'Price Discrepancy Alert', resolution: 'Updated price from source of truth', method: 'auto', timeToResolve: '1 min' },
    ],
  };
}
