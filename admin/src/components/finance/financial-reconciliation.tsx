'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, AreaChart, BarChart, DonutChart, ProgressBar } from '@tremor/react';
import { ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getFinancialReconciliationData } from '@/app/actions/commerce';
import { SimulatedDataBadge } from '@/components/shared/simulated-data-badge';
import { formatCurrency, formatPercentage } from '@/lib/utils';
import type { FinancialReconciliationData, ReconciliationCategory, DiscrepancyItem, ReconciliationTransaction, TremorColor } from '@/lib/types/dashboard-data';

interface FinancialReconciliationProps {
  data?: FinancialReconciliationData;
}

const statusColors: Record<string, string> = {
  reconciled: 'emerald',
  pending: 'amber',
  discrepancy: 'red',
  under_review: 'blue',
};

function FinancialReconciliationInner({ data: propData }: FinancialReconciliationProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getFinancialReconciliationData(),
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
        <Text className="text-red-600">Failed to load reconciliation data</Text>
      </Card>
    );
  }

  const { summary, cashFlow, discrepancies, transactions, reconciliationRate } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics.
          Reconciled/pending/discrepancy amounts are fixed ratios of real
          revenue, not actual processor reconciliation — there is no payment
          processor feed wired up yet. Net Cash (revenue minus refunds) is
          computed from real engine data. */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <div className="flex items-center justify-between">
            <Text>Total Reconciled</Text>
            <SimulatedDataBadge />
          </div>
          <Metric>{formatCurrency(summary?.totalReconciled || 458000)}</Metric>
          <Text className="text-xs text-emerald-600 mt-1">
            {formatPercentage(summary?.reconciledRate || 0.982)} of transactions
          </Text>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <div className="flex items-center justify-between">
            <Text>Pending Review</Text>
            <SimulatedDataBadge />
          </div>
          <Metric>{formatCurrency(summary?.pendingAmount || 12500)}</Metric>
          <Text className="text-xs text-amber-600 mt-1">
            {summary?.pendingCount || 23} transactions
          </Text>
        </Card>
        <Card decoration="top" decorationColor="red">
          <div className="flex items-center justify-between">
            <Text>Discrepancies</Text>
            <SimulatedDataBadge />
          </div>
          <Metric>{formatCurrency(summary?.discrepancyAmount || 3200)}</Metric>
          <Text className="text-xs text-red-600 mt-1">
            {summary?.discrepancyCount || 8} items flagged
          </Text>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>Net Cash Position</Text>
          <Metric>{formatCurrency(summary?.netCash || 125000)}</Metric>
        </Card>
      </Grid>

      {/* Reconciliation Progress.
          Per-category rates are hard-coded demo ratios applied to real
          revenue totals (see getFinancialReconciliationData). */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <div className="flex items-center gap-2">
              <Title>Reconciliation Progress</Title>
              <SimulatedDataBadge />
            </div>
            <Text className="text-gray-500">Current period status</Text>
          </div>
          <Badge color="emerald" size="lg">
            {formatPercentage(reconciliationRate?.overall || 0.94)} Complete
          </Badge>
        </div>
        <div className="space-y-4">
          {(reconciliationRate?.byCategory || generateDemoReconciliationCategories()).map((category: ReconciliationCategory) => (
            <div key={category.name}>
              <div className="flex justify-between mb-1">
                <Text className="font-medium">{category.name}</Text>
                <div className="flex items-center space-x-2">
                  <Text className="text-sm">{formatCurrency(category.reconciled)} / {formatCurrency(category.total)}</Text>
                  <Badge color={category.rate >= 0.95 ? 'emerald' : category.rate >= 0.8 ? 'amber' : 'red'} size="xs">
                    {formatPercentage(category.rate)}
                  </Badge>
                </div>
              </div>
              <ProgressBar
                value={category.rate * 100}
                color={category.rate >= 0.95 ? 'emerald' : category.rate >= 0.8 ? 'amber' : 'red'}
              />
            </div>
          ))}
        </div>
      </Card>

      {/* Cash Flow Chart */}
      <Card>
        <Title>Cash Flow Overview</Title>
        <Text className="text-gray-500 mb-4">Inflows and outflows over time</Text>
        <AreaChart
          className="h-72"
          data={cashFlow || generateDemoCashFlow()}
          index="date"
          categories={['inflow', 'outflow', 'net']}
          colors={['emerald', 'red', 'blue']}
          showAnimation
          curveType="monotone"
          valueFormatter={(value) => formatCurrency(value)}
        />
      </Card>

      {/* Charts Row */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        {/* Transaction Status.
            Distribution is a fixed split of real revenue, not actual
            reconciliation status counts. */}
        <Card>
          <div className="flex items-center justify-between">
            <Title>Transaction Status Distribution</Title>
            <SimulatedDataBadge />
          </div>
          <Text className="text-gray-500 mb-4">Breakdown by reconciliation status</Text>
          <DonutChart
            className="h-64"
            data={summary?.statusDistribution || generateDemoStatusDistribution()}
            category="value"
            index="status"
            colors={['emerald', 'amber', 'red', 'blue']}
            showAnimation
            valueFormatter={(value) => formatCurrency(value)}
          />
        </Card>

        {/* Discrepancy Types.
            Counts come from real return statuses, but amounts are fixed
            shares of a simulated discrepancy total. */}
        <Card>
          <div className="flex items-center justify-between">
            <Title>Discrepancy Analysis</Title>
            <SimulatedDataBadge />
          </div>
          <Text className="text-gray-500 mb-4">Common discrepancy types</Text>
          <BarChart
            className="h-64"
            data={discrepancies?.byType || generateDemoDiscrepancyTypes()}
            index="type"
            categories={['count', 'amount']}
            colors={['red', 'amber']}
            showAnimation
          />
        </Card>
      </Grid>

      {/* Flagged Discrepancies.
          Expected amounts come from real returns; the "actual"/"difference"
          deltas are deterministic jitter, not processor-reported figures. */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <div className="flex items-center gap-2">
              <Title>Flagged Discrepancies</Title>
              <SimulatedDataBadge />
            </div>
            <Text className="text-gray-500">Items requiring attention</Text>
          </div>
          <Badge color="red" size="lg">
            {discrepancies?.items?.length || 5} items
          </Badge>
        </div>
        <div className="space-y-3">
          {(discrepancies?.items || generateDemoDiscrepancies()).map((item: DiscrepancyItem, index: number) => (
            <motion.div
              key={item.id || index}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: index * 0.1 }}
              className="flex items-center justify-between p-4 border rounded-lg dark:border-gray-700 border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/10"
            >
              <div className="flex items-center space-x-4">
                <div className="w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
                  <ExclamationTriangleIcon className="w-5 h-5 text-red-600" />
                </div>
                <div>
                  <Text className="font-medium">{item.description}</Text>
                  <Text className="text-xs text-gray-500">
                    {item.transactionId} • {item.source}
                  </Text>
                </div>
              </div>
              <div className="flex items-center space-x-4">
                <div className="text-right">
                  <Text className="font-medium text-red-600">
                    {item.difference > 0 ? '+' : ''}{formatCurrency(item.difference)}
                  </Text>
                  <Text className="text-xs text-gray-500">
                    Expected: {formatCurrency(item.expected)}
                  </Text>
                </div>
                <Badge color={statusColors[item.status] as TremorColor || 'gray'}>
                  {item.status.replace('_', ' ')}
                </Badge>
              </div>
            </motion.div>
          ))}
        </div>
      </Card>

      {/* Recent Transactions.
          Amounts/dates come from real orders, but TXN ids, sources, types and
          reconciliation statuses are synthesized for display. */}
      <Card>
        <div className="flex items-center justify-between">
          <Title>Recent Transactions</Title>
          <SimulatedDataBadge />
        </div>
        <Text className="text-gray-500 mb-4">Latest reconciled transactions</Text>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b dark:border-gray-700">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Transaction ID</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Type</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Source</th>
                <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Amount</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Status</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Date</th>
              </tr>
            </thead>
            <tbody>
              {(transactions || generateDemoTransactions()).slice(0, 10).map((txn, index: number) => (
                <motion.tr
                  key={txn.id || index}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ delay: index * 0.03 }}
                  className="border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                >
                  <td className="py-2 px-3 text-sm font-mono">{txn.id}</td>
                  <td className="py-2 px-3">
                    <Badge color={txn.type === 'inflow' ? 'emerald' : 'red'} size="xs">
                      {txn.type}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-sm">{txn.source}</td>
                  <td className="py-2 px-3 text-sm text-right font-medium">
                    <span className={txn.type === 'inflow' ? 'text-emerald-600' : 'text-red-600'}>
                      {txn.type === 'inflow' ? '+' : '-'}{formatCurrency(Math.abs(txn.amount))}
                    </span>
                  </td>
                  <td className="py-2 px-3">
                    <Badge color={statusColors[txn.status] as TremorColor || 'gray'} size="xs">
                      {txn.status}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-sm text-gray-500">{txn.date}</td>
                </motion.tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </motion.div>
  );
}

