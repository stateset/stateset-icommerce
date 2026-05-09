'use client';

import { useCallback, useMemo, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { cancelOrder, updateOrderStatus } from '@/app/actions/commerce';
import { ordersToCsv, downloadCsv } from '@/lib/orders/csv';
import type { Order } from '@/lib/types';

const STATUS_BADGE_COLOR: Record<Order['status'], React.ComponentProps<typeof Badge>['color']> = {
  pending: 'amber',
  confirmed: 'blue',
  processing: 'indigo',
  shipped: 'purple',
  delivered: 'emerald',
  cancelled: 'red',
};

const STATUS_FILTER_OPTIONS: Array<Order['status'] | 'all'> = [
  'all',
  'pending',
  'confirmed',
  'processing',
  'shipped',
  'delivered',
  'cancelled',
];

export interface BulkOrdersClientProps {
  initialOrders: Order[];
}

export function BulkOrdersClient({ initialOrders }: BulkOrdersClientProps) {
  const [orders, setOrders] = useState<Order[]>(initialOrders);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<Order['status'] | 'all'>('all');
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);

  const visible = useMemo(() => {
    if (filter === 'all') return orders;
    return orders.filter((o) => o.status === filter);
  }, [orders, filter]);

  const replaceOrder = useCallback((next: Order) => {
    setOrders((prev) => prev.map((o) => (o.id === next.id ? next : o)));
  }, []);

  const toggleSelect = useCallback((id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleSelectAll = useCallback(() => {
    setSelected((prev) =>
      prev.size === visible.length ? new Set() : new Set(visible.map((o) => o.id)),
    );
  }, [visible]);

  const onBulkCancel = useCallback(async () => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    const reason = window.prompt(
      `Cancel ${ids.length} order${ids.length === 1 ? '' : 's'}? Reason:`,
      'bulk operator action',
    );
    if (!reason) return;
    setBusy(true);
    setErrors([]);
    const failures: string[] = [];
    for (const id of ids) {
      try {
        const next = await cancelOrder(id, reason);
        replaceOrder(next);
      } catch (err) {
        failures.push(`${id}: ${err instanceof Error ? err.message : 'unknown'}`);
      }
    }
    setSelected(new Set());
    setErrors(failures);
    setBusy(false);
  }, [selected, replaceOrder]);

  const onBulkAdvance = useCallback(
    async (target: Order['status']) => {
      const ids = Array.from(selected);
      if (ids.length === 0) return;
      setBusy(true);
      setErrors([]);
      const failures: string[] = [];
      for (const id of ids) {
        try {
          const next = await updateOrderStatus(id, target);
          replaceOrder(next);
        } catch (err) {
          failures.push(`${id}: ${err instanceof Error ? err.message : 'unknown'}`);
        }
      }
      setSelected(new Set());
      setErrors(failures);
      setBusy(false);
    },
    [selected, replaceOrder],
  );

  const onExport = useCallback(() => {
    // If anything is selected, export selection. Otherwise export the
    // current visible (filtered) view.
    const subset = selected.size > 0 ? orders.filter((o) => selected.has(o.id)) : visible;
    const csv = ordersToCsv(subset);
    const ts = new Date().toISOString().replace(/[:.]/g, '-');
    downloadCsv(`orders-${ts}.csv`, csv);
  }, [orders, selected, visible]);

  const allVisibleSelected = visible.length > 0 && selected.size === visible.length;

  // What bulk actions make sense given the current selection?
  // We're conservative: only enable an action when *every* selected row is
  // in a state that allows it, otherwise the operator gets a partial-failure
  // surprise via the per-row error list.
  const selectedStatuses = useMemo(() => {
    const statuses = new Set<Order['status']>();
    for (const o of orders) {
      if (selected.has(o.id)) statuses.add(o.status);
    }
    return statuses;
  }, [orders, selected]);

  const canBulkCancel =
    selected.size > 0 &&
    Array.from(selectedStatuses).every((s) => s !== 'cancelled' && s !== 'delivered');
  const canBulkConfirm =
    selected.size > 0 && Array.from(selectedStatuses).every((s) => s === 'pending');
  const canBulkProcess =
    selected.size > 0 && Array.from(selectedStatuses).every((s) => s === 'confirmed');

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <Badge color="blue">{visible.length} shown</Badge>
            <span className="text-sm text-gray-500">{orders.length} total</span>
            {selected.size > 0 && (
              <Badge color="indigo">{selected.size} selected</Badge>
            )}
          </div>
          <div className="flex items-center gap-2">
            {STATUS_FILTER_OPTIONS.map((opt) => (
              <Button
                key={opt}
                variant={filter === opt ? 'default' : 'outline'}
                size="sm"
                onClick={() => setFilter(opt)}
              >
                {opt}
              </Button>
            ))}
          </div>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2 border-t border-gray-100 dark:border-gray-800 pt-3">
          <Button
            variant="outline"
            size="sm"
            disabled={!canBulkConfirm || busy}
            onClick={() => onBulkAdvance('confirmed')}
          >
            Confirm ({selected.size})
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={!canBulkProcess || busy}
            onClick={() => onBulkAdvance('processing')}
          >
            Move to processing ({selected.size})
          </Button>
          <Button
            variant="destructive"
            size="sm"
            disabled={!canBulkCancel || busy}
            onClick={onBulkCancel}
          >
            Cancel ({selected.size})
          </Button>
          <Button variant="outline" size="sm" onClick={onExport} disabled={busy}>
            Export CSV ({selected.size > 0 ? selected.size : visible.length})
          </Button>
        </CardContent>
        {errors.length > 0 && (
          <CardContent className="border-t border-red-100 bg-red-50 dark:bg-red-900/20">
            <p className="text-sm font-medium text-red-700 dark:text-red-300 mb-1">
              {errors.length} failure{errors.length === 1 ? '' : 's'}:
            </p>
            <ul className="text-xs font-mono text-red-700 dark:text-red-300 space-y-0.5">
              {errors.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          </CardContent>
        )}
      </Card>

      <Card>
        <CardContent className="p-0">
          {visible.length === 0 ? (
            <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">
              No orders match the current filter.
            </div>
          ) : (
            <table className="w-full text-sm">
              <thead className="border-b border-gray-100 dark:border-gray-800 text-left text-xs uppercase tracking-wide text-gray-500">
                <tr>
                  <th className="px-4 py-3 w-10">
                    <input
                      type="checkbox"
                      checked={allVisibleSelected}
                      onChange={toggleSelectAll}
                      aria-label="Select all visible orders"
                    />
                  </th>
                  <th className="px-4 py-3">Order</th>
                  <th className="px-4 py-3">Customer</th>
                  <th className="px-4 py-3">Items</th>
                  <th className="px-4 py-3">Total</th>
                  <th className="px-4 py-3">Status</th>
                  <th className="px-4 py-3">Created</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
                {visible.map((o) => {
                  const isSelected = selected.has(o.id);
                  return (
                    <tr
                      key={o.id}
                      className={isSelected ? 'bg-indigo-50/40 dark:bg-indigo-900/10' : ''}
                    >
                      <td className="px-4 py-3">
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={() => toggleSelect(o.id)}
                          aria-label={`Select order ${o.id}`}
                        />
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">{o.id}</td>
                      <td className="px-4 py-3 font-mono text-xs">{o.customerId}</td>
                      <td className="px-4 py-3">{o.items.length}</td>
                      <td className="px-4 py-3 tabular-nums">
                        {o.currency} {o.totalAmount.toFixed(2)}
                      </td>
                      <td className="px-4 py-3">
                        <Badge color={STATUS_BADGE_COLOR[o.status]}>{o.status}</Badge>
                      </td>
                      <td className="px-4 py-3 text-xs text-gray-500">
                        {o.createdAt.slice(0, 10)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
