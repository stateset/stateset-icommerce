'use client';

import { ProgressBar, type Color } from '@tremor/react';
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
import { ExclamationTriangleIcon, BoltIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import type {
  ExceptionManagementData,
  ExceptionItem,
  ExceptionResolution,
} from '@/lib/types/dashboard-data';

interface ExceptionManagementProps {
  data?: ExceptionManagementData;
}

const severityProgressColors: Record<string, Color> = {
  critical: 'red',
  high: 'amber',
  medium: 'indigo',
  low: 'gray',
};

const severityStatus: Record<string, StatusTone> = {
  critical: 'fail',
  high: 'warn',
  medium: 'run',
  low: 'idle',
};

const statusPillMap: Record<string, StatusTone> = {
  open: 'fail',
  investigating: 'warn',
  resolved: 'ok',
  dismissed: 'idle',
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
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <MetricCard
          label="Open Exceptions"
          value={summary.openCount}
          subtitle={`${summary.criticalCount} critical`}
          tone="danger"
        />
        <MetricCard label="Investigating" value={summary.investigatingCount} tone="warning" />
        <MetricCard label="Resolved Today" value={summary.resolvedToday} tone="success" />
        <MetricCard
          label="Auto-Resolved"
          value={`${summary.autoResolvedPercent}%`}
          tone="primary"
        />
      </div>

      {/* Severity Distribution */}
      <Card>
        <CardHeader>
          <CardTitle>Exception Severity Distribution</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {Object.entries(summary.bySeverity || {}).map(([severity, count]) => {
              const total = summary.openCount || 1;
              const percentage = ((count as number) / total) * 100;
              return (
                <div key={severity}>
                  <div className="flex justify-between mb-1">
                    <p className="text-sm font-medium capitalize text-ds-foreground">{severity}</p>
                    <div className="flex items-center space-x-2">
                      <StatusPill status={severityStatus[severity] || 'idle'}>
                        {count as number}
                      </StatusPill>
                      <p className="text-sm text-ds-foreground">{percentage.toFixed(1)}%</p>
                    </div>
                  </div>
                  <ProgressBar
                    value={percentage}
                    color={severityProgressColors[severity] || 'gray'}
                  />
                </div>
              );
            })}
          </div>
        </CardContent>
      </Card>

      {/* Active Exceptions */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Active Exceptions</CardTitle>
              <CardDescription>Issues requiring attention</CardDescription>
            </div>
            <StatusPill status="fail">{summary.openCount} open</StatusPill>
          </div>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {exceptions
              .filter((e: ExceptionItem) => e.status !== 'resolved')
              .slice(0, 8)
              .map((exception: ExceptionItem, index: number) => (
                <motion.div
                  key={exception.id || index}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: index * 0.05 }}
                  className={`p-4 border rounded-lg ${
                    exception.severity === 'critical'
                      ? 'border-ds-status-fail/40 bg-ds-status-fail/10'
                      : exception.severity === 'high'
                        ? 'border-ds-status-warn/40 bg-ds-status-warn/10'
                        : 'border-ds-enterprise-line'
                  }`}
                >
                  <div className="flex items-start justify-between">
                    <div className="flex items-start space-x-3">
                      <div
                        className={`w-10 h-10 rounded-full flex items-center justify-center ${
                          exception.severity === 'critical'
                            ? 'bg-ds-status-fail/15'
                            : exception.severity === 'high'
                              ? 'bg-ds-status-warn/15'
                              : 'bg-ds-status-run/15'
                        }`}
                      >
                        <ExclamationTriangleIcon
                          className={`w-5 h-5 ${
                            exception.severity === 'critical'
                              ? 'text-ds-status-fail'
                              : exception.severity === 'high'
                                ? 'text-ds-status-warn'
                                : 'text-ds-status-run'
                          }`}
                        />
                      </div>
                      <div>
                        <p className="text-sm font-medium text-ds-foreground">{exception.title}</p>
                        <p className="text-sm text-ds-muted-foreground">{exception.description}</p>
                        <div className="flex items-center space-x-3 mt-2">
                          <StatusPill status={severityStatus[exception.severity] || 'idle'}>
                            {exception.severity}
                          </StatusPill>
                          <StatusPill status={statusPillMap[exception.status] || 'idle'}>
                            {exception.status}
                          </StatusPill>
                          <p className="text-xs text-ds-muted-foreground">{exception.category}</p>
                        </div>
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-xs text-ds-muted-foreground">{exception.timestamp}</p>
                      {exception.suggestedAction && (
                        <div className="mt-2 flex items-center space-x-1 text-ds-primary">
                          <BoltIcon className="w-3 h-3" />
                          <p className="text-xs">Auto-fix available</p>
                        </div>
                      )}
                    </div>
                  </div>
                </motion.div>
              ))}
          </div>
        </CardContent>
      </Card>

      {/* Recent Resolutions */}
      <Card>
        <CardHeader>
          <CardTitle>Recent Resolutions</CardTitle>
          <CardDescription>Successfully resolved exceptions</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-ds-enterprise-line">
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Exception
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Resolution
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Method
                  </th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Time to Resolve
                  </th>
                </tr>
              </thead>
              <tbody>
                {recentResolutions.map((resolution: ExceptionResolution, index: number) => (
                  <motion.tr
                    key={resolution.id || index}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: index * 0.03 }}
                    className="border-b border-ds-enterprise-line"
                  >
                    <td className="py-2 px-3">
                      <p className="text-sm font-medium text-ds-foreground">{resolution.title}</p>
                    </td>
                    <td className="py-2 px-3 text-sm text-ds-foreground">
                      {resolution.resolution}
                    </td>
                    <td className="py-2 px-3">
                      <Badge variant={resolution.method === 'auto' ? 'success' : 'primary'}>
                        {resolution.method === 'auto' ? 'Auto-resolved' : 'Manual'}
                      </Badge>
                    </td>
                    <td className="py-2 px-3 text-sm text-right text-ds-foreground">
                      {resolution.timeToResolve}
                    </td>
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
      {
        id: '1',
        title: 'Payment Processing Failed',
        description: 'Multiple payment attempts failing for Stripe gateway',
        severity: 'critical',
        status: 'investigating',
        category: 'Payments',
        timestamp: '5 min ago',
        suggestedAction: true,
      },
      {
        id: '2',
        title: 'Inventory Sync Delayed',
        description: 'Warehouse inventory sync exceeded 15 minute threshold',
        severity: 'high',
        status: 'open',
        category: 'Inventory',
        timestamp: '12 min ago',
        suggestedAction: true,
      },
      {
        id: '3',
        title: 'Order Fulfillment Stuck',
        description: '3 orders stuck in processing for over 2 hours',
        severity: 'high',
        status: 'investigating',
        category: 'Orders',
        timestamp: '25 min ago',
        suggestedAction: false,
      },
      {
        id: '4',
        title: 'High Return Rate Alert',
        description: 'Product SKU-1234 return rate exceeds 15%',
        severity: 'medium',
        status: 'open',
        category: 'Returns',
        timestamp: '1 hour ago',
        suggestedAction: true,
      },
      {
        id: '5',
        title: 'Customer Complaint Spike',
        description: 'Unusual increase in customer complaints detected',
        severity: 'medium',
        status: 'open',
        category: 'Support',
        timestamp: '2 hours ago',
        suggestedAction: false,
      },
      {
        id: '6',
        title: 'Low Stock Warning',
        description: '5 popular items below safety stock levels',
        severity: 'low',
        status: 'open',
        category: 'Inventory',
        timestamp: '3 hours ago',
        suggestedAction: true,
      },
    ],
    recentResolutions: [
      {
        id: '1',
        title: 'Database Connection Pool Exhausted',
        resolution: 'Increased pool size from 20 to 50',
        method: 'manual',
        timeToResolve: '15 min',
      },
      {
        id: '2',
        title: 'Shipping Label Generation Failed',
        resolution: 'Retried with backup carrier API',
        method: 'auto',
        timeToResolve: '2 min',
      },
      {
        id: '3',
        title: 'Duplicate Order Detection',
        resolution: 'Merged duplicate orders automatically',
        method: 'auto',
        timeToResolve: '30 sec',
      },
      {
        id: '4',
        title: 'Price Discrepancy Alert',
        resolution: 'Updated price from source of truth',
        method: 'auto',
        timeToResolve: '1 min',
      },
    ],
  };
}
