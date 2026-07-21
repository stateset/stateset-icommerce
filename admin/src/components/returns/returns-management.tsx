'use client';

import { memo } from 'react';
import { DonutChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Badge,
  MetricCard,
  StatusPill,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@stateset/design';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getReturnsManagementData } from '@/app/actions/commerce';
import { formatCurrency, formatPercentage, formatDateTime } from '@/lib/utils';
import type { ReturnsManagementData, ReturnsPipelineStage } from '@/lib/types/dashboard-data';
import type { Return } from '@/lib/types';

type DsStatus = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';

interface ReturnsManagementProps {
  data?: ReturnsManagementData;
}

const reasonLabels: Record<string, string> = {
  defective: 'Defective Product',
  wrong_item: 'Wrong Item',
  not_as_described: 'Not as Described',
  changed_mind: 'Changed Mind',
  other: 'Other',
};

const statusPills: Record<string, DsStatus> = {
  requested: 'review',
  approved: 'run',
  received: 'run',
  inspected: 'review',
  refunded: 'ok',
  rejected: 'fail',
  closed: 'idle',
};

function ReturnsManagementInner({ data: propData }: ReturnsManagementProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getReturnsManagementData(), {
    initialData: propData,
    refreshInterval: 30000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-48 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load returns data</p>
        </CardContent>
      </Card>
    );
  }

  const { returns, analytics, pipeline } = data;

  const reasonChartData = Object.entries(analytics.returnsByReason || {}).map(
    ([reason, count]) => ({
      name: reasonLabels[reason] || reason,
      value: count as number,
    }),
  );

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard label="Total Returns" value={analytics.totalReturns} tone="primary" />
        <MetricCard
          label="Return Rate"
          value={formatPercentage(analytics.returnRate)}
          tone="warning"
        />
        <MetricCard
          label="Refund Total"
          value={formatCurrency(analytics.refundTotal)}
          tone="danger"
        />
        <MetricCard
          label="Avg Processing"
          value={`${analytics.averageProcessingTime}h`}
          tone="success"
        />
      </div>

      {/* Return Pipeline */}
      <Card>
        <CardHeader>
          <CardTitle>Return Pipeline</CardTitle>
          <CardDescription>Returns moving through processing stages</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between overflow-x-auto pb-4">
            {pipeline.map((stage: ReturnsPipelineStage, index: number) => (
              <motion.div
                key={stage.stage}
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.1 }}
                className="flex items-center"
              >
                <div className="text-center min-w-[100px]">
                  <div
                    className={`mx-auto w-12 h-12 rounded-full flex items-center justify-center mb-2 ${
                      stage.count > 0 ? 'bg-ds-brand-100 dark:bg-ds-brand-900/30' : 'bg-ds-muted'
                    }`}
                  >
                    <span
                      className={`font-bold ${stage.count > 0 ? 'text-ds-primary' : 'text-ds-muted-foreground'}`}
                    >
                      {stage.count}
                    </span>
                  </div>
                  <p className="text-sm font-medium text-ds-foreground">{stage.stage}</p>
                </div>
                {index < pipeline.length - 1 && (
                  <div className="w-8 h-0.5 bg-ds-enterprise-line mx-2" />
                )}
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Returns by Reason */}
        <Card>
          <CardHeader>
            <CardTitle>Returns by Reason</CardTitle>
            <CardDescription>Why customers are returning</CardDescription>
          </CardHeader>
          <CardContent>
            {reasonChartData.length > 0 ? (
              <DonutChart
                className="h-64"
                data={reasonChartData}
                category="value"
                index="name"
                colors={['indigo', 'emerald', 'violet', 'amber', 'cyan']}
                showAnimation
              />
            ) : (
              <div className="h-64 flex items-center justify-center">
                <p className="text-sm text-ds-muted-foreground">No return reason data</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Returns by Status */}
        <Card>
          <CardHeader>
            <CardTitle>Returns by Status</CardTitle>
            <CardDescription>Current status distribution</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {Object.entries(analytics.returnsByStatus || {}).map(([status, count]) => {
                const total = analytics.totalReturns || 1;
                const percentage = ((count as number) / total) * 100;

                return (
                  <div key={status}>
                    <div className="flex justify-between mb-1">
                      <p className="text-sm font-medium text-ds-foreground capitalize">{status}</p>
                      <div className="flex items-center space-x-2">
                        <StatusPill status={statusPills[status] || 'idle'}>
                          {count as number}
                        </StatusPill>
                        <p className="text-sm text-ds-muted-foreground">{percentage.toFixed(1)}%</p>
                      </div>
                    </div>
                    <ProgressBar
                      value={percentage}
                      color={
                        status === 'rejected'
                          ? 'rose'
                          : status === 'refunded'
                            ? 'emerald'
                            : 'indigo'
                      }
                    />
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Top Returned Products */}
      {analytics.topReturnedProducts && analytics.topReturnedProducts.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Top Returned Products</CardTitle>
            <CardDescription>Products with highest return rates</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {analytics.topReturnedProducts
                .slice(0, 5)
                .map(
                  (
                    product: { productId: string; name: string; count: number; rate: number },
                    index: number,
                  ) => (
                    <div
                      key={product.productId}
                      className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg"
                    >
                      <div className="flex items-center space-x-3">
                        <div
                          className={`w-8 h-8 rounded-full flex items-center justify-center ${
                            index < 2 ? 'bg-ds-status-fail/10' : 'bg-ds-status-warn/10'
                          }`}
                        >
                          <span
                            className={`text-sm font-bold ${
                              index < 2 ? 'text-ds-status-fail' : 'text-ds-status-warn'
                            }`}
                          >
                            {index + 1}
                          </span>
                        </div>
                        <div>
                          <p className="text-sm font-medium text-ds-foreground">{product.name}</p>
                          <p className="text-xs text-ds-muted-foreground">{product.productId}</p>
                        </div>
                      </div>
                      <div className="text-right">
                        <Badge variant={index < 2 ? 'danger' : 'warning'}>
                          {product.count} returns
                        </Badge>
                        <p className="text-xs text-ds-muted-foreground mt-1">
                          {formatPercentage(product.rate)} rate
                        </p>
                      </div>
                    </div>
                  ),
                )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Recent Returns */}
      <Card>
        <CardHeader>
          <CardTitle>Recent Returns</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Return ID</TableHead>
                  <TableHead>Order ID</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Reason</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                  <TableHead>Created</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {returns.slice(0, 10).map((ret: Return) => (
                  <TableRow key={ret.id}>
                    <TableCell className="text-sm font-mono">{ret.id.slice(0, 8)}...</TableCell>
                    <TableCell className="text-sm font-mono">
                      {ret.orderId.slice(0, 8)}...
                    </TableCell>
                    <TableCell>
                      <StatusPill status={statusPills[ret.status] || 'idle'}>
                        {ret.status}
                      </StatusPill>
                    </TableCell>
                    <TableCell className="text-sm">
                      {reasonLabels[ret.reasonCategory] || ret.reasonCategory}
                    </TableCell>
                    <TableCell tone="numeric" className="text-sm font-medium">
                      {ret.refundAmount ? formatCurrency(ret.refundAmount) : '-'}
                    </TableCell>
                    <TableCell className="text-sm text-ds-muted-foreground">
                      {formatDateTime(ret.createdAt)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

const ReturnsManagement = memo(ReturnsManagementInner);
export default ReturnsManagement;
