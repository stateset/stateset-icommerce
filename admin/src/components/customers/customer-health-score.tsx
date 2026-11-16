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
  MetricCard,
} from '@stateset/design';
import { HeartIcon, ExclamationCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getCustomerHealthData } from '@/app/actions/commerce';
import { formatCurrency, formatMeasured, formatNumber } from '@/lib/utils';
import type {
  CustomerHealthData,
  CustomerHealthMetric,
  AtRiskCustomer,
  CustomerSegmentDetail,
} from '@/lib/types/dashboard-data';
import { isEngineFeatureUnavailable } from '@/lib/types/dashboard-data';
import { EngineFeatureNotice } from '@/components/shared/engine-feature-notice';

interface CustomerHealthScoreProps {
  data?: CustomerHealthData;
}

const healthIconColors: Record<string, string> = {
  excellent: 'text-ds-status-ok',
  good: 'text-ds-primary',
  fair: 'text-ds-status-warn',
  at_risk: 'text-ds-status-fail',
};

function CustomerHealthScoreInner({ data: propData }: CustomerHealthScoreProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getCustomerHealthData(), {
    initialData: propData,
    refreshInterval: 60000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-64 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (isEngineFeatureUnavailable(data)) {
    return <EngineFeatureNotice title="Customer health" message={data.unavailable} />;
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load customer health data</p>
        </CardContent>
      </Card>
    );
  }

  const { summary, segments, atRiskCustomers, trends } = data;

  const segmentChartData = Object.entries(segments || {}).map(([segment, count]) => ({
    name: segment.charAt(0).toUpperCase() + segment.slice(1).replace('_', ' '),
    value: count as number,
  }));

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics. A missing figure reads as unavailable, never as a
          stand-in number (a store with no at-risk customers used to show 127). */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard
          label="Overall Health Score"
          value={formatMeasured(summary?.overallScore, (score) => `${score}/100`)}
          tone="success"
        />
        <MetricCard
          label="Total Customers"
          value={formatMeasured(summary?.totalCustomers, formatNumber)}
          tone="primary"
        />
        <MetricCard
          label="At Risk"
          value={formatMeasured(summary?.atRiskCount, String)}
          tone="warning"
        />
        <MetricCard
          label="Avg Lifetime Value"
          value={formatMeasured(summary?.avgLifetimeValue, (amount) => formatCurrency(amount))}
          tone="accent"
        />
      </div>

      {/* Health Distribution */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Customer Health Distribution</CardTitle>
            <CardDescription>Breakdown by health segment</CardDescription>
          </CardHeader>
          <CardContent>
            <DonutChart
              className="h-64"
              data={segmentChartData}
              category="value"
              index="name"
              colors={['emerald', 'indigo', 'amber', 'rose']}
              showAnimation
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Health Metrics Breakdown</CardTitle>
            <CardDescription>Contributing factors to health score</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {(summary?.metrics ?? []).length === 0 && (
                <p className="text-sm text-ds-muted-foreground">No health metrics reported</p>
              )}
              {(summary?.metrics ?? []).map((metric: CustomerHealthMetric) => (
                <div key={metric.name}>
                  <div className="flex justify-between mb-1">
                    <p className="text-sm font-medium text-ds-foreground">{metric.name}</p>
                    <p className="text-sm text-ds-muted-foreground">{metric.score}/100</p>
                  </div>
                  <ProgressBar
                    value={metric.score}
                    color={metric.score >= 70 ? 'emerald' : metric.score >= 50 ? 'amber' : 'rose'}
                  />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* At Risk Customers */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>At-Risk Customers</CardTitle>
              <CardDescription>Customers requiring immediate attention</CardDescription>
            </div>
            {atRiskCustomers && <Badge variant="danger">{atRiskCustomers.length} customers</Badge>}
          </div>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {(atRiskCustomers ?? []).length === 0 && (
              <p className="text-sm text-ds-muted-foreground">No at-risk customers</p>
            )}
            {(atRiskCustomers ?? []).map((customer: AtRiskCustomer, index: number) => (
              <motion.div
                key={customer.id || index}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.1 }}
                className="flex items-center justify-between p-4 border rounded-lg border-ds-status-fail/25 bg-ds-status-fail/10"
              >
                <div className="flex items-center space-x-4">
                  <div className="w-10 h-10 rounded-full bg-ds-status-fail/15 flex items-center justify-center">
                    <ExclamationCircleIcon className="w-5 h-5 text-ds-status-fail" />
                  </div>
                  <div>
                    <p className="text-sm font-medium text-ds-foreground">{customer.name}</p>
                    <p className="text-xs text-ds-muted-foreground">{customer.email}</p>
                  </div>
                </div>
                <div className="flex items-center space-x-4">
                  <div className="text-right">
                    <p className="text-sm font-medium text-ds-foreground">
                      Health: {customer.healthScore}
                    </p>
                    <p className="text-xs text-ds-status-fail">{customer.riskReason}</p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm text-ds-foreground">
                      LTV: {formatCurrency(customer.lifetimeValue)}
                    </p>
                    <p className="text-xs text-ds-muted-foreground">
                      Last order: {customer.daysSinceLastOrder}d ago
                    </p>
                  </div>
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Customer Trends */}
      <Card>
        <CardHeader>
          <CardTitle>Health Score Trends</CardTitle>
          <CardDescription>Average health score over time</CardDescription>
        </CardHeader>
        <CardContent>
          <BarChart
            className="h-64"
            data={trends?.timeline ?? []}
            index="month"
            categories={['excellent', 'good', 'fair', 'atRisk']}
            colors={['emerald', 'indigo', 'amber', 'rose']}
            stack
            showAnimation
          />
        </CardContent>
      </Card>

      {/* Segment Details */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {(Object.entries(segments ?? {}) as [string, number | CustomerSegmentDetail][]).map(
          ([segment, data]) => (
            <Card key={segment}>
              <CardContent className="p-5">
                <div className="flex items-center space-x-2 mb-2">
                  <HeartIcon
                    className={`w-5 h-5 ${healthIconColors[segment] || 'text-ds-muted-foreground'}`}
                  />
                  <p className="text-sm font-medium text-ds-foreground capitalize">
                    {segment.replace('_', ' ')}
                  </p>
                </div>
                <p className="ds-instrument-number text-3xl text-ds-foreground">
                  {typeof data === 'number' ? data : data.count}
                </p>
                <p className="text-xs text-ds-muted-foreground mt-1">
                  {typeof data === 'number'
                    ? 'customers'
                    : `Avg LTV: ${formatCurrency(data.avgLtv)}`}
                </p>
              </CardContent>
            </Card>
          ),
        )}
      </div>
    </motion.div>
  );
}

const CustomerHealthScore = memo(CustomerHealthScoreInner);
export default CustomerHealthScore;
