'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, DonutChart, ProgressBar } from '@tremor/react';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getReturnsManagementData } from '@/app/actions/commerce';
import { formatCurrency, formatPercentage, formatDateTime } from '@/lib/utils';
import type { ReturnsManagementData, ReturnsPipelineStage, TremorColor } from '@/lib/types/dashboard-data';
import type { Return } from '@/lib/types';

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

const statusColors: Record<string, string> = {
  requested: 'amber',
  approved: 'blue',
  received: 'indigo',
  inspected: 'purple',
  refunded: 'emerald',
  rejected: 'red',
  closed: 'gray',
};

function ReturnsManagementInner({ data: propData }: ReturnsManagementProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getReturnsManagementData(),
    { initialData: propData, refreshInterval: 30000 }
  );

  if (isLoading && !data) {
    return (
      <Card>
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-gray-200 rounded w-48" />
          <div className="h-48 bg-gray-200 rounded" />
        </div>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-red-200">
        <Text className="text-red-600">Failed to load returns data</Text>
      </Card>
    );
  }

  const { returns, analytics, pipeline } = data;

  const reasonChartData = Object.entries(analytics.returnsByReason || {}).map(([reason, count]) => ({
    name: reasonLabels[reason] || reason,
    value: count as number,
  }));

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <Text>Total Returns</Text>
          <Metric>{analytics.totalReturns}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Return Rate</Text>
          <Metric>{formatPercentage(analytics.returnRate)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="red">
          <Text>Refund Total</Text>
          <Metric>{formatCurrency(analytics.refundTotal)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Avg Processing</Text>
          <Metric>{analytics.averageProcessingTime}h</Metric>
        </Card>
      </Grid>

      {/* Return Pipeline */}
      <Card>
        <Title>Return Pipeline</Title>
        <Text className="text-gray-500 mb-6">Returns moving through processing stages</Text>

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
                <div className={`mx-auto w-12 h-12 rounded-full flex items-center justify-center mb-2 ${
                  stage.count > 0 ? 'bg-indigo-100 dark:bg-indigo-900/30' : 'bg-gray-100 dark:bg-gray-800'
                }`}>
                  <span className={`font-bold ${stage.count > 0 ? 'text-indigo-600' : 'text-gray-400'}`}>
                    {stage.count}
                  </span>
                </div>
                <Text className="text-sm font-medium">{stage.stage}</Text>
              </div>
              {index < pipeline.length - 1 && (
                <div className="w-8 h-0.5 bg-gray-200 dark:bg-gray-700 mx-2" />
              )}
            </motion.div>
          ))}
        </div>
      </Card>

      {/* Charts */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        {/* Returns by Reason */}
        <Card>
          <Title>Returns by Reason</Title>
          <Text className="text-gray-500 mb-4">Why customers are returning</Text>
          {reasonChartData.length > 0 ? (
            <DonutChart
              className="h-64"
              data={reasonChartData}
              category="value"
              index="name"
              colors={['red', 'amber', 'blue', 'purple', 'gray']}
              showAnimation
            />
          ) : (
            <div className="h-64 flex items-center justify-center">
              <Text className="text-gray-400">No return reason data</Text>
            </div>
          )}
        </Card>

        {/* Returns by Status */}
        <Card>
          <Title>Returns by Status</Title>
          <Text className="text-gray-500 mb-4">Current status distribution</Text>
          <div className="space-y-4">
            {Object.entries(analytics.returnsByStatus || {}).map(([status, count]) => {
              const total = analytics.totalReturns || 1;
              const percentage = ((count as number) / total) * 100;

              return (
                <div key={status}>
                  <div className="flex justify-between mb-1">
                    <Text className="font-medium capitalize">{status}</Text>
                    <div className="flex items-center space-x-2">
                      <Badge color={statusColors[status] as TremorColor || 'gray'} size="xs">
                        {count as number}
                      </Badge>
                      <Text className="text-sm">{percentage.toFixed(1)}%</Text>
                    </div>
                  </div>
                  <ProgressBar
                    value={percentage}
                    color={statusColors[status] as TremorColor || 'gray'}
                  />
                </div>
              );
            })}
          </div>
        </Card>
      </Grid>

      {/* Top Returned Products */}
      {analytics.topReturnedProducts && analytics.topReturnedProducts.length > 0 && (
        <Card>
          <Title>Top Returned Products</Title>
          <Text className="text-gray-500 mb-4">Products with highest return rates</Text>
          <div className="space-y-3">
            {analytics.topReturnedProducts.slice(0, 5).map((product: { productId: string; name: string; count: number; rate: number }, index: number) => (
              <div key={product.productId} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700">
                <div className="flex items-center space-x-3">
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center ${
                    index < 2 ? 'bg-red-100 dark:bg-red-900/30' : 'bg-amber-100 dark:bg-amber-900/30'
                  }`}>
                    <span className={`text-sm font-bold ${
                      index < 2 ? 'text-red-600' : 'text-amber-600'
                    }`}>
                      {index + 1}
                    </span>
                  </div>
                  <div>
                    <Text className="font-medium">{product.name}</Text>
                    <Text className="text-xs text-gray-500">{product.productId}</Text>
                  </div>
                </div>
                <div className="text-right">
                  <Badge color={index < 2 ? 'red' : 'amber'}>
                    {product.count} returns
                  </Badge>
                  <Text className="text-xs text-gray-500 mt-1">
                    {formatPercentage(product.rate)} rate
                  </Text>
                </div>
              </div>
            ))}
          </div>
        </Card>
      )}

      {/* Recent Returns */}
      <Card>
        <Title>Recent Returns</Title>
        <div className="mt-4 overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Return ID</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Order ID</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Status</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Reason</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Amount</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Created</th>
              </tr>
            </thead>
            <tbody>
              {returns.slice(0, 10).map((ret: Return) => (
                <tr key={ret.id} className="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800">
                  <td className="py-2 px-3 text-sm font-mono">{ret.id.slice(0, 8)}...</td>
                  <td className="py-2 px-3 text-sm font-mono">{ret.orderId.slice(0, 8)}...</td>
                  <td className="py-2 px-3">
                    <Badge color={statusColors[ret.status] as TremorColor || 'gray'} size="xs">
                      {ret.status}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-sm">
                    {reasonLabels[ret.reasonCategory] || ret.reasonCategory}
                  </td>
                  <td className="py-2 px-3 text-sm text-right font-medium">
                    {ret.refundAmount ? formatCurrency(ret.refundAmount) : '-'}
                  </td>
                  <td className="py-2 px-3 text-sm text-gray-500">
                    {formatDateTime(ret.createdAt)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </motion.div>
  );
}

const ReturnsManagement = memo(ReturnsManagementInner);
export default ReturnsManagement;
