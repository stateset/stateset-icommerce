'use client';

import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  MetricCard,
  StatusPill,
  Badge,
  type StatusTone,
} from '@stateset/design';
import { CogIcon, BoltIcon, ArrowRightIcon, CheckCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { formatNumber } from '@/lib/utils';
import type {
  WorkflowBuilderData,
  Workflow,
  WorkflowTemplate,
  WorkflowExecution,
} from '@/lib/types/dashboard-data';

interface WorkflowBuilderProps {
  data?: WorkflowBuilderData;
}

const statusPillMap: Record<string, StatusTone> = {
  active: 'ok',
  paused: 'warn',
  draft: 'idle',
  error: 'fail',
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
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <MetricCard label="Active Workflows" value={summary.activeCount} tone="success" />
        <MetricCard
          label="Executions Today"
          value={formatNumber(summary.executionsToday)}
          tone="primary"
        />
        <MetricCard label="Success Rate" value={`${summary.successRate}%`} tone="accent" />
        <MetricCard label="Time Saved" value={`${summary.hoursSaved}h`} tone="warning" />
      </div>

      {/* Active Workflows */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Active Workflows</CardTitle>
              <CardDescription>
                Automated processes running on your commerce operations
              </CardDescription>
            </div>
            <StatusPill status="ok">{summary.activeCount} active</StatusPill>
          </div>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {workflows.map((workflow: Workflow, index: number) => (
              <motion.div
                key={workflow.id || index}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 border border-ds-enterprise-line rounded-lg hover:border-ds-brand-300 transition-colors"
              >
                <div className="flex items-start justify-between">
                  <div className="flex items-start space-x-4">
                    <div
                      className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                        workflow.status === 'active'
                          ? 'bg-ds-status-ok/15'
                          : workflow.status === 'paused'
                            ? 'bg-ds-status-warn/15'
                            : 'bg-ds-muted'
                      }`}
                    >
                      <CogIcon
                        className={`w-5 h-5 ${
                          workflow.status === 'active'
                            ? 'text-ds-status-ok'
                            : workflow.status === 'paused'
                              ? 'text-ds-status-warn'
                              : 'text-ds-muted-foreground'
                        }`}
                      />
                    </div>
                    <div>
                      <div className="flex items-center space-x-2">
                        <p className="text-sm font-medium text-ds-foreground">{workflow.name}</p>
                        <StatusPill status={statusPillMap[workflow.status] || 'idle'}>
                          {workflow.status}
                        </StatusPill>
                      </div>
                      <p className="text-sm text-ds-muted-foreground mt-1">
                        {workflow.description}
                      </p>

                      {/* Workflow Steps */}
                      <div className="flex items-center space-x-2 mt-3">
                        <Badge variant="primary">
                          {triggerIcons[workflow.trigger] || workflow.trigger}
                        </Badge>
                        <ArrowRightIcon className="w-4 h-4 text-ds-muted-foreground" />
                        {workflow.steps.map((step: string, i: number) => (
                          <div key={i} className="flex items-center space-x-2">
                            <span className="text-xs px-2 py-1 bg-ds-muted rounded text-ds-foreground">
                              {step}
                            </span>
                            {i < workflow.steps.length - 1 && (
                              <ArrowRightIcon className="w-3 h-3 text-ds-muted-foreground" />
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>

                  <div className="text-right">
                    <div className="flex items-center space-x-2">
                      <p className="text-sm text-ds-muted-foreground">
                        {formatNumber(workflow.executions)} runs
                      </p>
                      <div className="flex items-center space-x-1">
                        <CheckCircleIcon className="w-4 h-4 text-ds-status-ok" />
                        <p className="text-sm text-ds-status-ok">{workflow.successRate}%</p>
                      </div>
                    </div>
                    <p className="text-xs text-ds-muted-foreground mt-1">
                      Last run: {workflow.lastRun}
                    </p>
                  </div>
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Workflow Templates */}
      <Card>
        <CardHeader>
          <CardTitle>Quick Start Templates</CardTitle>
          <CardDescription>Pre-built workflows for common commerce operations</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {templates.map((template: WorkflowTemplate, index: number) => (
              <motion.div
                key={template.id || index}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 border border-ds-enterprise-line rounded-lg hover:border-ds-brand-300 cursor-pointer transition-colors"
              >
                <div className="flex items-center space-x-3 mb-2">
                  <div className="w-8 h-8 rounded-lg bg-ds-brand-100 flex items-center justify-center">
                    <BoltIcon className="w-4 h-4 text-ds-primary" />
                  </div>
                  <p className="text-sm font-medium text-ds-foreground">{template.name}</p>
                </div>
                <p className="text-sm text-ds-muted-foreground">{template.description}</p>
                <div className="flex items-center justify-between mt-3">
                  <Badge variant="default">{template.category}</Badge>
                  <p className="text-xs text-ds-primary">
                    {formatNumber(template.usedBy)} businesses use this
                  </p>
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Execution History */}
      <Card>
        <CardHeader>
          <CardTitle>Recent Executions</CardTitle>
          <CardDescription>Latest workflow runs and their outcomes</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-ds-enterprise-line">
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Workflow
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Trigger
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
                {summary.recentExecutions?.map((execution: WorkflowExecution, index: number) => (
                  <tr key={index} className="border-b border-ds-enterprise-line">
                    <td className="py-2 px-3 text-sm font-medium text-ds-foreground">
                      {execution.workflow}
                    </td>
                    <td className="py-2 px-3 text-sm text-ds-muted-foreground">
                      {execution.trigger}
                    </td>
                    <td className="py-2 px-3">
                      <StatusPill status={execution.status === 'success' ? 'ok' : 'fail'}>
                        {execution.status}
                      </StatusPill>
                    </td>
                    <td className="py-2 px-3 text-sm text-right text-ds-foreground">
                      {execution.duration}
                    </td>
                    <td className="py-2 px-3 text-sm text-ds-muted-foreground">{execution.time}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
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
        {
          workflow: 'Order Confirmation',
          trigger: 'Order #1234',
          status: 'success',
          duration: '1.2s',
          time: '2 min ago',
        },
        {
          workflow: 'Low Stock Alert',
          trigger: 'SKU-789',
          status: 'success',
          duration: '0.8s',
          time: '5 min ago',
        },
        {
          workflow: 'Customer Welcome',
          trigger: 'New signup',
          status: 'success',
          duration: '2.1s',
          time: '8 min ago',
        },
        {
          workflow: 'Return Processing',
          trigger: 'RMA #456',
          status: 'success',
          duration: '3.5s',
          time: '12 min ago',
        },
        {
          workflow: 'Inventory Sync',
          trigger: 'Scheduled',
          status: 'failed',
          duration: '45s',
          time: '15 min ago',
        },
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
        description: "Engage customers who haven't purchased in 30 days",
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
      {
        id: '1',
        name: 'Abandoned Cart Recovery',
        description: 'Send reminder emails to customers with abandoned carts',
        category: 'Marketing',
        usedBy: 15420,
      },
      {
        id: '2',
        name: 'VIP Customer Recognition',
        description: 'Automatically upgrade customers based on purchase history',
        category: 'Customers',
        usedBy: 8930,
      },
      {
        id: '3',
        name: 'Fraud Detection Alert',
        description: 'Flag suspicious orders for manual review',
        category: 'Security',
        usedBy: 12100,
      },
      {
        id: '4',
        name: 'Review Request Automation',
        description: 'Request product reviews after delivery confirmation',
        category: 'Marketing',
        usedBy: 21500,
      },
      {
        id: '5',
        name: 'Subscription Renewal Reminder',
        description: 'Notify customers before subscription renewal',
        category: 'Subscriptions',
        usedBy: 6780,
      },
      {
        id: '6',
        name: 'Price Drop Notification',
        description: 'Alert customers when wishlist items go on sale',
        category: 'Marketing',
        usedBy: 9450,
      },
    ],
  };
}
