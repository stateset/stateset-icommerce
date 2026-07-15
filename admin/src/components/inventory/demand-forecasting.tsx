'use client';

import { AreaChart, BarChart } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Badge,
  MetricCard,
} from '@stateset/design';
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
        <CardContent className="p-5">
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-64 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/25">
        <CardContent className="p-5">
          <p className="text-sm text-ds-status-fail">Failed to load demand forecasting data</p>
        </CardContent>
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
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard
          tone="primary"
          label="Forecast Accuracy"
          value={hasForecastTimeline ? `${accuracy.overall}%` : 'Unavailable'}
        />
        <MetricCard
          tone="success"
          label="Predicted Revenue"
          value={hasForecastTimeline ? formatCurrency(forecast.predictedRevenue) : 'Unavailable'}
        />
        <MetricCard
          tone="warning"
          label="Stock Alerts"
          value={alerts.length}
        />
        <MetricCard
          tone="accent"
          label="Trend Score"
          value={hasForecastTimeline ? forecast.trendScore : 'Unavailable'}
        />
      </div>

      {/* Demand Forecast Chart */}
      <Card>
        <CardHeader>
          <CardTitle>30-Day Demand Forecast</CardTitle>
          <CardDescription>Live forecast output from the embedded inventory engine</CardDescription>
        </CardHeader>
        <CardContent>
          {hasForecastTimeline ? (
            <AreaChart
              className="h-72"
              data={forecast.timeline}
              index="date"
              categories={['predicted', 'actual', 'lowerBound', 'upperBound']}
              colors={['indigo', 'emerald', 'violet', 'amber']}
              showAnimation
              curveType="monotone"
            />
          ) : (
            <div className="flex h-72 items-center justify-center rounded-lg border border-dashed border-ds-enterprise-line">
              <p className="text-sm text-ds-muted-foreground">
                No live demand forecast is available yet. Enable forecast coverage for at least one SKU.
              </p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Top Products by Demand */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>High Demand Products</CardTitle>
            <CardDescription>Products predicted to have high demand</CardDescription>
          </CardHeader>
          <CardContent>
            {hasHighDemandProducts ? (
              <div className="space-y-3">
                {topProducts.highDemand.map((product: DemandHighProduct, index: number) => (
                  <div key={product.id || index} className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg">
                    <div className="flex items-center space-x-3">
                      <div className="w-8 h-8 rounded-full bg-ds-status-ok/10 flex items-center justify-center">
                        <ArrowTrendingUpIcon className="w-4 h-4 text-ds-status-ok" />
                      </div>
                      <div>
                        <p className="text-sm font-medium text-ds-foreground">{product.name}</p>
                        <p className="text-xs text-ds-muted-foreground">{product.sku}</p>
                      </div>
                    </div>
                    <div className="text-right">
                      <Badge variant="success">{product.growthRate >= 0 ? '+' : ''}{product.growthRate}%</Badge>
                      <p className="text-xs text-ds-muted-foreground mt-1">
                        {formatNumber(product.predictedUnits)} units
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-dashed border-ds-enterprise-line p-6 text-center">
                <p className="text-sm text-ds-muted-foreground">
                  No forecast-backed product demand signals are available yet.
                </p>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Restock Alerts</CardTitle>
            <CardDescription>Products requiring attention</CardDescription>
          </CardHeader>
          <CardContent>
            {hasAlerts ? (
              <div className="space-y-3">
                {alerts.map((alert: DemandAlert, index: number) => (
                  <div key={alert.productId || index} className="flex items-center justify-between p-3 border rounded-lg border-ds-status-warn/25">
                    <div className="flex items-center space-x-3">
                      <div className="w-8 h-8 rounded-full bg-ds-status-warn/10 flex items-center justify-center">
                        <ExclamationTriangleIcon className="w-4 h-4 text-ds-status-warn" />
                      </div>
                      <div>
                        <p className="text-sm font-medium text-ds-foreground">{alert.productName}</p>
                        <p className="text-xs text-ds-muted-foreground">{alert.reason}</p>
                      </div>
                    </div>
                    <div className="text-right">
                      <Badge variant="warning">{alert.daysUntilStockout} days</Badge>
                      <p className="text-xs text-ds-muted-foreground mt-1">
                        Restock: {formatNumber(alert.recommendedQuantity)}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-dashed border-ds-enterprise-line p-6 text-center">
                <p className="text-sm text-ds-muted-foreground">No active restock alerts from live forecast data.</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Category Demand */}
      <Card>
        <CardHeader>
          <CardTitle>Demand by Category</CardTitle>
          <CardDescription>Categories covered by live forecast results</CardDescription>
        </CardHeader>
        <CardContent>
          {hasCategoryDemand ? (
            <BarChart
              className="h-64"
              data={forecast.categoryDemand}
              index="category"
              categories={['current', 'predicted']}
              colors={['violet', 'indigo']}
              showAnimation
            />
          ) : (
            <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-ds-enterprise-line">
              <p className="text-sm text-ds-muted-foreground">
                Category demand will appear after forecast coverage is available for synced inventory.
              </p>
            </div>
          )}
        </CardContent>
      </Card>
    </motion.div>
  );
}
