'use client';

import { Card, Title, Text, Badge, Grid, Metric } from '@tremor/react';
import { CogIcon, BoltIcon, ArrowRightIcon, CheckCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { formatNumber } from '@/lib/utils';
import type { WorkflowBuilderData, Workflow, WorkflowTemplate, WorkflowExecution, TremorColor } from '@/lib/types/dashboard-data';

interface WorkflowBuilderProps {
  data?: WorkflowBuilderData;
}

const statusColors: Record<string, string> = {
  active: 'emerald',
  paused: 'amber',
  draft: 'gray',
  error: 'red',
};

const triggerIcons: Record<string, string> = {
  order: 'Order created',
  inventory: 'Stock level changed',
  customer: 'Customer action',
  time: 'Scheduled time',
  webhook: 'External webhook',
};

export default function WorkflowBuilder({ data: propData }: WorkflowBuilderProps) {
  // Demo data - in production this would come from embedded API
  const data: WorkflowBuilderData = propData || generateDemoData();

  const { summary, workflows, templates } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <Text>Active Workflows</Text>
          <Metric>{summary.activeCount}</Metric>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Executions Today</Text>
          <Metric>{formatNumber(summary.executionsToday)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Success Rate</Text>
          <Metric>{summary.successRate}%</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Time Saved</Text>
          <Metric>{summary.hoursSaved}h</Metric>
        </Card>
      </Grid>

      {/* Active Workflows */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Active Workflows</Title>
            <Text className="text-gray-500">Automated processes running on your commerce operations</Text>
          </div>
          <Badge color="emerald" size="lg">
            {summary.activeCount} active
          </Badge>
        </div>

        <div className="space-y-4">
          {workflows.map((workflow: Workflow, index: number) => (
            <motion.div
              key={workflow.id || index}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.05 }}
              className="p-4 border rounded-lg dark:border-gray-700 hover:border-indigo-300 dark:hover:border-indigo-700 transition-colors"
            >
              <div className="flex items-start justify-between">
                <div className="flex items-start space-x-4">
                  <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                    workflow.status === 'active' ? 'bg-emerald-100 dark:bg-emerald-900/30' :
                    workflow.status === 'paused' ? 'bg-amber-100 dark:bg-amber-900/30' :
                    'bg-gray-100 dark:bg-gray-800'
                  }`}>
                    <CogIcon className={`w-5 h-5 ${
                      workflow.status === 'active' ? 'text-emerald-600' :
                      workflow.status === 'paused' ? 'text-amber-600' :
                      'text-gray-500'
                    }`} />
                  </div>
                  <div>
                    <div className="flex items-center space-x-2">
                      <Text className="font-medium">{workflow.name}</Text>
                      <Badge color={statusColors[workflow.status] as TremorColor || 'gray'} size="xs">
                        {workflow.status}
                      </Badge>
                    </div>
                    <Text className="text-sm text-gray-500 mt-1">{workflow.description}</Text>

                    {/* Workflow Steps */}
                    <div className="flex items-center space-x-2 mt-3">
                      <Badge color="blue" size="xs">
                        {triggerIcons[workflow.trigger] || workflow.trigger}
                      </Badge>
                      <ArrowRightIcon className="w-4 h-4 text-gray-400" />
                      {workflow.steps.map((step: string, i: number) => (
                        <div key={i} className="flex items-center space-x-2">
                          <Text className="text-xs px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded">
                            {step}
                          </Text>
                          {i < workflow.steps.length - 1 && (
                            <ArrowRightIcon className="w-3 h-3 text-gray-400" />
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                </div>

                <div className="text-right">
                  <div className="flex items-center space-x-2">
                    <Text className="text-sm text-gray-500">
                      {formatNumber(workflow.executions)} runs
                    </Text>
                    <div className="flex items-center space-x-1">
                      <CheckCircleIcon className="w-4 h-4 text-emerald-500" />
                      <Text className="text-sm text-emerald-600">{workflow.successRate}%</Text>
                    </div>
                  </div>
                  <Text className="text-xs text-gray-500 mt-1">
                    Last run: {workflow.lastRun}
                  </Text>
                </div>
              </div>
            </motion.div>
          ))}
        </div>
      </Card>

      {/* Workflow Templates */}
      <Card>
        <Title>Quick Start Templates</Title>
        <Text className="text-gray-500 mb-4">Pre-built workflows for common commerce operations</Text>

        <Grid numItems={1} numItemsSm={2} numItemsLg={3} className="gap-4">
          {templates.map((template: WorkflowTemplate, index: number) => (
            <motion.div
              key={template.id || index}
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: index * 0.05 }}
              className="p-4 border rounded-lg dark:border-gray-700 hover:border-indigo-300 dark:hover:border-indigo-700 cursor-pointer transition-colors"
            >
              <div className="flex items-center space-x-3 mb-2">
                <div className="w-8 h-8 rounded-lg bg-indigo-100 dark:bg-indigo-900/30 flex items-center justify-center">
                  <BoltIcon className="w-4 h-4 text-indigo-600" />
                </div>
                <Text className="font-medium">{template.name}</Text>
              </div>
              <Text className="text-sm text-gray-500">{template.description}</Text>
              <div className="flex items-center justify-between mt-3">
                <Badge color="gray" size="xs">{template.category}</Badge>
                <Text className="text-xs text-indigo-600">
                  {formatNumber(template.usedBy)} businesses use this
                </Text>
              </div>
            </motion.div>
          ))}
        </Grid>
      </Card>

      {/* Execution History */}
      <Card>
        <Title>Recent Executions</Title>
        <Text className="text-gray-500 mb-4">Latest workflow runs and their outcomes</Text>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Workflow</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Trigger</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Status</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Duration</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Time</th>
              </tr>
            </thead>
            <tbody>
              {summary.recentExecutions?.map((execution: WorkflowExecution, index: number) => (
                <tr key={index} className="border-b dark:border-gray-700">
                  <td className="py-2 px-3 text-sm font-medium">{execution.workflow}</td>
                  <td className="py-2 px-3 text-sm text-gray-500">{execution.trigger}</td>
                  <td className="py-2 px-3">
                    <Badge color={execution.status === 'success' ? 'emerald' : 'red'} size="xs">
                      {execution.status}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-sm text-right">{execution.duration}</td>
                  <td className="py-2 px-3 text-sm text-gray-500">{execution.time}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </motion.div>
  );
}

function generateDemoData(): WorkflowBuilderData {
  return {
    summary: {
      activeCount: 8,
      executionsToday: 1247,
      successRate: 98.5,
      hoursSaved: 42,
      recentExecutions: [
        { workflow: 'Order Confirmation', trigger: 'Order #1234', status: 'success', duration: '1.2s', time: '2 min ago' },
        { workflow: 'Low Stock Alert', trigger: 'SKU-789', status: 'success', duration: '0.8s', time: '5 min ago' },
        { workflow: 'Customer Welcome', trigger: 'New signup', status: 'success', duration: '2.1s', time: '8 min ago' },
        { workflow: 'Return Processing', trigger: 'RMA #456', status: 'success', duration: '3.5s', time: '12 min ago' },
        { workflow: 'Inventory Sync', trigger: 'Scheduled', status: 'failed', duration: '45s', time: '15 min ago' },
      ],
    },
    workflows: [
      {
        id: '1',
        name: 'Order Confirmation & Fulfillment',
        description: 'Automatically confirm orders, reserve inventory, and initiate fulfillment',
        status: 'active',
        trigger: 'order',
        steps: ['Validate', 'Reserve Stock', 'Send Email', 'Create Shipment'],
        executions: 12450,
        successRate: 99.2,
        lastRun: '2 min ago',
      },
      {
        id: '2',
        name: 'Low Stock Auto-Reorder',
        description: 'Monitor inventory levels and create purchase orders when below threshold',
        status: 'active',
        trigger: 'inventory',
        steps: ['Check Level', 'Calculate Qty', 'Create PO'],
        executions: 890,
        successRate: 97.8,
        lastRun: '15 min ago',
      },
      {
        id: '3',
        name: 'Customer Win-Back Campaign',
        description: 'Engage customers who haven\'t purchased in 30 days',
        status: 'active',
        trigger: 'time',
        steps: ['Find Customers', 'Generate Offer', 'Send Email'],
        executions: 2100,
        successRate: 95.5,
        lastRun: '1 hour ago',
      },
      {
        id: '4',
        name: 'Return Processing Automation',
        description: 'Auto-approve eligible returns and initiate refund processing',
        status: 'paused',
        trigger: 'customer',
        steps: ['Validate RMA', 'Check Policy', 'Process Refund'],
        executions: 3450,
        successRate: 98.1,
        lastRun: '3 hours ago',
      },
    ],
    templates: [
      { id: '1', name: 'Abandoned Cart Recovery', description: 'Send reminder emails to customers with abandoned carts', category: 'Marketing', usedBy: 15420 },
      { id: '2', name: 'VIP Customer Recognition', description: 'Automatically upgrade customers based on purchase history', category: 'Customers', usedBy: 8930 },
      { id: '3', name: 'Fraud Detection Alert', description: 'Flag suspicious orders for manual review', category: 'Security', usedBy: 12100 },
      { id: '4', name: 'Review Request Automation', description: 'Request product reviews after delivery confirmation', category: 'Marketing', usedBy: 21500 },
      { id: '5', name: 'Subscription Renewal Reminder', description: 'Notify customers before subscription renewal', category: 'Subscriptions', usedBy: 6780 },
      { id: '6', name: 'Price Drop Notification', description: 'Alert customers when wishlist items go on sale', category: 'Marketing', usedBy: 9450 },
    ],
  };
}
