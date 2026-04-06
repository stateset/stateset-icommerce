'use client';

import { Card, Title, Text, Badge, Grid, Metric, AreaChart, BarChart } from '@tremor/react';
import { ArrowTrendingUpIcon, ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getDemandForecastingData } from '@/app/actions/commerce';
import { formatCurrency, formatNumber } from '@/lib/utils';
import type { DemandForecastingData, DemandHighProduct, DemandAlert } from '@/lib/types/dashboard-data';

interface DemandForecastingProps {
  data?: DemandForecastingData;
}

export default function DemandForecasting({ data: propData }: DemandForecastingProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getDemandForecastingData(),
    { initialData: propData, refreshInterval: 60000 }
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
        <Text className="text-red-600">Failed to load demand forecasting data</Text>
      </Card>
    );
  }

  const { forecast, topProducts, alerts, accuracy } = data;
  const hasForecastTimeline = forecast.timeline.length > 0;
  const hasHighDemandProducts = topProducts.highDemand.length > 0;
  const hasAlerts = alerts.length > 0;
  const hasCategoryDemand = forecast.categoryDemand.length > 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <Text>Forecast Accuracy</Text>
          <Metric>{hasForecastTimeline ? `${accuracy.overall}%` : 'Unavailable'}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Predicted Revenue</Text>
          <Metric>{hasForecastTimeline ? formatCurrency(forecast.predictedRevenue) : 'Unavailable'}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Stock Alerts</Text>
          <Metric>{alerts.length}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Trend Score</Text>
          <Metric>{hasForecastTimeline ? forecast.trendScore : 'Unavailable'}</Metric>
        </Card>
      </Grid>

      {/* Demand Forecast Chart */}
      <Card>
        <Title>30-Day Demand Forecast</Title>
        <Text className="text-gray-500 mb-4">Live forecast output from the embedded inventory engine</Text>
        {hasForecastTimeline ? (
          <AreaChart
            className="h-72"
            data={forecast.timeline}
            index="date"
            categories={['predicted', 'actual', 'lowerBound', 'upperBound']}
            colors={['blue', 'emerald', 'gray', 'gray']}
            showAnimation
            curveType="monotone"
          />
        ) : (
          <div className="flex h-72 items-center justify-center rounded-lg border border-dashed border-gray-200 dark:border-gray-700">
            <Text className="text-gray-500">
              No live demand forecast is available yet. Enable forecast coverage for at least one SKU.
            </Text>
          </div>
        )}
      </Card>

      {/* Top Products by Demand */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        <Card>
          <Title>High Demand Products</Title>
          <Text className="text-gray-500 mb-4">Products predicted to have high demand</Text>
          {hasHighDemandProducts ? (
            <div className="space-y-3">
              {topProducts.highDemand.map((product: DemandHighProduct, index: number) => (
                <div key={product.id || index} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700">
                  <div className="flex items-center space-x-3">
                    <div className="w-8 h-8 rounded-full bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center">
                      <ArrowTrendingUpIcon className="w-4 h-4 text-emerald-600" />
                    </div>
                    <div>
                      <Text className="font-medium">{product.name}</Text>
                      <Text className="text-xs text-gray-500">{product.sku}</Text>
                    </div>
                  </div>
                  <div className="text-right">
                    <Badge color="emerald">{product.growthRate >= 0 ? '+' : ''}{product.growthRate}%</Badge>
                    <Text className="text-xs text-gray-500 mt-1">
                      {formatNumber(product.predictedUnits)} units
                    </Text>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
              <Text className="text-gray-500">
                No forecast-backed product demand signals are available yet.
              </Text>
            </div>
          )}
        </Card>

        <Card>
          <Title>Restock Alerts</Title>
          <Text className="text-gray-500 mb-4">Products requiring attention</Text>
          {hasAlerts ? (
            <div className="space-y-3">
              {alerts.map((alert: DemandAlert, index: number) => (
                <div key={alert.productId || index} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700 border-amber-200 dark:border-amber-800">
                  <div className="flex items-center space-x-3">
                    <div className="w-8 h-8 rounded-full bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center">
                      <ExclamationTriangleIcon className="w-4 h-4 text-amber-600" />
                    </div>
                    <div>
                      <Text className="font-medium">{alert.productName}</Text>
                      <Text className="text-xs text-gray-500">{alert.reason}</Text>
                    </div>
                  </div>
                  <div className="text-right">
                    <Badge color="amber">{alert.daysUntilStockout} days</Badge>
                    <Text className="text-xs text-gray-500 mt-1">
                      Restock: {formatNumber(alert.recommendedQuantity)}
                    </Text>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
              <Text className="text-gray-500">No active restock alerts from live forecast data.</Text>
            </div>
          )}
        </Card>
      </Grid>

      {/* Category Demand */}
      <Card>
        <Title>Demand by Category</Title>
        <Text className="text-gray-500 mb-4">Categories covered by live forecast results</Text>
        {hasCategoryDemand ? (
          <BarChart
            className="h-64"
            data={forecast.categoryDemand}
            index="category"
            categories={['current', 'predicted']}
            colors={['gray', 'blue']}
            showAnimation
          />
        ) : (
          <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-gray-200 dark:border-gray-700">
            <Text className="text-gray-500">
              Category demand will appear after forecast coverage is available for synced inventory.
            </Text>
          </div>
        )}
      </Card>
    </motion.div>
  );
}
