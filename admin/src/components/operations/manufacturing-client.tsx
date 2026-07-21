'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getManufacturingPageData } from '@/app/actions/operations';
import type { WorkOrder } from '@/lib/embedded';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const workOrderStatusVariants: Record<string, DsBadgeVariant> = {
  draft: 'outline',
  released: 'primary',
  in_progress: 'accent',
  completed: 'success',
  cancelled: 'outline',
};

const severityVariants: Record<string, DsBadgeVariant> = {
  minor: 'default',
  major: 'warning',
  critical: 'danger',
};

function countBy<T>(items: T[], key: (item: T) => string): { key: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const item of items) {
    const k = key(item);
    counts.set(k, (counts.get(k) || 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([k, count]) => ({ key: k, count }))
    .sort((left, right) => left.key.localeCompare(right.key));
}

export default function ManufacturingClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getManufacturingPageData(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');

  const statuses = useMemo(() => {
    const unique = new Set((data?.workOrders || []).map((order) => order.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const inspectionCounts = useMemo(
    () => countBy(data?.inspections || [], (inspection) => inspection.status),
    [data],
  );

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="manufacturing-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load manufacturing data</p>
        </CardContent>
      </Card>
    );
  }

  const workOrders: WorkOrder[] =
    statusFilter === 'all'
      ? data.workOrders
      : data.workOrders.filter((order) => order.status === statusFilter);

  const openNcrs = data.ncrs.filter((ncr) => ncr.status !== 'closed');

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Manufacturing</h1>
          <p className="text-sm text-ds-muted-foreground">
            Work orders and quality inspections (read-only)
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter work orders by status"
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value)}
            className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
          >
            {statuses.map((status) => (
              <option key={status} value={status}>
                {status === 'all' ? 'All statuses' : status}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
        <Card>
          <CardContent>
            <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
              Work orders
            </p>
            <p className="mt-1 font-mono text-lg font-semibold">{data.workOrders.length}</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent>
            <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
              Inspections
            </p>
            <p className="mt-1 font-mono text-lg font-semibold">{data.inspections.length}</p>
          </CardContent>
        </Card>
        <Card className={openNcrs.length > 0 ? 'border-ds-status-fail/30' : undefined}>
          <CardContent>
            <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
              Open NCRs
            </p>
            <p className="mt-1 font-mono text-lg font-semibold">{openNcrs.length}</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Work orders ({workOrders.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Work order</th>
                  <th className="py-2 pr-4 font-medium">Product</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Priority</th>
                  <th className="py-2 pr-4 font-medium">Created</th>
                  <th className="py-2 pr-4 text-right font-medium">To build</th>
                  <th className="py-2 text-right font-medium">Completed</th>
                </tr>
              </thead>
              <tbody>
                {workOrders.map((order) => (
                  <tr key={order.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{order.workOrderNumber}</td>
                    <td className="py-2 pr-4 font-mono">{order.productId}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={workOrderStatusVariants[order.status] || 'default'}>
                        {order.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{order.priority}</td>
                    <td className="py-2 pr-4">{order.createdAt.slice(0, 10)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{order.quantityToBuild}</td>
                    <td className="py-2 text-right font-mono">{order.quantityCompleted}</td>
                  </tr>
                ))}
                {workOrders.length === 0 && (
                  <tr>
                    <td colSpan={7} className="py-6 text-center text-ds-muted-foreground">
                      No work orders match this filter.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Inspections by status
          </h2>
          <div className="flex flex-wrap gap-2">
            {inspectionCounts.map((entry) => (
              <span
                key={entry.key}
                className="rounded-md border border-ds-border px-3 py-1.5 text-sm"
              >
                {entry.key}: <span className="font-mono font-semibold">{entry.count}</span>
              </span>
            ))}
            {inspectionCounts.length === 0 && (
              <p className="text-sm text-ds-muted-foreground">No inspections recorded.</p>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Non-conformance reports ({data.ncrs.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">NCR</th>
                  <th className="py-2 pr-4 font-medium">SKU</th>
                  <th className="py-2 pr-4 font-medium">Source</th>
                  <th className="py-2 pr-4 font-medium">Severity</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 text-right font-medium">Qty affected</th>
                </tr>
              </thead>
              <tbody>
                {data.ncrs.map((ncr) => (
                  <tr key={ncr.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{ncr.ncrNumber}</td>
                    <td className="py-2 pr-4 font-mono">{ncr.sku}</td>
                    <td className="py-2 pr-4">{ncr.source}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={severityVariants[ncr.severity] || 'default'}>
                        {ncr.severity}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{ncr.status}</td>
                    <td className="py-2 text-right font-mono">{ncr.quantityAffected}</td>
                  </tr>
                ))}
                {data.ncrs.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-ds-muted-foreground">
                      No non-conformance reports.
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
