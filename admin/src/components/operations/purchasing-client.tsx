'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getPurchasingPageData } from '@/app/actions/operations';
import { formatMoney } from '@/lib/finance/format';
import type { PurchaseOrder } from '@/lib/embedded';

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
  submitted: 'primary',
  approved: 'accent',
  sent: 'primary',
  received: 'success',
  cancelled: 'outline',
};

export default function PurchasingClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getPurchasingPageData(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');

  const statuses = useMemo(() => {
    const unique = new Set((data?.purchaseOrders || []).map((po) => po.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const supplierNames = useMemo(() => {
    const names = new Map<string, string>();
    for (const supplier of data?.suppliers || []) {
      names.set(supplier.id, supplier.name);
    }
    return names;
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="purchasing-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load purchasing</p>
        </CardContent>
      </Card>
    );
  }

  const purchaseOrders: PurchaseOrder[] =
    statusFilter === 'all'
      ? data.purchaseOrders
      : data.purchaseOrders.filter((po) => po.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Purchasing</h1>
          <p className="text-sm text-ds-muted-foreground">
            Purchase orders and suppliers (read-only)
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter purchase orders by status"
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
            Purchase orders ({purchaseOrders.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">PO</th>
                  <th className="py-2 pr-4 font-medium">Supplier</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Created</th>
                  <th className="py-2 pr-4 font-medium">Updated</th>
                  <th className="py-2 pr-4 text-right font-medium">Subtotal</th>
                  <th className="py-2 text-right font-medium">Total</th>
                </tr>
              </thead>
              <tbody>
                {purchaseOrders.map((po) => (
                  <tr key={po.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{po.poNumber}</td>
                    <td className="py-2 pr-4">
                      {supplierNames.get(po.supplierId) || po.supplierId}
                    </td>
                    <td className="py-2 pr-4">
                      <Badge variant={statusBadgeVariants[po.status] || 'default'}>
                        {po.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{po.createdAt.slice(0, 10)}</td>
                    <td className="py-2 pr-4">{po.updatedAt.slice(0, 10)}</td>
                    <td className="py-2 pr-4 text-right font-mono">{formatMoney(po.subtotal)}</td>
                    <td className="py-2 text-right font-mono">{formatMoney(po.total)}</td>
                  </tr>
                ))}
                {purchaseOrders.length === 0 && (
                  <tr>
                    <td colSpan={7} className="py-6 text-center text-ds-muted-foreground">
                      No purchase orders match this filter.
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
            Suppliers ({data.suppliers.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Supplier</th>
                  <th className="py-2 pr-4 font-medium">Code</th>
                  <th className="py-2 pr-4 font-medium">Email</th>
                  <th className="py-2 pr-4 font-medium">Phone</th>
                  <th className="py-2 font-medium">Active</th>
                </tr>
              </thead>
              <tbody>
                {data.suppliers.map((supplier) => (
                  <tr key={supplier.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4">{supplier.name}</td>
                    <td className="py-2 pr-4 font-mono">{supplier.supplierCode || '—'}</td>
                    <td className="py-2 pr-4">{supplier.email || '—'}</td>
                    <td className="py-2 pr-4">{supplier.phone || '—'}</td>
                    <td className="py-2">
                      <Badge variant={supplier.isActive ? 'success' : 'outline'}>
                        {supplier.isActive ? 'active' : 'inactive'}
                      </Badge>
                    </td>
                  </tr>
                ))}
                {data.suppliers.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-ds-muted-foreground">
                      No suppliers found.
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
