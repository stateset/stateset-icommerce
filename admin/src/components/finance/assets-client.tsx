'use client';

import { Fragment, useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getFixedAssets, getAssetDepreciationSchedule } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';
import type { DepreciationSchedule } from '@/lib/embedded';

type DsBadgeVariant = 'default' | 'primary' | 'accent' | 'success' | 'warning' | 'danger' | 'outline';

const statusBadgeVariants: Record<string, DsBadgeVariant> = {
  draft: 'outline',
  in_service: 'success',
  fully_depreciated: 'accent',
  disposed: 'warning',
  written_off: 'danger',
};

type ScheduleState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'loaded'; schedule: DepreciationSchedule | null };

export default function AssetsClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getFixedAssets(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [schedules, setSchedules] = useState<Record<string, ScheduleState>>({});

  const statuses = useMemo(() => {
    const unique = new Set((data || []).map((asset) => asset.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const toggleExpanded = (assetId: string) => {
    const next = expandedId === assetId ? null : assetId;
    setExpandedId(next);
    if (next && !schedules[next]) {
      setSchedules((current) => ({ ...current, [next]: { status: 'loading' } }));
      getAssetDepreciationSchedule(next)
        .then((schedule) => {
          setSchedules((current) => ({ ...current, [next]: { status: 'loaded', schedule } }));
        })
        .catch(() => {
          setSchedules((current) => ({ ...current, [next]: { status: 'error' } }));
        });
    }
  };

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="assets-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load fixed assets</p>
        </CardContent>
      </Card>
    );
  }

  const assets =
    statusFilter === 'all' ? data : data.filter((asset) => asset.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Fixed assets</h1>
          <p className="text-sm text-ds-muted-foreground">
            Asset register with book values and depreciation schedules
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter assets by status"
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value)}
            className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm capitalize"
          >
            {statuses.map((status) => (
              <option key={status} value={status}>
                {status === 'all' ? 'All statuses' : status.replaceAll('_', ' ')}
              </option>
            ))}
          </select>
        </label>
      </div>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Assets ({assets.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Asset</th>
                  <th className="py-2 pr-4 font-medium">Name</th>
                  <th className="py-2 pr-4 font-medium">Category</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Acquired</th>
                  <th className="py-2 pr-4 text-right font-medium">Cost</th>
                  <th className="py-2 pr-4 text-right font-medium">Accum. depr.</th>
                  <th className="py-2 text-right font-medium">Book value</th>
                </tr>
              </thead>
              <tbody>
                {assets.map((asset) => {
                  const expanded = expandedId === asset.id;
                  const scheduleState = schedules[asset.id];
                  return (
                    <Fragment key={asset.id}>
                      <tr
                        className="cursor-pointer border-b border-ds-border/50 hover:bg-ds-muted/40"
                        aria-expanded={expanded}
                        onClick={() => toggleExpanded(asset.id)}
                      >
                        <td className="py-2 pr-4 font-mono">{asset.assetNumber}</td>
                        <td className="py-2 pr-4">{asset.name}</td>
                        <td className="py-2 pr-4 capitalize">{asset.category}</td>
                        <td className="py-2 pr-4">
                          <Badge variant={statusBadgeVariants[asset.status] || 'default'}>
                            {asset.status.replaceAll('_', ' ')}
                          </Badge>
                        </td>
                        <td className="py-2 pr-4">{asset.acquisitionDate}</td>
                        <td className="py-2 pr-4 text-right font-mono">
                          {formatMoney(asset.acquisitionCost, asset.currency)}
                        </td>
                        <td className="py-2 pr-4 text-right font-mono">
                          {formatMoney(asset.accumulatedDepreciation, asset.currency)}
                        </td>
                        <td className="py-2 text-right font-mono font-semibold">
                          {formatMoney(asset.bookValue, asset.currency)}
                        </td>
                      </tr>
                      {expanded && (
                        <tr className="border-b border-ds-border/50 bg-ds-muted/20">
                          <td colSpan={8} className="px-4 py-3">
                            {(!scheduleState || scheduleState.status === 'loading') && (
                              <p className="text-sm text-ds-muted-foreground">
                                Loading depreciation schedule…
                              </p>
                            )}
                            {scheduleState?.status === 'error' && (
                              <p className="text-sm text-ds-status-fail">
                                Failed to load depreciation schedule
                              </p>
                            )}
                            {scheduleState?.status === 'loaded' && !scheduleState.schedule && (
                              <p className="text-sm text-ds-muted-foreground">
                                No depreciation schedule generated for this asset.
                              </p>
                            )}
                            {scheduleState?.status === 'loaded' && scheduleState.schedule && (
                              <div className="space-y-2">
                                <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
                                  Depreciation schedule ({scheduleState.schedule.method.replaceAll('_', ' ')})
                                  — total {formatMoney(scheduleState.schedule.totalDepreciation, asset.currency)}
                                </p>
                                <table className="min-w-[32rem] text-xs">
                                  <thead>
                                    <tr className="text-left text-ds-muted-foreground">
                                      <th className="py-1 pr-4 font-medium">Period</th>
                                      <th className="py-1 pr-4 text-right font-medium">Amount</th>
                                      <th className="py-1 pr-4 text-right font-medium">Accumulated</th>
                                      <th className="py-1 pr-4 text-right font-medium">Book value</th>
                                      <th className="py-1 font-medium">Status</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {scheduleState.schedule.entries.map((entry) => (
                                      <tr key={entry.period}>
                                        <td className="py-1 pr-4 font-mono">{entry.period}</td>
                                        <td className="py-1 pr-4 text-right font-mono">
                                          {formatMoney(entry.amount, asset.currency)}
                                        </td>
                                        <td className="py-1 pr-4 text-right font-mono">
                                          {formatMoney(entry.accumulated, asset.currency)}
                                        </td>
                                        <td className="py-1 pr-4 text-right font-mono">
                                          {formatMoney(entry.bookValue, asset.currency)}
                                        </td>
                                        <td className="py-1 capitalize">{entry.status}</td>
                                      </tr>
                                    ))}
                                  </tbody>
                                </table>
                              </div>
                            )}
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
                {assets.length === 0 && (
                  <tr>
                    <td colSpan={8} className="py-6 text-center text-ds-muted-foreground">
                      No assets match this filter.
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
