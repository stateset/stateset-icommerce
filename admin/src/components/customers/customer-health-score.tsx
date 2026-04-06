'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, DonutChart, BarChart, ProgressBar } from '@tremor/react';
import { HeartIcon, ExclamationCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getCustomerHealthData } from '@/app/actions/commerce';
import { formatCurrency, formatNumber } from '@/lib/utils';
import type { CustomerHealthData, CustomerHealthMetric, AtRiskCustomer, CustomerSegmentDetail, TremorColor } from '@/lib/types/dashboard-data';

interface CustomerHealthScoreProps {
  data?: CustomerHealthData;
}

const healthColors: Record<string, string> = {
  excellent: 'emerald',
  good: 'blue',
  fair: 'amber',
  at_risk: 'red',
};

function CustomerHealthScoreInner({ data: propData }: CustomerHealthScoreProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getCustomerHealthData(),
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
        <Text className="text-red-600">Failed to load customer health data</Text>
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
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <Text>Overall Health Score</Text>
          <Metric>{summary?.overallScore || 78}/100</Metric>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Total Customers</Text>
          <Metric>{formatNumber(summary?.totalCustomers || 2450)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>At Risk</Text>
          <Metric>{summary?.atRiskCount || 127}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Avg Lifetime Value</Text>
          <Metric>{formatCurrency(summary?.avgLifetimeValue || 485)}</Metric>
        </Card>
      </Grid>

      {/* Health Distribution */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        <Card>
          <Title>Customer Health Distribution</Title>
          <Text className="text-gray-500 mb-4">Breakdown by health segment</Text>
          <DonutChart
            className="h-64"
            data={segmentChartData.length > 0 ? segmentChartData : generateDemoSegments()}
            category="value"
            index="name"
            colors={['emerald', 'blue', 'amber', 'red']}
            showAnimation
          />
        </Card>

        <Card>
          <Title>Health Metrics Breakdown</Title>
          <Text className="text-gray-500 mb-4">Contributing factors to health score</Text>
          <div className="space-y-4">
            {(summary?.metrics || generateDemoMetrics()).map((metric: CustomerHealthMetric) => (
              <div key={metric.name}>
                <div className="flex justify-between mb-1">
                  <Text className="font-medium">{metric.name}</Text>
                  <Text className="text-sm">{metric.score}/100</Text>
                </div>
                <ProgressBar
                  value={metric.score}
                  color={metric.score >= 70 ? 'emerald' : metric.score >= 50 ? 'amber' : 'red'}
                />
              </div>
            ))}
          </div>
        </Card>
      </Grid>

      {/* At Risk Customers */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>At-Risk Customers</Title>
            <Text className="text-gray-500">Customers requiring immediate attention</Text>
          </div>
          <Badge color="red" size="lg">
            {atRiskCustomers?.length || 5} customers
          </Badge>
        </div>

        <div className="space-y-3">
          {(atRiskCustomers || generateDemoAtRiskCustomers()).map((customer: AtRiskCustomer, index: number) => (
            <motion.div
              key={customer.id || index}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: index * 0.1 }}
              className="flex items-center justify-between p-4 border rounded-lg dark:border-gray-700 border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/10"
            >
              <div className="flex items-center space-x-4">
                <div className="w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
                  <ExclamationCircleIcon className="w-5 h-5 text-red-600" />
                </div>
                <div>
                  <Text className="font-medium">{customer.name}</Text>
                  <Text className="text-xs text-gray-500">{customer.email}</Text>
                </div>
              </div>
              <div className="flex items-center space-x-4">
                <div className="text-right">
                  <Text className="text-sm font-medium">Health: {customer.healthScore}</Text>
                  <Text className="text-xs text-red-600">{customer.riskReason}</Text>
                </div>
                <div className="text-right">
                  <Text className="text-sm">LTV: {formatCurrency(customer.lifetimeValue)}</Text>
                  <Text className="text-xs text-gray-500">
                    Last order: {customer.daysSinceLastOrder}d ago
                  </Text>
                </div>
              </div>
            </motion.div>
          ))}
        </div>
      </Card>

      {/* Customer Trends */}
      <Card>
        <Title>Health Score Trends</Title>
        <Text className="text-gray-500 mb-4">Average health score over time</Text>
        <BarChart
          className="h-64"
          data={trends?.timeline || generateDemoTrends()}
          index="month"
          categories={['excellent', 'good', 'fair', 'atRisk']}
          colors={['emerald', 'blue', 'amber', 'red']}
          stack
          showAnimation
        />
      </Card>

      {/* Segment Details */}
      <Grid numItems={1} numItemsSm={2} numItemsLg={4} className="gap-4">
        {(Object.entries(segments || generateDemoSegmentDetails()) as [string, number | CustomerSegmentDetail][]).map(([segment, data]) => (
          <Card key={segment} decoration="left" decorationColor={healthColors[segment] as TremorColor || 'gray'}>
            <div className="flex items-center space-x-2 mb-2">
              <HeartIcon className={`w-5 h-5 text-${healthColors[segment] || 'gray'}-600`} />
              <Text className="font-medium capitalize">{segment.replace('_', ' ')}</Text>
            </div>
            <Metric>{typeof data === 'number' ? data : data.count}</Metric>
            <Text className="text-xs text-gray-500 mt-1">
              {typeof data === 'number' ? 'customers' : `Avg LTV: ${formatCurrency(data.avgLtv)}`}
            </Text>
          </Card>
        ))}
      </Grid>
    </motion.div>
  );
}