// Demo data generators
function generateDemoReconciliationCategories() {
  return [
    { name: 'Sales Revenue', reconciled: 325000, total: 328000, rate: 0.99 },
    { name: 'Refunds', reconciled: 18500, total: 19200, rate: 0.96 },
    { name: 'Payment Processing', reconciled: 8200, total: 8500, rate: 0.96 },
    { name: 'Subscription Billing', reconciled: 45000, total: 45000, rate: 1.0 },
    { name: 'Vendor Payments', reconciled: 61300, total: 72500, rate: 0.85 },
  ];
}

function generateDemoCashFlow() {
  return [
    { date: 'Dec 15', inflow: 45000, outflow: 28000, net: 17000 },
    { date: 'Dec 16', inflow: 52000, outflow: 31000, net: 21000 },
    { date: 'Dec 17', inflow: 48000, outflow: 35000, net: 13000 },
    { date: 'Dec 18', inflow: 61000, outflow: 29000, net: 32000 },
    { date: 'Dec 19', inflow: 55000, outflow: 42000, net: 13000 },
    { date: 'Dec 20', inflow: 68000, outflow: 38000, net: 30000 },
    { date: 'Dec 21', inflow: 72000, outflow: 35000, net: 37000 },
  ];
}

function generateDemoStatusDistribution() {
  return [
    { status: 'Reconciled', value: 458000 },
    { status: 'Pending', value: 12500 },
    { status: 'Discrepancy', value: 3200 },
    { status: 'Under Review', value: 5800 },
  ];
}

