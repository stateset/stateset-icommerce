'use client';

import { Card, CardContent, MetricCard, Badge } from '@stateset/design';
import { BarChart, ProgressBar, type Color } from '@tremor/react';
import { ShoppingCartIcon, TruckIcon, CheckCircleIcon, XCircleIcon, ClockIcon, ArrowRightIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { memo } from 'react';
import type { ForwardRefExoticComponent, RefAttributes, SVGProps } from 'react';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getOrderPipelineData } from '@/app/actions/commerce';
import { formatCurrency, formatCompactNumber } from '@/lib/utils';
import type { OrderPipelineData, OrderPipelineStatusGroup } from '@/lib/types/dashboard-data';
import type { Order } from '@/lib/types';

interface OrderPipelineProps {
  data?: OrderPipelineData;
}

type HeroIcon = ForwardRefExoticComponent<
  Omit<SVGProps<SVGSVGElement>, 'ref'> & {
    title?: string;
    titleId?: string;
  } & RefAttributes<SVGSVGElement>
>;

const statusIcons: Record<string, HeroIcon> = {
  pending: ClockIcon,
  confirmed: CheckCircleIcon,
  processing: ShoppingCartIcon,
  shipped: TruckIcon,
  delivered: CheckCircleIcon,
  cancelled: XCircleIcon,
};

type DsBadgeVariant = 'default' | 'primary' | 'accent' | 'success' | 'warning' | 'danger' | 'outline';

const statusBadgeVariants: Record<string, DsBadgeVariant> = {
  pending: 'warning',
  confirmed: 'primary',
  processing: 'primary',
  shipped: 'accent',
  delivered: 'success',
  cancelled: 'danger',
};

const progressColors: Record<string, Color> = {
  pending: 'amber',
  confirmed: 'blue',
  processing: 'indigo',
  shipped: 'violet',
  delivered: 'emerald',
  cancelled: 'red',
};

function OrderPipelineInner({ data: propData }: OrderPipelineProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getOrderPipelineData(),
    { initialData: propData, refreshInterval: 30000 }
  );

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-32 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load order pipeline</p>
        </CardContent>
      </Card>
    );
  }

  const { summary, statusGroups } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Summary Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
        <MetricCard label="Total Orders" value={formatCompactNumber(summary.totalOrders)} tone="primary" />
        <MetricCard label="Total Value" value={formatCurrency(summary.totalValue)} tone="success" />
        <MetricCard label="Avg Order Value" value={formatCurrency(summary.averageOrderValue)} tone="primary" />
        <MetricCard label="Delivered Rate" value={`${summary.deliveredRate.toFixed(1)}%`} tone="accent" />
        <MetricCard label="In Progress" value={summary.inProgressCount} tone="warning" />
        <MetricCard label="Exceptions" value={summary.exceptionsCount} tone="danger" />
      </div>

      {/* Order Pipeline Flow */}
      <Card>
        <CardContent>
          <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Order Pipeline</h3>
          <p className="text-sm text-ds-muted-foreground mb-6">Orders moving through fulfillment stages</p>

          <div className="flex items-center justify-between overflow-x-auto pb-4">
            {statusGroups.map((group: OrderPipelineStatusGroup, index: number) => {
              const Icon = statusIcons[group.key] || ClockIcon;

              return (
                <motion.div
                  key={group.key}
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: index * 0.1 }}
                  className="flex items-center"
                >
                  <div className="text-center min-w-[120px]">
                    <div className="mx-auto w-12 h-12 rounded-full bg-ds-muted flex items-center justify-center mb-2">
                      <Icon className="w-6 h-6 text-ds-muted-foreground" />
                    </div>
                    <p className="text-sm font-medium text-ds-foreground">{group.label}</p>
                    <p className="ds-instrument-number text-2xl text-ds-foreground">{group.count}</p>
                    <p className="text-xs text-ds-muted-foreground">
                      {formatCurrency(group.totalValue)}
                    </p>
                  </div>
                  {index < statusGroups.length - 1 && (
                    <ArrowRightIcon className="w-6 h-6 text-ds-muted-foreground/50 mx-2" />
                  )}
                </motion.div>
              );
            })}
          </div>
        </CardContent>
      </Card>

      {/* Status Breakdown */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardContent>
            <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Orders by Status</h3>
            <BarChart
              className="h-64 mt-4"
              data={statusGroups.map((g: OrderPipelineStatusGroup) => ({
                status: g.label,
                orders: g.count,
              }))}
              index="status"
              categories={['orders']}
              colors={['indigo']}
              showAnimation
            />
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Status Distribution</h3>
            <div className="mt-4 space-y-4">
              {statusGroups.map((group: OrderPipelineStatusGroup) => {
                const percentage = summary.totalOrders > 0
                  ? (group.count / summary.totalOrders) * 100
                  : 0;

                return (
                  <div key={group.key}>
                    <div className="flex justify-between mb-1">
                      <p className="text-sm font-medium text-ds-foreground">{group.label}</p>
                      <div className="flex items-center space-x-2">
                        <Badge variant={statusBadgeVariants[group.key] || 'default'}>
                          {group.count}
                        </Badge>
                        <p className="text-sm text-ds-muted-foreground">{percentage.toFixed(1)}%</p>
                      </div>
                    </div>
                    <ProgressBar
                      value={percentage}
                      color={progressColors[group.key] || 'indigo'}
                    />
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Recent Orders by Stage */}
      <Card>
        <CardContent>
          <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Recent Orders by Stage</h3>
          <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
            {statusGroups
              .filter((g: OrderPipelineStatusGroup) => ['pending', 'processing', 'shipped'].includes(g.key))
              .map((group: OrderPipelineStatusGroup) => (
                <div key={group.key} className="border border-ds-enterprise-line rounded-lg p-4">
                  <div className="flex items-center justify-between mb-3">
                    <p className="text-sm font-medium text-ds-foreground">{group.label}</p>
                    <Badge variant={statusBadgeVariants[group.key] || 'default'}>{group.count}</Badge>
                  </div>
                  <div className="space-y-2">
                    {(group.orders || []).slice(0, 3).map((order: Pick<Order, 'id' | 'totalAmount'>) => (
                      <div key={order.id} className="flex justify-between text-sm">
                        <p className="font-mono text-ds-foreground">{order.id.slice(0, 8)}...</p>
                        <p className="text-ds-foreground">{formatCurrency(order.totalAmount)}</p>
                      </div>
                    ))}
                    {(!group.orders || group.orders.length === 0) && (
                      <p className="text-ds-muted-foreground text-sm">No orders</p>
                    )}
                  </div>
                </div>
              ))}
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

const OrderPipeline = memo(OrderPipelineInner);
export default OrderPipeline;
