'use client';

import { memo } from 'react';
import { AreaChart, DonutChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Badge,
  MetricCard,
  StatusPill,
} from '@stateset/design';
import {
  CreditCardIcon,
  ArrowTrendingUpIcon,
  UserMinusIcon,
  CalendarIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSubscriptionAnalyticsData } from '@/app/actions/commerce';
import { formatCurrency, formatNumber, formatPercentage } from '@/lib/utils';
import type {
  SubscriptionAnalyticsData,
  ChurnReason,
  UpcomingRenewal,
} from '@/lib/types/dashboard-data';

type DsStatus = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';

interface SubscriptionAnalyticsProps {
  data?: SubscriptionAnalyticsData;
}

const statusPills: Record<string, DsStatus> = {
  active: 'ok',
  trialing: 'run',
  past_due: 'warn',
  canceled: 'fail',
  paused: 'idle',
};

function SubscriptionAnalyticsInner({ data: propData }: SubscriptionAnalyticsProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getSubscriptionAnalyticsData(), {
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

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load subscription analytics</p>
        </CardContent>
      </Card>
    );
  }

  const { summary, mrrTrend, churnAnalysis, planDistribution, upcomingRenewals } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard
          label="Monthly Recurring Revenue"
          value={formatCurrency(summary?.mrr || 45000)}
          subtitle={`+${formatPercentage(summary?.mrrGrowth || 0.12)} vs last month`}
          tone="success"
        />
        <MetricCard
          label="Active Subscriptions"
          value={formatNumber(summary?.activeCount || 1250)}
          tone="primary"
        />
        <MetricCard
          label="Churn Rate"
          value={formatPercentage(summary?.churnRate || 0.032)}
          tone="warning"
        />
        <MetricCard
          label="Avg Revenue/User"
          value={formatCurrency(summary?.arpu || 36)}
          tone="accent"
        />
      </div>

      {/* MRR Trend */}
      <Card>
        <CardHeader>
          <CardTitle>MRR Growth Trend</CardTitle>
          <CardDescription>Monthly recurring revenue over time</CardDescription>
        </CardHeader>
        <CardContent>
          <AreaChart
            className="h-72"
            data={mrrTrend || generateDemoMrrTrend()}
            index="month"
            categories={['mrr', 'newMrr', 'churnedMrr']}
            colors={['emerald', 'indigo', 'rose']}
            showAnimation
            curveType="monotone"
            valueFormatter={(value) => formatCurrency(value)}
          />
        </CardContent>
      </Card>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Plan Distribution */}
        <Card>
          <CardHeader>
            <CardTitle>Plan Distribution</CardTitle>
            <CardDescription>Subscribers by plan type</CardDescription>
          </CardHeader>
          <CardContent>
            <DonutChart
              className="h-64"
              data={planDistribution || generateDemoPlanDistribution()}
              category="count"
              index="plan"
              colors={['indigo', 'emerald', 'violet', 'amber']}
              showAnimation
              valueFormatter={(value) => `${value} subscribers`}
            />
          </CardContent>
        </Card>

        {/* Churn Analysis */}
        <Card>
          <CardHeader>
            <CardTitle>Churn Analysis</CardTitle>
            <CardDescription>Cancellation reasons</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {(churnAnalysis?.reasons || generateDemoChurnReasons()).map((reason: ChurnReason) => (
                <div key={reason.name}>
                  <div className="flex justify-between mb-1">
                    <p className="text-sm font-medium text-ds-foreground">{reason.name}</p>
                    <div className="flex items-center space-x-2">
                      <Badge variant="default">{reason.count}</Badge>
                      <p className="text-sm text-ds-muted-foreground">
                        {formatPercentage(reason.percentage)}
                      </p>
                    </div>
                  </div>
                  <ProgressBar value={reason.percentage * 100} color="rose" />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Subscription Status Breakdown */}
      <Card>
        <CardHeader>
          <CardTitle>Subscription Status</CardTitle>
          <CardDescription>Current status distribution</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 sm:grid-cols-5 gap-4">
            {Object.entries(summary?.statusBreakdown || generateDemoStatusBreakdown()).map(
              ([status, count]) => (
                <div
                  key={status}
                  className="text-center p-4 border border-ds-enterprise-line rounded-lg"
                >
                  <StatusPill status={statusPills[status] || 'idle'}>
                    {status.replace('_', ' ')}
                  </StatusPill>
                  <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
                    {count as number}
                  </p>
                </div>
              ),
            )}
          </div>
        </CardContent>
      </Card>

      {/* Upcoming Renewals */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Upcoming Renewals</CardTitle>
              <CardDescription>Subscriptions renewing in the next 7 days</CardDescription>
            </div>
            <Badge variant="primary">
              {formatCurrency(
                (upcomingRenewals || []).reduce(
                  (sum: number, r: UpcomingRenewal) => sum + r.amount,
                  0,
                ),
              )}{' '}
              expected
            </Badge>
          </div>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-ds-enterprise-line">
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Customer
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Plan
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Renewal Date
                  </th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Amount
                  </th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                    Risk
                  </th>
                </tr>
              </thead>
              <tbody>
                {(upcomingRenewals || generateDemoUpcomingRenewals()).map(
                  (renewal: UpcomingRenewal, index: number) => (
                    <motion.tr
                      key={renewal.id || index}
                      initial={{ opacity: 0, x: -20 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ delay: index * 0.05 }}
                      className="border-b border-ds-enterprise-line hover:bg-ds-muted"
                    >
                      <td className="py-3 px-3">
                        <div>
                          <p className="text-sm font-medium text-ds-foreground">
                            {renewal.customerName}
                          </p>
                          <p className="text-xs text-ds-muted-foreground">{renewal.email}</p>
                        </div>
                      </td>
                      <td className="py-3 px-3">
                        <Badge variant="primary">{renewal.plan}</Badge>
                      </td>
                      <td className="py-3 px-3">
                        <div className="flex items-center space-x-2">
                          <CalendarIcon className="w-4 h-4 text-ds-muted-foreground" />
                          <p className="text-sm text-ds-foreground">{renewal.renewalDate}</p>
                        </div>
                      </td>
                      <td className="py-3 px-3 text-right">
                        <p className="text-sm font-medium text-ds-foreground">
                          {formatCurrency(renewal.amount)}
                        </p>
                      </td>
                      <td className="py-3 px-3">
                        <StatusPill
                          status={
                            renewal.churnRisk < 0.2
                              ? 'ok'
                              : renewal.churnRisk < 0.5
                                ? 'warn'
                                : 'fail'
                          }
                        >
                          {renewal.churnRisk < 0.2
                            ? 'Low'
                            : renewal.churnRisk < 0.5
                              ? 'Medium'
                              : 'High'}
                        </StatusPill>
                      </td>
                    </motion.tr>
                  ),
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {/* Revenue Metrics */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <ArrowTrendingUpIcon className="w-5 h-5 text-ds-status-ok" />
              <p className="text-sm font-medium text-ds-foreground">New MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatCurrency(summary?.newMrr || 5200)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              From {summary?.newSubscribers || 145} new subscribers
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <CreditCardIcon className="w-5 h-5 text-ds-primary" />
              <p className="text-sm font-medium text-ds-foreground">Expansion MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatCurrency(summary?.expansionMrr || 1800)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              From {summary?.upgrades || 32} upgrades
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <UserMinusIcon className="w-5 h-5 text-ds-status-fail" />
              <p className="text-sm font-medium text-ds-foreground">Churned MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatCurrency(summary?.churnedMrr || 1400)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              From {summary?.cancellations || 38} cancellations
            </p>
          </CardContent>
        </Card>
      </div>
    </motion.div>
  );
}

// Demo data generators
function generateDemoMrrTrend() {
  return [
    { month: 'Jun', mrr: 38000, newMrr: 4200, churnedMrr: 1100 },
    { month: 'Jul', mrr: 40000, newMrr: 4500, churnedMrr: 1200 },
    { month: 'Aug', mrr: 41500, newMrr: 4800, churnedMrr: 1300 },
    { month: 'Sep', mrr: 43000, newMrr: 5000, churnedMrr: 1200 },
    { month: 'Oct', mrr: 44200, newMrr: 5100, churnedMrr: 1350 },
    { month: 'Nov', mrr: 45000, newMrr: 5200, churnedMrr: 1400 },
  ];
}

function generateDemoPlanDistribution() {
  return [
    { plan: 'Basic', count: 450, revenue: 8550 },
    { plan: 'Pro', count: 520, revenue: 20280 },
    { plan: 'Business', count: 230, revenue: 13800 },
    { plan: 'Enterprise', count: 50, revenue: 12500 },
  ];
}

function generateDemoChurnReasons() {
  return [
    { name: 'Too expensive', count: 15, percentage: 0.38 },
    { name: 'Missing features', count: 8, percentage: 0.21 },
    { name: 'Switched competitor', count: 7, percentage: 0.18 },
    { name: 'No longer needed', count: 5, percentage: 0.13 },
    { name: 'Other', count: 3, percentage: 0.1 },
  ];
}

function generateDemoStatusBreakdown() {
  return {
    active: 1150,
    trialing: 85,
    past_due: 12,
    paused: 25,
    canceled: 38,
  };
}

function generateDemoUpcomingRenewals() {
  return [
    {
      id: '1',
      customerName: 'Acme Corp',
      email: 'billing@acme.com',
      plan: 'Business',
      renewalDate: '2024-12-22',
      amount: 99,
      churnRisk: 0.1,
    },
    {
      id: '2',
      customerName: 'TechStart Inc',
      email: 'admin@techstart.io',
      plan: 'Pro',
      renewalDate: '2024-12-23',
      amount: 49,
      churnRisk: 0.3,
    },
    {
      id: '3',
      customerName: 'Global Retail',
      email: 'it@globalretail.com',
      plan: 'Enterprise',
      renewalDate: '2024-12-24',
      amount: 299,
      churnRisk: 0.15,
    },
    {
      id: '4',
      customerName: 'Local Shop',
      email: 'owner@localshop.com',
      plan: 'Basic',
      renewalDate: '2024-12-25',
      amount: 19,
      churnRisk: 0.55,
    },
    {
      id: '5',
      customerName: 'Digital Agency',
      email: 'accounts@digital.agency',
      plan: 'Pro',
      renewalDate: '2024-12-26',
      amount: 49,
      churnRisk: 0.2,
    },
  ];
}

const SubscriptionAnalytics = memo(SubscriptionAnalyticsInner);
export default SubscriptionAnalytics;
