'use client';

import { Fragment, useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getRevenueContracts } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const statusBadgeVariants: Record<string, DsBadgeVariant> = {
  draft: 'outline',
  active: 'primary',
  completed: 'success',
  cancelled: 'danger',
};

export default function RevenueClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getRevenueContracts(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const statuses = useMemo(() => {
    const unique = new Set((data || []).map((contract) => contract.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="revenue-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load revenue contracts</p>
        </CardContent>
      </Card>
    );
  }

  const contracts =
    statusFilter === 'all' ? data : data.filter((contract) => contract.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Revenue recognition</h1>
          <p className="text-sm text-ds-muted-foreground">
            ASC 606 contracts with recognized and deferred balances
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter contracts by status"
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

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Contracts ({contracts.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Contract</th>
                  <th className="py-2 pr-4 font-medium">Customer</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Effective</th>
                  <th className="py-2 pr-4 text-right font-medium">Price</th>
                  <th className="py-2 pr-4 text-right font-medium">Recognized</th>
                  <th className="py-2 text-right font-medium">Deferred</th>
                </tr>
              </thead>
              <tbody>
                {contracts.map((contract) => {
                  const expanded = expandedId === contract.id;
                  return (
                    <Fragment key={contract.id}>
                      <tr
                        className="cursor-pointer border-b border-ds-border/50 hover:bg-ds-muted/40"
                        aria-expanded={expanded}
                        onClick={() => setExpandedId(expanded ? null : contract.id)}
                      >
                        <td className="py-2 pr-4 font-mono">{contract.contractNumber}</td>
                        <td className="py-2 pr-4 font-mono">{contract.customerId}</td>
                        <td className="py-2 pr-4">
                          <Badge variant={statusBadgeVariants[contract.status] || 'default'}>
                            {contract.status}
                          </Badge>
                        </td>
                        <td className="py-2 pr-4">{contract.effectiveDate}</td>
                        <td className="py-2 pr-4 text-right font-mono">
                          {formatMoney(contract.transactionPrice, contract.currency)}
                        </td>
                        <td className="py-2 pr-4 text-right font-mono">
                          {formatMoney(contract.totalRecognized, contract.currency)}
                        </td>
                        <td className="py-2 text-right font-mono font-semibold">
                          {formatMoney(contract.deferredBalance, contract.currency)}
                        </td>
                      </tr>
                      {expanded && (
                        <tr className="border-b border-ds-border/50 bg-ds-muted/20">
                          <td colSpan={7} className="px-4 py-3">
                            <p className="mb-2 text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
                              Performance obligations ({contract.obligations.length})
                            </p>
                            <table className="min-w-[32rem] text-xs">
                              <thead>
                                <tr className="text-left text-ds-muted-foreground">
                                  <th className="py-1 pr-4 font-medium">Description</th>
                                  <th className="py-1 pr-4 font-medium">Method</th>
                                  <th className="py-1 pr-4 text-right font-medium">Allocated</th>
                                  <th className="py-1 pr-4 text-right font-medium">Recognized</th>
                                  <th className="py-1 text-right font-medium">Deferred</th>
                                </tr>
                              </thead>
                              <tbody>
                                {contract.obligations.map((obligation) => (
                                  <tr key={obligation.id}>
                                    <td className="py-1 pr-4">{obligation.description}</td>
                                    <td className="py-1 pr-4 capitalize">
                                      {obligation.recognitionMethod.replaceAll('_', ' ')}
                                    </td>
                                    <td className="py-1 pr-4 text-right font-mono">
                                      {formatMoney(obligation.allocatedAmount, contract.currency)}
                                    </td>
                                    <td className="py-1 pr-4 text-right font-mono">
                                      {formatMoney(obligation.recognizedAmount, contract.currency)}
                                    </td>
                                    <td className="py-1 text-right font-mono">
                                      {formatMoney(obligation.deferredAmount, contract.currency)}
                                    </td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
                {contracts.length === 0 && (
                  <tr>
                    <td colSpan={7} className="py-6 text-center text-ds-muted-foreground">
                      No contracts match this filter.
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
