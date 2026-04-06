'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, DonutChart, BarChart, ProgressBar } from '@tremor/react';
import { ExclamationTriangleIcon, ArrowTrendingUpIcon, ArrowTrendingDownIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getInventoryAnalyticsData } from '@/app/actions/commerce';
import { formatCurrency, formatCompactNumber } from '@/lib/utils';
import type { InventoryAnalyticsData, InventoryCategory, TopMovingItem, SlowMovingItem } from '@/lib/types/dashboard-data';
import type { InventoryItem } from '@/lib/types';

interface InventoryAnalyticsProps {
  data?: InventoryAnalyticsData;
}

function InventoryAnalyticsInner({ data: propData }: InventoryAnalyticsProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getInventoryAnalyticsData(),
    { initialData: propData, refreshInterval: 60000 }
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
        <Text className="text-red-600">Failed to load inventory analytics</Text>
      </Card>
    );
  }

  const healthScore = Math.max(0, 100 - (data.lowStockItems * 2) - (data.outOfStockItems * 5));

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={3} numItemsLg={6} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <Text>Total SKUs</Text>
          <Metric>{formatCompactNumber(data.totalSKUs)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Total Units</Text>
          <Metric>{formatCompactNumber(data.totalUnits)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="indigo">
          <Text>Total Value</Text>
          <Metric>{formatCurrency(data.totalValue)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Low Stock</Text>
          <Metric>{data.lowStockItems}</Metric>
          {data.lowStockItems > 0 && (
            <Badge color="amber" icon={ExclamationTriangleIcon} className="mt-1">
              Needs attention
            </Badge>
          )}
        </Card>
        <Card decoration="top" decorationColor="red">
          <Text>Out of Stock</Text>
          <Metric>{data.outOfStockItems}</Metric>
          {data.outOfStockItems > 0 && (
            <Badge color="red" icon={ExclamationTriangleIcon} className="mt-1">
              Critical
            </Badge>
          )}
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Turnover Rate</Text>
          <Metric>{(data.turnoverRate || 0).toFixed(1)}x</Metric>
        </Card>
      </Grid>

      {/* Inventory Health */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Inventory Health Score</Title>
            <Text className="text-gray-500">Overall inventory status</Text>
          </div>
          <div className="text-right">
            <Metric className={`${
              healthScore >= 80 ? 'text-emerald-600' :
              healthScore >= 60 ? 'text-amber-600' : 'text-red-600'
            }`}>
              {healthScore}%
            </Metric>
            <Badge color={
              healthScore >= 80 ? 'emerald' :
              healthScore >= 60 ? 'amber' : 'red'
            }>
              {healthScore >= 80 ? 'Healthy' : healthScore >= 60 ? 'Fair' : 'Critical'}
            </Badge>
          </div>
        </div>

        <ProgressBar
          value={healthScore}
          color={
            healthScore >= 80 ? 'emerald' :
            healthScore >= 60 ? 'amber' : 'red'
          }
        />

        <div className="mt-4 grid grid-cols-3 gap-4 text-center">
          <div className="p-3 bg-emerald-50 dark:bg-emerald-900/20 rounded-lg">
            <Text className="text-emerald-600 font-medium">In Stock</Text>
            <Text className="text-2xl font-bold text-emerald-700">
              {data.totalSKUs - data.lowStockItems - data.outOfStockItems}
            </Text>
          </div>
          <div className="p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
            <Text className="text-amber-600 font-medium">Low Stock</Text>
            <Text className="text-2xl font-bold text-amber-700">{data.lowStockItems}</Text>
          </div>
          <div className="p-3 bg-red-50 dark:bg-red-900/20 rounded-lg">
            <Text className="text-red-600 font-medium">Out of Stock</Text>
            <Text className="text-2xl font-bold text-red-700">{data.outOfStockItems}</Text>
          </div>
        </div>
      </Card>

      {/* Charts */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        {/* Category Distribution */}
        <Card>
          <Title>Inventory by Category</Title>
          <Text className="text-gray-500 mb-4">Units distribution</Text>
          {data.categories && data.categories.length > 0 ? (
            <DonutChart
              className="h-64"
              data={data.categories.map((c: InventoryCategory) => ({
                name: c.name,
                value: c.units,
              }))}
              category="value"
              index="name"
              colors={['indigo', 'blue', 'emerald', 'amber', 'purple', 'pink']}
              showAnimation
            />
          ) : (
            <div className="h-64 flex items-center justify-center">
              <Text className="text-gray-400">No category data available</Text>
            </div>
          )}
        </Card>

        {/* Category Value */}
        <Card>
          <Title>Value by Category</Title>
          <Text className="text-gray-500 mb-4">Inventory value distribution</Text>
          {data.categories && data.categories.length > 0 ? (
            <BarChart
              className="h-64"
              data={data.categories.map((c: InventoryCategory) => ({
                category: c.name,
                value: c.value,
              }))}
              index="category"
              categories={['value']}
              colors={['indigo']}
              valueFormatter={(v) => formatCurrency(v)}
              showAnimation
            />
          ) : (
            <div className="h-64 flex items-center justify-center">
              <Text className="text-gray-400">No value data available</Text>
            </div>
          )}
        </Card>
      </Grid>

      {/* Top Moving Items */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        <Card>
          <div className="flex items-center space-x-2 mb-4">
            <ArrowTrendingUpIcon className="w-5 h-5 text-emerald-600" />
            <Title>Top Moving Items</Title>
          </div>
          <div className="space-y-3">
            {(data.topMovingItems || []).slice(0, 5).map((item: TopMovingItem, index: number) => (
              <div key={item.sku} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700">
                <div className="flex items-center space-x-3">
                  <div className="w-8 h-8 bg-emerald-100 dark:bg-emerald-900/30 rounded-full flex items-center justify-center">
                    <span className="text-sm font-bold text-emerald-600">{index + 1}</span>
                  </div>
                  <div>
                    <Text className="font-medium">{item.name}</Text>
                    <Text className="text-xs text-gray-500">{item.sku}</Text>
                  </div>
                </div>
                <Badge color="emerald">{item.velocity} units/day</Badge>
              </div>
            ))}
            {(!data.topMovingItems || data.topMovingItems.length === 0) && (
              <Text className="text-gray-400 text-center py-4">No data available</Text>
            )}
          </div>
        </Card>

        <Card>
          <div className="flex items-center space-x-2 mb-4">
            <ArrowTrendingDownIcon className="w-5 h-5 text-red-600" />
            <Title>Slow Moving Items</Title>
          </div>
          <div className="space-y-3">
            {(data.slowMovingItems || []).slice(0, 5).map((item: SlowMovingItem, index: number) => (
              <div key={item.sku} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700">
                <div className="flex items-center space-x-3">
                  <div className="w-8 h-8 bg-red-100 dark:bg-red-900/30 rounded-full flex items-center justify-center">
                    <span className="text-sm font-bold text-red-600">{index + 1}</span>
                  </div>
                  <div>
                    <Text className="font-medium">{item.name}</Text>
                    <Text className="text-xs text-gray-500">{item.sku}</Text>
                  </div>
                </div>
                <Badge color="red">{item.daysSinceLastSale}+ days</Badge>
              </div>
            ))}
            {(!data.slowMovingItems || data.slowMovingItems.length === 0) && (
              <Text className="text-gray-400 text-center py-4">No data available</Text>
            )}
          </div>
        </Card>
      </Grid>

      {/* Critical Items Alert */}
      {data.criticalItems && data.criticalItems.length > 0 && (
        <Card className="border-red-200 bg-red-50/50 dark:bg-red-900/20">
          <div className="flex items-center space-x-2 mb-4">
            <ExclamationTriangleIcon className="w-5 h-5 text-red-600" />
            <Title className="text-red-800 dark:text-red-200">Critical Stock Alerts</Title>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {data.criticalItems.slice(0, 6).map((item: InventoryItem) => (
              <div key={item.sku} className="bg-white dark:bg-gray-800 p-3 rounded-lg border border-red-200 dark:border-red-800">
                <Text className="font-medium">{item.productName}</Text>
                <Text className="text-xs text-gray-500 font-mono">{item.sku}</Text>
                <div className="flex items-center justify-between mt-2">
                  <Text className="text-red-600 font-medium">
                    {item.availableQuantity} left
                  </Text>
                  <Badge color="red" size="xs">
                    Reorder: {item.reorderPoint}
                  </Badge>
                </div>
              </div>
            ))}
          </div>
        </Card>
      )}
    </motion.div>
  );
}

const InventoryAnalytics = memo(InventoryAnalyticsInner);
export default InventoryAnalytics;
