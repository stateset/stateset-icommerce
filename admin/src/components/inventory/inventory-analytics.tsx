'use client';

import { memo } from 'react';
import { DonutChart, BarChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Badge,
  StatusPill,
  MetricCard,
} from '@stateset/design';
import {
  ExclamationTriangleIcon,
  ArrowTrendingUpIcon,
  ArrowTrendingDownIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getInventoryAnalyticsData } from '@/app/actions/commerce';
import { formatCurrency, formatCompactNumber } from '@/lib/utils';
import type {
  InventoryAnalyticsData,
  InventoryCategory,
  TopMovingItem,
  SlowMovingItem,
} from '@/lib/types/dashboard-data';
import type { InventoryItem } from '@/lib/types';

interface InventoryAnalyticsProps {
  data?: InventoryAnalyticsData;
}

function InventoryAnalyticsInner({ data: propData }: InventoryAnalyticsProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getInventoryAnalyticsData(), {
    initialData: propData,
    refreshInterval: 60000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent className="p-5">
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
      <Card className="border-ds-status-fail/25">
        <CardContent className="p-5">
          <p className="text-sm text-ds-status-fail">Failed to load inventory analytics</p>
        </CardContent>
      </Card>
    );
  }

  const healthScore = Math.max(0, 100 - data.lowStockItems * 2 - data.outOfStockItems * 5);
  const healthStatus = healthScore >= 80 ? 'ok' : healthScore >= 60 ? 'warn' : 'fail';
  const healthLabel = healthScore >= 80 ? 'Healthy' : healthScore >= 60 ? 'Fair' : 'Critical';
  const healthTextClass =
    healthScore >= 80
      ? 'text-ds-status-ok'
      : healthScore >= 60
        ? 'text-ds-status-warn'
        : 'text-ds-status-fail';
  const healthBarColor = healthScore >= 80 ? 'emerald' : healthScore >= 60 ? 'amber' : 'rose';

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
        <MetricCard tone="primary" label="Total SKUs" value={formatCompactNumber(data.totalSKUs)} />
        <MetricCard
          tone="success"
          label="Total Units"
          value={formatCompactNumber(data.totalUnits)}
        />
        <MetricCard tone="primary" label="Total Value" value={formatCurrency(data.totalValue)} />
        <Card className="p-5">
          <p className="text-sm text-ds-muted-foreground">Low Stock</p>
          <p className="ds-instrument-number text-3xl text-ds-foreground">{data.lowStockItems}</p>
          {data.lowStockItems > 0 && (
            <Badge variant="warning" className="mt-1">
              Needs attention
            </Badge>
          )}
        </Card>
        <Card className="p-5">
          <p className="text-sm text-ds-muted-foreground">Out of Stock</p>
          <p className="ds-instrument-number text-3xl text-ds-foreground">{data.outOfStockItems}</p>
          {data.outOfStockItems > 0 && (
            <Badge variant="danger" className="mt-1">
              Critical
            </Badge>
          )}
        </Card>
        <MetricCard
          tone="accent"
          label="Turnover Rate"
          value={`${(data.turnoverRate || 0).toFixed(1)}x`}
        />
      </div>

      {/* Inventory Health */}
      <Card>
        <CardContent className="p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                Inventory Health Score
              </h3>
              <p className="text-sm text-ds-muted-foreground">Overall inventory status</p>
            </div>
            <div className="text-right">
              <p className={`ds-instrument-number text-3xl ${healthTextClass}`}>{healthScore}%</p>
              <StatusPill status={healthStatus}>{healthLabel}</StatusPill>
            </div>
          </div>

          <ProgressBar value={healthScore} color={healthBarColor} />

          <div className="mt-4 grid grid-cols-3 gap-4 text-center">
            <div className="p-3 bg-ds-status-ok/10 rounded-lg">
              <p className="text-sm font-medium text-ds-status-ok">In Stock</p>
              <p className="text-2xl font-bold text-ds-status-ok">
                {data.totalSKUs - data.lowStockItems - data.outOfStockItems}
              </p>
            </div>
            <div className="p-3 bg-ds-status-warn/10 rounded-lg">
              <p className="text-sm font-medium text-ds-status-warn">Low Stock</p>
              <p className="text-2xl font-bold text-ds-status-warn">{data.lowStockItems}</p>
            </div>
            <div className="p-3 bg-ds-status-fail/10 rounded-lg">
              <p className="text-sm font-medium text-ds-status-fail">Out of Stock</p>
              <p className="text-2xl font-bold text-ds-status-fail">{data.outOfStockItems}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Category Distribution */}
        <Card>
          <CardHeader>
            <CardTitle>Inventory by Category</CardTitle>
            <CardDescription>Units distribution</CardDescription>
          </CardHeader>
          <CardContent>
            {data.categories && data.categories.length > 0 ? (
              <DonutChart
                className="h-64"
                data={data.categories.map((c: InventoryCategory) => ({
                  name: c.name,
                  value: c.units,
                }))}
                category="value"
                index="name"
                colors={['indigo', 'emerald', 'violet', 'amber', 'cyan', 'pink']}
                showAnimation
              />
            ) : (
              <div className="h-64 flex items-center justify-center">
                <p className="text-sm text-ds-muted-foreground">No category data available</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Category Value */}
        <Card>
          <CardHeader>
            <CardTitle>Value by Category</CardTitle>
            <CardDescription>Inventory value distribution</CardDescription>
          </CardHeader>
          <CardContent>
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
                <p className="text-sm text-ds-muted-foreground">No value data available</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Top Moving Items */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2 mb-4">
              <ArrowTrendingUpIcon className="w-5 h-5 text-ds-status-ok" />
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                Top Moving Items
              </h3>
            </div>
            <div className="space-y-3">
              {(data.topMovingItems || []).slice(0, 5).map((item: TopMovingItem, index: number) => (
                <div
                  key={item.sku}
                  className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg"
                >
                  <div className="flex items-center space-x-3">
                    <div className="w-8 h-8 bg-ds-status-ok/10 rounded-full flex items-center justify-center">
                      <span className="text-sm font-bold text-ds-status-ok">{index + 1}</span>
                    </div>
                    <div>
                      <p className="text-sm font-medium text-ds-foreground">{item.name}</p>
                      <p className="text-xs text-ds-muted-foreground">{item.sku}</p>
                    </div>
                  </div>
                  <Badge variant="success">{item.velocity} units/day</Badge>
                </div>
              ))}
              {(!data.topMovingItems || data.topMovingItems.length === 0) && (
                <p className="text-sm text-ds-muted-foreground text-center py-4">
                  No data available
                </p>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2 mb-4">
              <ArrowTrendingDownIcon className="w-5 h-5 text-ds-status-fail" />
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                Slow Moving Items
              </h3>
            </div>
            <div className="space-y-3">
              {(data.slowMovingItems || [])
                .slice(0, 5)
                .map((item: SlowMovingItem, index: number) => (
                  <div
                    key={item.sku}
                    className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg"
                  >
                    <div className="flex items-center space-x-3">
                      <div className="w-8 h-8 bg-ds-status-fail/10 rounded-full flex items-center justify-center">
                        <span className="text-sm font-bold text-ds-status-fail">{index + 1}</span>
                      </div>
                      <div>
                        <p className="text-sm font-medium text-ds-foreground">{item.name}</p>
                        <p className="text-xs text-ds-muted-foreground">{item.sku}</p>
                      </div>
                    </div>
                    <Badge variant="danger">{item.daysSinceLastSale}+ days</Badge>
                  </div>
                ))}
              {(!data.slowMovingItems || data.slowMovingItems.length === 0) && (
                <p className="text-sm text-ds-muted-foreground text-center py-4">
                  No data available
                </p>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Critical Items Alert */}
      {data.criticalItems && data.criticalItems.length > 0 && (
        <Card className="border-ds-status-fail/25 bg-ds-status-fail/10">
          <CardContent className="p-5">
            <div className="flex items-center space-x-2 mb-4">
              <ExclamationTriangleIcon className="w-5 h-5 text-ds-status-fail" />
              <h3 className="font-ds-display text-base font-semibold text-ds-status-fail">
                Critical Stock Alerts
              </h3>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
              {data.criticalItems.slice(0, 6).map((item: InventoryItem) => (
                <div
                  key={item.sku}
                  className="bg-ds-card p-3 rounded-lg border border-ds-status-fail/25"
                >
                  <p className="text-sm font-medium text-ds-foreground">{item.productName}</p>
                  <p className="text-xs text-ds-muted-foreground font-mono">{item.sku}</p>
                  <div className="flex items-center justify-between mt-2">
                    <p className="text-sm font-medium text-ds-status-fail">
                      {item.availableQuantity} left
                    </p>
                    <Badge variant="danger">Reorder: {item.reorderPoint}</Badge>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </motion.div>
  );
}

const InventoryAnalytics = memo(InventoryAnalyticsInner);
export default InventoryAnalytics;
