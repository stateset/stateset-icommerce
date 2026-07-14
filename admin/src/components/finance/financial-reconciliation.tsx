'use client';

import { memo } from 'react';
import { AreaChart, BarChart, DonutChart, ProgressBar } from '@tremor/react';
import { Badge, Card, CardContent, CardHeader, CardTitle, CardDescription } from '@stateset/design';
import { ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getFinancialReconciliationData } from '@/app/actions/commerce';
import { SimulatedDataBadge } from '@/components/shared/simulated-data-badge';
import { formatCurrency, formatPercentage } from '@/lib/utils';
import type { FinancialReconciliationData, ReconciliationCategory, DiscrepancyItem, ReconciliationTransaction } from '@/lib/types/dashboard-data';

interface FinancialReconciliationProps {
  data?: FinancialReconciliationData;
}

const statusBadgeVariants: Record<string, 'success' | 'warning' | 'danger' | 'primary'> = {
  reconciled: 'success',
  pending: 'warning',
  discrepancy: 'danger',
  under_review: 'primary',
};

function FinancialReconciliationInner({ data: propData }: FinancialReconciliationProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getFinancialReconciliationData(),
    { initialData: propData, refreshInterval: 60000 }
  );

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
      <Card className="border-ds-status-fail/25">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load reconciliation data</p>
        </CardContent>
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
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <Card className="border-t-2 border-t-ds-status-ok">
          <CardContent>
            <div className="flex items-center justify-between">
              <p className="text-sm text-ds-muted-foreground">Total Reconciled</p>
              <SimulatedDataBadge />
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground">{formatCurrency(summary?.totalReconciled || 458000)}</p>
            <p className="text-xs text-ds-status-ok mt-1">
              {formatPercentage(summary?.reconciledRate || 0.982)} of transactions
            </p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-status-warn">
          <CardContent>
            <div className="flex items-center justify-between">
              <p className="text-sm text-ds-muted-foreground">Pending Review</p>
              <SimulatedDataBadge />
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground">{formatCurrency(summary?.pendingAmount || 12500)}</p>
            <p className="text-xs text-ds-status-warn mt-1">
              {summary?.pendingCount || 23} transactions
            </p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-status-fail">
          <CardContent>
            <div className="flex items-center justify-between">
              <p className="text-sm text-ds-muted-foreground">Discrepancies</p>
              <SimulatedDataBadge />
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground">{formatCurrency(summary?.discrepancyAmount || 3200)}</p>
            <p className="text-xs text-ds-status-fail mt-1">
              {summary?.discrepancyCount || 8} items flagged
            </p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-status-run">
          <CardContent>
            <p className="text-sm text-ds-muted-foreground">Net Cash Position</p>
            <p className="ds-instrument-number text-3xl text-ds-foreground">{formatCurrency(summary?.netCash || 125000)}</p>
          </CardContent>
        </Card>
      </div>

      {/* Reconciliation Progress.
          Per-category rates are hard-coded demo ratios applied to real
          revenue totals (see getFinancialReconciliationData). */}
      <Card>
        <CardContent>
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Reconciliation Progress</h3>
                <SimulatedDataBadge />
              </div>
              <p className="text-sm text-ds-muted-foreground">Current period status</p>
            </div>
            <Badge variant="success">
              {formatPercentage(reconciliationRate?.overall || 0.94)} Complete
            </Badge>
          </div>
          <div className="space-y-4">
            {(reconciliationRate?.byCategory || generateDemoReconciliationCategories()).map((category: ReconciliationCategory) => (
              <div key={category.name}>
                <div className="flex justify-between mb-1">
                  <p className="text-sm font-medium text-ds-foreground">{category.name}</p>
                  <div className="flex items-center space-x-2">
                    <p className="text-sm text-ds-muted-foreground">{formatCurrency(category.reconciled)} / {formatCurrency(category.total)}</p>
                    <Badge variant={category.rate >= 0.95 ? 'success' : category.rate >= 0.8 ? 'warning' : 'danger'}>
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
        </CardContent>
      </Card>

      {/* Cash Flow Chart */}
      <Card>
        <CardHeader>
          <CardTitle>Cash Flow Overview</CardTitle>
          <CardDescription>Inflows and outflows over time</CardDescription>
        </CardHeader>
        <CardContent>
          <AreaChart
            className="h-72"
            data={cashFlow || generateDemoCashFlow()}
            index="date"
            categories={['inflow', 'outflow', 'net']}
            colors={['emerald', 'red', 'indigo']}
            showAnimation
            curveType="monotone"
            valueFormatter={(value) => formatCurrency(value)}
          />
        </CardContent>
      </Card>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Transaction Status.
            Distribution is a fixed split of real revenue, not actual
            reconciliation status counts. */}
        <Card>
          <CardContent>
            <div className="flex items-center justify-between">
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Transaction Status Distribution</h3>
              <SimulatedDataBadge />
            </div>
            <p className="text-sm text-ds-muted-foreground mb-4">Breakdown by reconciliation status</p>
            <DonutChart
              className="h-64"
              data={summary?.statusDistribution || generateDemoStatusDistribution()}
              category="value"
              index="status"
              colors={['emerald', 'amber', 'red', 'indigo']}
              showAnimation
              valueFormatter={(value) => formatCurrency(value)}
            />
          </CardContent>
        </Card>

        {/* Discrepancy Types.
            Counts come from real return statuses, but amounts are fixed
            shares of a simulated discrepancy total. */}
        <Card>
          <CardContent>
            <div className="flex items-center justify-between">
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Discrepancy Analysis</h3>
              <SimulatedDataBadge />
            </div>
            <p className="text-sm text-ds-muted-foreground mb-4">Common discrepancy types</p>
            <BarChart
              className="h-64"
              data={discrepancies?.byType || generateDemoDiscrepancyTypes()}
              index="type"
              categories={['count', 'amount']}
              colors={['red', 'amber']}
              showAnimation
            />
          </CardContent>
        </Card>
      </div>

      {/* Flagged Discrepancies.
          Expected amounts come from real returns; the "actual"/"difference"
          deltas are deterministic jitter, not processor-reported figures. */}
      <Card>
        <CardContent>
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Flagged Discrepancies</h3>
                <SimulatedDataBadge />
              </div>
              <p className="text-sm text-ds-muted-foreground">Items requiring attention</p>
            </div>
            <Badge variant="danger">
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
                className="flex items-center justify-between p-4 border rounded-lg border-ds-status-fail/25 bg-ds-status-fail/10"
              >
                <div className="flex items-center space-x-4">
                  <div className="w-10 h-10 rounded-full bg-ds-status-fail/15 flex items-center justify-center">
                    <ExclamationTriangleIcon className="w-5 h-5 text-ds-status-fail" />
                  </div>
                  <div>
                    <p className="text-sm font-medium text-ds-foreground">{item.description}</p>
                    <p className="text-xs text-ds-muted-foreground">
                      {item.transactionId} • {item.source}
                    </p>
                  </div>
                </div>
                <div className="flex items-center space-x-4">
                  <div className="text-right">
                    <p className="text-sm font-medium text-ds-status-fail">
                      {item.difference > 0 ? '+' : ''}{formatCurrency(item.difference)}
                    </p>
                    <p className="text-xs text-ds-muted-foreground">
                      Expected: {formatCurrency(item.expected)}
                    </p>
                  </div>
                  <Badge variant={statusBadgeVariants[item.status] || 'default'}>
                    {item.status.replace('_', ' ')}
                  </Badge>
                </div>
              </motion.div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Recent Transactions.
          Amounts/dates come from real orders, but TXN ids, sources, types and
          reconciliation statuses are synthesized for display. */}
      <Card>
        <CardContent>
          <div className="flex items-center justify-between">
            <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Recent Transactions</h3>
            <SimulatedDataBadge />
          </div>
          <p className="text-sm text-ds-muted-foreground mb-4">Latest reconciled transactions</p>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-ds-enterprise-line">
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">Transaction ID</th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">Type</th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">Source</th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-ds-muted-foreground">Amount</th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">Status</th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">Date</th>
                </tr>
              </thead>
              <tbody>
                {(transactions || generateDemoTransactions()).slice(0, 10).map((txn, index: number) => (
                  <motion.tr
                    key={txn.id || index}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: index * 0.03 }}
                    className="border-b border-ds-enterprise-line hover:bg-ds-muted"
                  >
                    <td className="py-2 px-3 text-sm font-mono">{txn.id}</td>
                    <td className="py-2 px-3">
                      <Badge variant={txn.type === 'inflow' ? 'success' : 'danger'}>
                        {txn.type}
                      </Badge>
                    </td>
                    <td className="py-2 px-3 text-sm">{txn.source}</td>
                    <td className="py-2 px-3 text-sm text-right font-medium">
                      <span className={txn.type === 'inflow' ? 'text-ds-status-ok' : 'text-ds-status-fail'}>
                        {txn.type === 'inflow' ? '+' : '-'}{formatCurrency(Math.abs(txn.amount))}
                      </span>
                    </td>
                    <td className="py-2 px-3">
                      <Badge variant={statusBadgeVariants[txn.status] || 'default'}>
                        {txn.status}
                      </Badge>
                    </td>
                    <td className="py-2 px-3 text-sm text-ds-muted-foreground">{txn.date}</td>
                  </motion.tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
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