// Demo data generators
function generateDemoSegments() {
  return [
    { name: 'Excellent', value: 850 },
    { name: 'Good', value: 1200 },
    { name: 'Fair', value: 280 },
    { name: 'At Risk', value: 120 },
  ];
}

function generateDemoMetrics() {
  return [
    { name: 'Purchase Frequency', score: 82 },
    { name: 'Order Value', score: 75 },
    { name: 'Engagement', score: 68 },
    { name: 'Support Interactions', score: 88 },
    { name: 'Return Rate', score: 91 },
  ];
}

function generateDemoAtRiskCustomers() {
  return [
    { id: '1', name: 'John Smith', email: 'john@example.com', healthScore: 25, riskReason: 'No orders in 90 days', lifetimeValue: 1250, daysSinceLastOrder: 95 },
    { id: '2', name: 'Sarah Johnson', email: 'sarah@example.com', healthScore: 32, riskReason: 'High return rate', lifetimeValue: 890, daysSinceLastOrder: 45 },
    { id: '3', name: 'Mike Wilson', email: 'mike@example.com', healthScore: 28, riskReason: 'Negative feedback', lifetimeValue: 2100, daysSinceLastOrder: 60 },
    { id: '4', name: 'Emily Brown', email: 'emily@example.com', healthScore: 35, riskReason: 'Declining order value', lifetimeValue: 650, daysSinceLastOrder: 30 },
  ];
}

function generateDemoTrends() {
  return [
    { month: 'Jul', excellent: 780, good: 1100, fair: 320, atRisk: 150 },
    { month: 'Aug', excellent: 800, good: 1120, fair: 300, atRisk: 140 },
    { month: 'Sep', excellent: 820, good: 1150, fair: 290, atRisk: 130 },
    { month: 'Oct', excellent: 835, good: 1180, fair: 280, atRisk: 125 },
    { month: 'Nov', excellent: 850, good: 1200, fair: 280, atRisk: 120 },
  ];
}

function generateDemoSegmentDetails() {
  return {
    excellent: { count: 850, avgLtv: 1250 },
    good: { count: 1200, avgLtv: 680 },
    fair: { count: 280, avgLtv: 320 },
    at_risk: { count: 120, avgLtv: 450 },
  };
}

const CustomerHealthScore = memo(CustomerHealthScoreInner);
export default CustomerHealthScore;
