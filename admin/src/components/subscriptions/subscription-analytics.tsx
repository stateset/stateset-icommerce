'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, AreaChart, DonutChart, ProgressBar } from '@tremor/react';
import { CreditCardIcon, ArrowTrendingUpIcon, UserMinusIcon, CalendarIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSubscriptionAnalyticsData } from '@/app/actions/commerce';
import { formatCurrency, formatNumber, formatPercentage } from '@/lib/utils';
import type { SubscriptionAnalyticsData, ChurnReason, UpcomingRenewal, TremorColor } from '@/lib/types/dashboard-data';

interface SubscriptionAnalyticsProps {
  data?: SubscriptionAnalyticsData;
}

const statusColors: Record<string, string> = {
  active: 'emerald',
  trialing: 'blue',
  past_due: 'amber',
  canceled: 'red',
  paused: 'gray',
};

function SubscriptionAnalyticsInner({ data: propData }: SubscriptionAnalyticsProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getSubscriptionAnalyticsData(),
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
        <Text className="text-red-600">Failed to load subscription analytics</Text>
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
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <Text>Monthly Recurring Revenue</Text>
          <Metric>{formatCurrency(summary?.mrr || 45000)}</Metric>
          <Text className="text-xs text-emerald-600 mt-1">
            +{formatPercentage(summary?.mrrGrowth || 0.12)} vs last month
          </Text>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Active Subscriptions</Text>
          <Metric>{formatNumber(summary?.activeCount || 1250)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Churn Rate</Text>
          <Metric>{formatPercentage(summary?.churnRate || 0.032)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Avg Revenue/User</Text>
          <Metric>{formatCurrency(summary?.arpu || 36)}</Metric>
        </Card>
      </Grid>

      {/* MRR Trend */}
      <Card>
        <Title>MRR Growth Trend</Title>
        <Text className="text-gray-500 mb-4">Monthly recurring revenue over time</Text>
        <AreaChart
          className="h-72"
          data={mrrTrend || generateDemoMrrTrend()}
          index="month"
          categories={['mrr', 'newMrr', 'churnedMrr']}
          colors={['emerald', 'blue', 'red']}
          showAnimation
          curveType="monotone"
          valueFormatter={(value) => formatCurrency(value)}
        />
      </Card>

      {/* Charts Row */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        {/* Plan Distribution */}
        <Card>
          <Title>Plan Distribution</Title>
          <Text className="text-gray-500 mb-4">Subscribers by plan type</Text>
          <DonutChart
            className="h-64"
            data={planDistribution || generateDemoPlanDistribution()}
            category="count"
            index="plan"
            colors={['emerald', 'blue', 'purple', 'amber']}
            showAnimation
            valueFormatter={(value) => `${value} subscribers`}
          />
        </Card>

        {/* Churn Analysis */}
        <Card>
          <Title>Churn Analysis</Title>
          <Text className="text-gray-500 mb-4">Cancellation reasons</Text>
          <div className="space-y-4">
            {(churnAnalysis?.reasons || generateDemoChurnReasons()).map((reason: ChurnReason) => (
              <div key={reason.name}>
                <div className="flex justify-between mb-1">
                  <Text className="font-medium">{reason.name}</Text>
                  <div className="flex items-center space-x-2">
                    <Badge color="gray" size="xs">{reason.count}</Badge>
                    <Text className="text-sm">{formatPercentage(reason.percentage)}</Text>
                  </div>
                </div>
                <ProgressBar
                  value={reason.percentage * 100}
                  color="red"
                />
              </div>
            ))}
          </div>
        </Card>
      </Grid>

      {/* Subscription Status Breakdown */}
      <Card>
        <Title>Subscription Status</Title>
        <Text className="text-gray-500 mb-4">Current status distribution</Text>
        <Grid numItems={2} numItemsSm={5} className="gap-4">
          {Object.entries(summary?.statusBreakdown || generateDemoStatusBreakdown()).map(([status, count]) => (
            <div key={status} className="text-center p-4 border rounded-lg dark:border-gray-700">
              <Badge color={statusColors[status] as TremorColor || 'gray'} size="lg">
                {status.replace('_', ' ')}
              </Badge>
              <Metric className="mt-2">{count as number}</Metric>
            </div>
          ))}
        </Grid>
      </Card>

      {/* Upcoming Renewals */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Upcoming Renewals</Title>
            <Text className="text-gray-500">Subscriptions renewing in the next 7 days</Text>
          </div>
          <Badge color="blue" size="lg">
            {formatCurrency((upcomingRenewals || []).reduce((sum: number, r: UpcomingRenewal) => sum + r.amount, 0))} expected
          </Badge>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Customer</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Plan</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Renewal Date</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Amount</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Risk</th>
              </tr>
            </thead>
            <tbody>
              {(upcomingRenewals || generateDemoUpcomingRenewals()).map((renewal: UpcomingRenewal, index: number) => (
                <motion.tr
                  key={renewal.id || index}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: index * 0.05 }}
                  className="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                >
                  <td className="py-3 px-3">
                    <div>
                      <Text className="font-medium">{renewal.customerName}</Text>
                      <Text className="text-xs text-gray-500">{renewal.email}</Text>
                    </div>
                  </td>
                  <td className="py-3 px-3">
                    <Badge color="blue" size="xs">{renewal.plan}</Badge>
                  </td>
                  <td className="py-3 px-3">
                    <div className="flex items-center space-x-2">
                      <CalendarIcon className="w-4 h-4 text-gray-400" />
                      <Text className="text-sm">{renewal.renewalDate}</Text>
                    </div>
                  </td>
                  <td className="py-3 px-3 text-right">
                    <Text className="font-medium">{formatCurrency(renewal.amount)}</Text>
                  </td>
                  <td className="py-3 px-3">
                    <Badge
                      color={renewal.churnRisk < 0.2 ? 'emerald' : renewal.churnRisk < 0.5 ? 'amber' : 'red'}
                      size="xs"
                    >
                      {renewal.churnRisk < 0.2 ? 'Low' : renewal.churnRisk < 0.5 ? 'Medium' : 'High'}
                    </Badge>
                  </td>
                </motion.tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      {/* Revenue Metrics */}
      <Grid numItems={1} numItemsSm={3} className="gap-4">
        <Card decoration="left" decorationColor="emerald">
          <div className="flex items-center space-x-2">
            <ArrowTrendingUpIcon className="w-5 h-5 text-emerald-600" />
            <Text className="font-medium">New MRR</Text>
          </div>
          <Metric className="mt-2">{formatCurrency(summary?.newMrr || 5200)}</Metric>
          <Text className="text-xs text-gray-500">From {summary?.newSubscribers || 145} new subscribers</Text>
        </Card>
        <Card decoration="left" decorationColor="blue">
          <div className="flex items-center space-x-2">
            <CreditCardIcon className="w-5 h-5 text-blue-600" />
            <Text className="font-medium">Expansion MRR</Text>
          </div>
          <Metric className="mt-2">{formatCurrency(summary?.expansionMrr || 1800)}</Metric>
          <Text className="text-xs text-gray-500">From {summary?.upgrades || 32} upgrades</Text>
        </Card>
        <Card decoration="left" decorationColor="red">
          <div className="flex items-center space-x-2">
            <UserMinusIcon className="w-5 h-5 text-red-600" />
            <Text className="font-medium">Churned MRR</Text>
          </div>
          <Metric className="mt-2">{formatCurrency(summary?.churnedMrr || 1400)}</Metric>
          <Text className="text-xs text-gray-500">From {summary?.cancelations || 38} cancelations</Text>
        </Card>
      </Grid>
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
    { name: 'Other', count: 3, percentage: 0.10 },
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
    { id: '1', customerName: 'Acme Corp', email: 'billing@acme.com', plan: 'Business', renewalDate: '2024-12-22', amount: 99, churnRisk: 0.1 },
    { id: '2', customerName: 'TechStart Inc', email: 'admin@techstart.io', plan: 'Pro', renewalDate: '2024-12-23', amount: 49, churnRisk: 0.3 },
    { id: '3', customerName: 'Global Retail', email: 'it@globalretail.com', plan: 'Enterprise', renewalDate: '2024-12-24', amount: 299, churnRisk: 0.15 },
    { id: '4', customerName: 'Local Shop', email: 'owner@localshop.com', plan: 'Basic', renewalDate: '2024-12-25', amount: 19, churnRisk: 0.55 },
    { id: '5', customerName: 'Digital Agency', email: 'accounts@digital.agency', plan: 'Pro', renewalDate: '2024-12-26', amount: 49, churnRisk: 0.2 },
  ];
}

const SubscriptionAnalytics = memo(SubscriptionAnalyticsInner);
export default SubscriptionAnalytics;
