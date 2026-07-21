'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getBillsPageData } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';
import type { Bill } from '@/lib/embedded';

type DsBadgeVariant = 'default' | 'primary' | 'accent' | 'success' | 'warning' | 'danger' | 'outline';

const statusBadgeVariants: Record<string, DsBadgeVariant> = {
  open: 'primary',
  approved: 'accent',
  paid: 'success',
  overdue: 'danger',
  cancelled: 'outline',
};

const AGING_BUCKETS = [
  { key: 'current', label: 'Current' },
  { key: 'days130', label: '1–30 days' },
  { key: 'days3160', label: '31–60 days' },
  { key: 'days6190', label: '61–90 days' },
  { key: 'daysOver90', label: '90+ days' },
  { key: 'total', label: 'Total due' },
] as const;

export default function BillsClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getBillsPageData(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');

  const statuses = useMemo(() => {
    const unique = new Set((data?.bills || []).map((bill) => bill.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="bills-loading">
            <div className="h-6 w-48 rounded bg-ds-muted" />
            <div className="h-32 rounded bg-ds-muted" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load accounts payable</p>
        </CardContent>
      </Card>
    );
  }

  const bills: Bill[] =
    statusFilter === 'all'
      ? data.bills
      : data.bills.filter((bill) => bill.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Bills</h1>
          <p className="text-sm text-ds-muted-foreground">
            Accounts payable — supplier bills and aging
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter bills by status"
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value)}
            className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm capitalize"
          >
            {statuses.map((status) => (
              <option key={status} value={status}>
                {status === 'all' ? 'All statuses' : status}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
        {AGING_BUCKETS.map((bucket) => (
          <Card key={bucket.key} className={bucket.key === 'total' ? 'border-ds-primary/40' : undefined}>
            <CardContent>
              <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
                {bucket.label}
              </p>
              <p className="mt-1 font-mono text-lg font-semibold">
                {formatMoney(data.aging[bucket.key])}
              </p>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Bills ({bills.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Bill</th>
                  <th className="py-2 pr-4 font-medium">Supplier</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Due date</th>
                  <th className="py-2 pr-4 text-right font-medium">Total</th>
                  <th className="py-2 pr-4 text-right font-medium">Paid</th>
                  <th className="py-2 text-right font-medium">Due</th>
                </tr>
              </thead>
              <tbody>
                {bills.map((bill) => (
                  <tr key={bill.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{bill.billNumber}</td>
                    <td className="py-2 pr-4 font-mono">{bill.supplierId}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={statusBadgeVariants[bill.status] || 'default'}>
                        {bill.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{bill.dueDate}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(bill.totalAmount)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(bill.amountPaid)}</td>
                    <td className="py-2 text-right font-mono">{formatMoney(bill.amountDue)}</td>
                  </tr>
                ))}
                {bills.length === 0 && (
                  <tr>
                    <td colSpan={7} className="py-6 text-center text-ds-muted-foreground">
                      No bills match this filter.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
