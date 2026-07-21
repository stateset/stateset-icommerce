'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getWarehousePageData } from '@/app/actions/operations';
import type { CycleCount, WarehouseLocation } from '@/lib/embedded';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const countStatusVariants: Record<string, DsBadgeVariant> = {
  draft: 'outline',
  in_progress: 'primary',
  completed: 'success',
  cancelled: 'outline',
};

export default function WarehouseClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getWarehousePageData(), {
    refreshInterval: 60000,
  });
  const [warehouseFilter, setWarehouseFilter] = useState('all');
  const [countStatusFilter, setCountStatusFilter] = useState('all');

  const countStatuses = useMemo(() => {
    const unique = new Set((data?.cycleCounts || []).map((count) => count.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const warehouseNames = useMemo(() => {
    const names = new Map<number, string>();
    for (const warehouse of data?.warehouses || []) {
      names.set(warehouse.id, warehouse.name);
    }
    return names;
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="warehouse-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load warehouse data</p>
        </CardContent>
      </Card>
    );
  }

  const locations: WarehouseLocation[] =
    warehouseFilter === 'all'
      ? data.locations
      : data.locations.filter((location) => String(location.warehouseId) === warehouseFilter);

  const cycleCounts: CycleCount[] = (
    countStatusFilter === 'all'
      ? data.cycleCounts
      : data.cycleCounts.filter((count) => count.status === countStatusFilter)
  ).filter((count) => warehouseFilter === 'all' || String(count.warehouseId) === warehouseFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Warehouse</h1>
          <p className="text-sm text-ds-muted-foreground">
            Warehouses, storage locations, and cycle counts (read-only)
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Warehouse</span>
            <select
              aria-label="Filter by warehouse"
              value={warehouseFilter}
              onChange={(event) => setWarehouseFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              <option value="all">All warehouses</option>
              {data.warehouses.map((warehouse) => (
                <option key={warehouse.id} value={String(warehouse.id)}>
                  {warehouse.code}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Count status</span>
            <select
              aria-label="Filter cycle counts by status"
              value={countStatusFilter}
              onChange={(event) => setCountStatusFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              {countStatuses.map((status) => (
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
            Warehouses ({data.warehouses.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Code</th>
                  <th className="py-2 pr-4 font-medium">Name</th>
                  <th className="py-2 pr-4 font-medium">Type</th>
                  <th className="py-2 pr-4 font-medium">Timezone</th>
                  <th className="py-2 font-medium">Active</th>
                </tr>
              </thead>
              <tbody>
                {data.warehouses.map((warehouse) => (
                  <tr key={warehouse.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{warehouse.code}</td>
                    <td className="py-2 pr-4">{warehouse.name}</td>
                    <td className="py-2 pr-4">{warehouse.warehouseType}</td>
                    <td className="py-2 pr-4">{warehouse.timezone || '—'}</td>
                    <td className="py-2">
                      <Badge variant={warehouse.isActive ? 'success' : 'outline'}>
                        {warehouse.isActive ? 'active' : 'inactive'}
                      </Badge>
                    </td>
                  </tr>
                ))}
                {data.warehouses.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-ds-muted-foreground">
                      No warehouses found.
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
            Locations ({locations.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Location</th>
                  <th className="py-2 pr-4 font-medium">Warehouse</th>
                  <th className="py-2 pr-4 font-medium">Type</th>
                  <th className="py-2 pr-4 font-medium">Zone</th>
                  <th className="py-2 pr-4 font-medium">Aisle</th>
                  <th className="py-2 pr-4 font-medium">Rack</th>
                  <th className="py-2 pr-4 font-medium">Bin</th>
                  <th className="py-2 font-medium">Flags</th>
                </tr>
              </thead>
              <tbody>
                {locations.map((location) => (
                  <tr key={location.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{location.code}</td>
                    <td className="py-2 pr-4">
                      {warehouseNames.get(location.warehouseId) || location.warehouseId}
                    </td>
                    <td className="py-2 pr-4">{location.locationType}</td>
                    <td className="py-2 pr-4">{location.zone || '—'}</td>
                    <td className="py-2 pr-4">{location.aisle || '—'}</td>
                    <td className="py-2 pr-4">{location.rack || '—'}</td>
                    <td className="py-2 pr-4">{location.bin || '—'}</td>
                    <td className="py-2">
                      <span className="flex gap-1">
                        {location.isPickable && <Badge variant="primary">pickable</Badge>}
                        {location.isReceivable && <Badge variant="accent">receivable</Badge>}
                        {!location.isActive && <Badge variant="outline">inactive</Badge>}
                      </span>
                    </td>
                  </tr>
                ))}
                {locations.length === 0 && (
                  <tr>
                    <td colSpan={8} className="py-6 text-center text-ds-muted-foreground">
                      No locations match this filter.
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
            Cycle counts ({cycleCounts.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Count</th>
                  <th className="py-2 pr-4 font-medium">Warehouse</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Scheduled</th>
                  <th className="py-2 pr-4 font-medium">Counted by</th>
                  <th className="py-2 text-right font-medium">Lines</th>
                </tr>
              </thead>
              <tbody>
                {cycleCounts.map((count) => (
                  <tr key={count.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{count.id}</td>
                    <td className="py-2 pr-4">
                      {warehouseNames.get(count.warehouseId) || count.warehouseId}
                    </td>
                    <td className="py-2 pr-4">
                      <Badge variant={countStatusVariants[count.status] || 'default'}>
                        {count.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{count.scheduledDate || '—'}</td>
                    <td className="py-2 pr-4">{count.countedBy || '—'}</td>
                    <td className="py-2 text-right font-mono">{count.lines.length}</td>
                  </tr>
                ))}
                {cycleCounts.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-ds-muted-foreground">
                      No cycle counts match this filter.
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
