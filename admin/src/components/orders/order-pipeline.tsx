'use client';

import { Card, Title, Text, Badge, Grid, Metric, BarChart, ProgressBar } from '@tremor/react';
import { ShoppingCartIcon, TruckIcon, CheckCircleIcon, XCircleIcon, ClockIcon, ArrowRightIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { memo } from 'react';
import type { ForwardRefExoticComponent, RefAttributes, SVGProps } from 'react';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getOrderPipelineData } from '@/app/actions/commerce';
import { formatCurrency, formatCompactNumber } from '@/lib/utils';
import type { OrderPipelineData, OrderPipelineStatusGroup, TremorColor } from '@/lib/types/dashboard-data';
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

const statusColors: Record<string, string> = {
  pending: 'amber',
  confirmed: 'blue',
  processing: 'indigo',
  shipped: 'purple',
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
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-gray-200 rounded w-48" />
          <div className="h-32 bg-gray-200 rounded" />
        </div>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-red-200">
        <Text className="text-red-600">Failed to load order pipeline</Text>
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
      <Grid numItems={2} numItemsSm={3} numItemsLg={6} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <Text>Total Orders</Text>
          <Metric>{formatCompactNumber(summary.totalOrders)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Total Value</Text>
          <Metric>{formatCurrency(summary.totalValue)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="indigo">
          <Text>Avg Order Value</Text>
          <Metric>{formatCurrency(summary.averageOrderValue)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Delivered Rate</Text>
          <Metric>{summary.deliveredRate.toFixed(1)}%</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>In Progress</Text>
          <Metric>{summary.inProgressCount}</Metric>
        </Card>
        <Card decoration="top" decorationColor="red">
          <Text>Exceptions</Text>
          <Metric>{summary.exceptionsCount}</Metric>
        </Card>
      </Grid>

      {/* Order Pipeline Flow */}
      <Card>
        <Title>Order Pipeline</Title>
        <Text className="text-gray-500 mb-6">Orders moving through fulfillment stages</Text>

        <div className="flex items-center justify-between overflow-x-auto pb-4">
          {statusGroups.map((group: OrderPipelineStatusGroup, index: number) => {
            const Icon = statusIcons[group.key] || ClockIcon;
            const color = statusColors[group.key] || 'gray';

            return (
              <motion.div
                key={group.key}
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.1 }}
                className="flex items-center"
              >
                <div className="text-center min-w-[120px]">
                  <div className={`mx-auto w-12 h-12 rounded-full bg-${color}-100 dark:bg-${color}-900/30 flex items-center justify-center mb-2`}>
                    <Icon className={`w-6 h-6 text-${color}-600`} />
                  </div>
                  <Text className="font-medium">{group.label}</Text>
                  <Metric className="text-2xl">{group.count}</Metric>
                  <Text className="text-xs text-gray-500">
                    {formatCurrency(group.totalValue)}
                  </Text>
                </div>
                {index < statusGroups.length - 1 && (
                  <ArrowRightIcon className="w-6 h-6 text-gray-300 mx-2" />
                )}
              </motion.div>
            );
          })}
        </div>
      </Card>

      {/* Status Breakdown */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        <Card>
          <Title>Orders by Status</Title>
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
        </Card>

        <Card>
          <Title>Status Distribution</Title>
          <div className="mt-4 space-y-4">
            {statusGroups.map((group: OrderPipelineStatusGroup) => {
              const percentage = summary.totalOrders > 0
                ? (group.count / summary.totalOrders) * 100
                : 0;

              return (
                <div key={group.key}>
                  <div className="flex justify-between mb-1">
                    <Text className="font-medium">{group.label}</Text>
                    <div className="flex items-center space-x-2">
                      <Badge color={statusColors[group.key] as TremorColor} size="xs">
                        {group.count}
                      </Badge>
                      <Text className="text-sm">{percentage.toFixed(1)}%</Text>
                    </div>
                  </div>
                  <ProgressBar
                    value={percentage}
                    color={statusColors[group.key] as TremorColor}
                  />
                </div>
              );
            })}
          </div>
        </Card>
      </Grid>

      {/* Recent Orders by Stage */}
      <Card>
        <Title>Recent Orders by Stage</Title>
        <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
          {statusGroups
            .filter((g: OrderPipelineStatusGroup) => ['pending', 'processing', 'shipped'].includes(g.key))
            .map((group: OrderPipelineStatusGroup) => (
              <div key={group.key} className="border rounded-lg p-4 dark:border-gray-700">
                <div className="flex items-center justify-between mb-3">
                  <Text className="font-medium">{group.label}</Text>
                  <Badge color={statusColors[group.key] as TremorColor}>{group.count}</Badge>
                </div>
                <div className="space-y-2">
                  {(group.orders || []).slice(0, 3).map((order: Pick<Order, 'id' | 'totalAmount'>) => (
                    <div key={order.id} className="flex justify-between text-sm">
                      <Text className="font-mono">{order.id.slice(0, 8)}...</Text>
                      <Text>{formatCurrency(order.totalAmount)}</Text>
                    </div>
                  ))}
                  {(!group.orders || group.orders.length === 0) && (
                    <Text className="text-gray-400 text-sm">No orders</Text>
                  )}
                </div>
              </div>
            ))}
        </div>
      </Card>
    </motion.div>
  );
}

const OrderPipeline = memo(OrderPipelineInner);
export default OrderPipeline;
