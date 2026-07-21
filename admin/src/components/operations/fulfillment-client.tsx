'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getFulfillmentPageData } from '@/app/actions/operations';
import type { PickTask, Wave } from '@/lib/embedded';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const waveStatusVariants: Record<string, DsBadgeVariant> = {
  created: 'outline',
  released: 'primary',
  picking: 'accent',
  completed: 'success',
  cancelled: 'outline',
};

const pickStatusVariants: Record<string, DsBadgeVariant> = {
  pending: 'outline',
  assigned: 'primary',
  in_progress: 'accent',
  picked: 'success',
  cancelled: 'outline',
};

export default function FulfillmentClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getFulfillmentPageData(), {
    refreshInterval: 60000,
  });
  const [waveStatusFilter, setWaveStatusFilter] = useState('all');
  const [pickStatusFilter, setPickStatusFilter] = useState('all');

  const waveStatuses = useMemo(() => {
    const unique = new Set((data?.waves || []).map((wave) => wave.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const pickStatuses = useMemo(() => {
    const unique = new Set((data?.picks || []).map((pick) => pick.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const waveNumbers = useMemo(() => {
    const numbers = new Map<string, string>();
    for (const wave of data?.waves || []) {
      numbers.set(wave.id, wave.waveNumber);
    }
    return numbers;
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="fulfillment-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load fulfillment data</p>
        </CardContent>
      </Card>
    );
  }

  const waves: Wave[] =
    waveStatusFilter === 'all'
      ? data.waves
      : data.waves.filter((wave) => wave.status === waveStatusFilter);

  const picks: PickTask[] =
    pickStatusFilter === 'all'
      ? data.picks
      : data.picks.filter((pick) => pick.status === pickStatusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Fulfillment</h1>
          <p className="text-sm text-ds-muted-foreground">
            Release waves and pick tasks (read-only)
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Wave status</span>
            <select
              aria-label="Filter waves by status"
              value={waveStatusFilter}
              onChange={(event) => setWaveStatusFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              {waveStatuses.map((status) => (
                <option key={status} value={status}>
                  {status === 'all' ? 'All statuses' : status}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Pick status</span>
            <select
              aria-label="Filter pick tasks by status"
              value={pickStatusFilter}
              onChange={(event) => setPickStatusFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              {pickStatuses.map((status) => (
                <option key={status} value={status}>
                  {status === 'all' ? 'All statuses' : status}
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Waves ({waves.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Wave</th>
                  <th className="py-2 pr-4 font-medium">Warehouse</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 text-right font-medium">Orders</th>
                  <th className="py-2 font-medium">Created</th>
                </tr>
              </thead>
              <tbody>
                {waves.map((wave) => (
                  <tr key={wave.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{wave.waveNumber}</td>
                    <td className="py-2 pr-4">{wave.warehouseId}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={waveStatusVariants[wave.status] || 'default'}>
                        {wave.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4 text-right font-mono">{wave.orderCount}</td>
                    <td className="py-2">{wave.createdAt.slice(0, 10)}</td>
                  </tr>
                ))}
                {waves.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-ds-muted-foreground">
                      No waves match this filter.
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
            Pick tasks ({picks.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Task</th>
                  <th className="py-2 pr-4 font-medium">Wave</th>
                  <th className="py-2 pr-4 font-medium">Order</th>
                  <th className="py-2 pr-4 font-medium">SKU</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Assignee</th>
                  <th className="py-2 pr-4 text-right font-medium">Requested</th>
                  <th className="py-2 text-right font-medium">Picked</th>
                </tr>
              </thead>
              <tbody>
                {picks.map((pick) => (
                  <tr key={pick.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{pick.id}</td>
                    <td className="py-2 pr-4">
                      {pick.waveId ? waveNumbers.get(pick.waveId) || pick.waveId : '—'}
                    </td>
                    <td className="py-2 pr-4 font-mono">{pick.orderId}</td>
                    <td className="py-2 pr-4 font-mono">{pick.sku}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={pickStatusVariants[pick.status] || 'default'}>
                        {pick.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{pick.assignedTo || '—'}</td>
                    <td className="py-2 pr-4 text-right font-mono">{pick.quantityRequested}</td>
                    <td className="py-2 text-right font-mono">{pick.quantityPicked}</td>
                  </tr>
                ))}
                {picks.length === 0 && (
                  <tr>
                    <td colSpan={8} className="py-6 text-center text-ds-muted-foreground">
                      No pick tasks match this filter.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <p className="text-xs text-ds-muted-foreground">
        Pack tasks, cartons, and ship tasks are not yet exposed by the engine binding
        (`commerce.fulfillment` provides waves and pick tasks only).
      </p>
    </div>
  );
}
