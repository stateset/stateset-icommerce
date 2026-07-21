'use client';

import { Card, CardContent } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getReceivablesPageData } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';

const AGING_BUCKETS = [
  { key: 'current', label: 'Current' },
  { key: 'days130', label: '1–30 days' },
  { key: 'days3160', label: '31–60 days' },
  { key: 'days6190', label: '61–90 days' },
  { key: 'daysOver90', label: '90+ days' },
  { key: 'total', label: 'Total due' },
] as const;

export default function ReceivablesClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getReceivablesPageData(), {
    refreshInterval: 60000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="receivables-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load accounts receivable</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Receivables</h1>
          <p className="text-sm text-ds-muted-foreground">
            Accounts receivable — customer aging and collections
          </p>
        </div>
        {data.dso !== null && (
          <Card className="border-ds-primary/40">
            <CardContent>
              <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
                DSO ({data.dsoWindowDays}d)
              </p>
              <p className="mt-1 font-mono text-lg font-semibold" data-testid="dso-stat">
                {data.dso.toFixed(1)} days
              </p>
            </CardContent>
          </Card>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
        {AGING_BUCKETS.map((bucket) => (
          <Card
            key={bucket.key}
            className={bucket.key === 'total' ? 'border-ds-primary/40' : undefined}
          >
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
            Aging by customer ({data.customers.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Customer</th>
                  <th className="py-2 pr-4 text-right font-medium">Current</th>
                  <th className="py-2 pr-4 text-right font-medium">1–30</th>
                  <th className="py-2 pr-4 text-right font-medium">31–60</th>
                  <th className="py-2 pr-4 text-right font-medium">61–90</th>
                  <th className="py-2 pr-4 text-right font-medium">90+</th>
                  <th className="py-2 text-right font-medium">Total</th>
                </tr>
              </thead>
              <tbody>
                {data.customers.map((row) => (
                  <tr key={row.customerId} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{row.customerId}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(row.current)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(row.days130)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(row.days3160)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(row.days6190)}</td>
                    <td className="py-2 pr-4 text-right font-mono">
                      {formatMoney(row.daysOver90)}
                    </td>
                    <td className="py-2 text-right font-mono font-semibold">
                      {formatMoney(row.total)}
                    </td>
                  </tr>
                ))}
                {data.customers.length === 0 && (
                  <tr>
                    <td colSpan={7} className="py-6 text-center text-ds-muted-foreground">
                      No open receivables.
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