function generateDemoDiscrepancyTypes() {
  return [
    { type: 'Amount Mismatch', count: 12, amount: 1850 },
    { type: 'Missing Transaction', count: 5, amount: 950 },
    { type: 'Duplicate Entry', count: 3, amount: 280 },
    { type: 'Date Discrepancy', count: 2, amount: 120 },
  ];
}

function generateDemoDiscrepancies() {
  return [
    { id: '1', transactionId: 'TXN-2847', description: 'Payment amount mismatch', source: 'Stripe', expected: 125.00, actual: 122.50, difference: -2.50, status: 'under_review' },
    { id: '2', transactionId: 'TXN-2851', description: 'Missing refund record', source: 'PayPal', expected: 89.99, actual: 0, difference: -89.99, status: 'discrepancy' },
    { id: '3', transactionId: 'TXN-2856', description: 'Duplicate charge detected', source: 'Stripe', expected: 45.00, actual: 90.00, difference: 45.00, status: 'pending' },
    { id: '4', transactionId: 'TXN-2862', description: 'Currency conversion variance', source: 'Bank', expected: 1250.00, actual: 1247.85, difference: -2.15, status: 'under_review' },
  ];
}

function generateDemoTransactions(): ReconciliationTransaction[] {
  return [
    { id: 'TXN-2890', type: 'inflow', source: 'Stripe', amount: 1250.00, status: 'reconciled', date: 'Today' },
    { id: 'TXN-2889', type: 'inflow', source: 'PayPal', amount: 89.99, status: 'reconciled', date: 'Today' },
    { id: 'TXN-2888', type: 'outflow', source: 'Vendor', amount: 450.00, status: 'reconciled', date: 'Today' },
    { id: 'TXN-2887', type: 'inflow', source: 'Stripe', amount: 329.00, status: 'pending', date: 'Yesterday' },
    { id: 'TXN-2886', type: 'outflow', source: 'Refund', amount: 75.50, status: 'reconciled', date: 'Yesterday' },
    { id: 'TXN-2885', type: 'inflow', source: 'Bank Transfer', amount: 5000.00, status: 'reconciled', date: 'Yesterday' },
    { id: 'TXN-2884', type: 'outflow', source: 'Payroll', amount: 12500.00, status: 'reconciled', date: '2 days ago' },
    { id: 'TXN-2883', type: 'inflow', source: 'Stripe', amount: 2100.00, status: 'reconciled', date: '2 days ago' },
  ];
}

const FinancialReconciliation = memo(FinancialReconciliationInner);
export default FinancialReconciliation;
