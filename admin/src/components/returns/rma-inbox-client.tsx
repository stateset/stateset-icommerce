'use client';

import { useCallback, useMemo, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { approveReturn, processRefund, receiveReturn, rejectReturn } from '@/app/actions/commerce';
import type { Return } from '@/lib/types';

const STATUS_BADGE_COLOR: Record<Return['status'], React.ComponentProps<typeof Badge>['color']> = {
  requested: 'amber',
  approved: 'blue',
  received: 'indigo',
  inspected: 'purple',
  refunded: 'emerald',
  rejected: 'red',
  closed: 'gray',
};

const REASON_LABELS: Record<Return['reasonCategory'], string> = {
  defective: 'Defective',
  wrong_item: 'Wrong item',
  not_as_described: 'Not as described',
  changed_mind: 'Changed mind',
  other: 'Other',
};

/** Per-row UI state (which row is busy, plus any error message). */
type RowState = { busy: boolean; error?: string };

export interface RmaInboxClientProps {
  initialReturns: Return[];
}

export function RmaInboxClient({ initialReturns }: RmaInboxClientProps) {
  const [returns, setReturns] = useState<Return[]>(initialReturns);
  const [rowStates, setRowStates] = useState<Record<string, RowState>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [statusFilter, setStatusFilter] = useState<'pending' | 'all'>('pending');

  const visible = useMemo(() => {
    if (statusFilter === 'all') return returns;
    // "pending" = anything an operator hasn't fully resolved yet.
    const open: Return['status'][] = ['requested', 'approved', 'received', 'inspected'];
    return returns.filter((r) => open.includes(r.status));
  }, [returns, statusFilter]);

  const replaceReturn = useCallback((next: Return) => {
    setReturns((prev) => prev.map((r) => (r.id === next.id ? next : r)));
  }, []);

  const setRow = useCallback((id: string, state: RowState) => {
    setRowStates((prev) => ({ ...prev, [id]: state }));
  }, []);

  /**
   * Wrap a server action in optimistic-busy UI state. We don't optimistically
   * mutate the row because rejections can fail (validation, permissions);
   * we just disable the buttons until the action settles.
   */
  const runAction = useCallback(
    async (id: string, fn: () => Promise<Return>) => {
      setRow(id, { busy: true });
      try {
        const next = await fn();
        replaceReturn(next);
        setRow(id, { busy: false });
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        setRow(id, { busy: false, error: message });
      }
    },
    [replaceReturn, setRow],
  );

  const onApprove = useCallback(
    (id: string) => runAction(id, () => approveReturn(id)),
    [runAction],
  );

  const onReject = useCallback(
    (id: string) => {
      const reason = window.prompt('Rejection reason?')?.trim();
      if (!reason) return;
      void runAction(id, () => rejectReturn(id, reason));
    },
    [runAction],
  );

  const onReceive = useCallback(
    (r: Return) => {
      // Mark every line item as "opened"; a richer dialog would let the
      // operator set per-item condition. This is the sensible default.
      const items = r.items.map((it) => ({ productId: it.productId, condition: 'opened' }));
      void runAction(r.id, () => receiveReturn(r.id, items));
    },
    [runAction],
  );

  const onRefund = useCallback(
    (r: Return) => {
      const choice = window.prompt(
        'Refund method? (original / store_credit / exchange)',
        r.refundMethod ?? 'original',
      );
      if (!choice) return;
      const method = choice.trim() as Return['refundMethod'];
      if (!['original', 'store_credit', 'exchange'].includes(method as string)) {
        setRow(r.id, { busy: false, error: `Invalid method: ${choice}` });
        return;
      }
      void runAction(r.id, () => processRefund(r.id, method));
    },
    [runAction, setRow],
  );

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
      prev.size === visible.length ? new Set() : new Set(visible.map((r) => r.id)),
    );
  }, [visible]);

  const onBulkApprove = useCallback(async () => {
    const ids = Array.from(selected);
    setSelected(new Set());
    // Sequential rather than parallel: the engine has its own write
    // serialization but the UI feedback is clearer one-at-a-time.
    for (const id of ids) {
      await runAction(id, () => approveReturn(id));
    }
  }, [runAction, selected]);

  const allSelected = visible.length > 0 && selected.size === visible.length;
  const someSelected = selected.size > 0;

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <Badge color="blue">{visible.length} shown</Badge>
            <span className="text-sm text-ds-muted-foreground">{returns.length} total</span>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant={statusFilter === 'pending' ? 'default' : 'outline'}
              size="sm"
              onClick={() => setStatusFilter('pending')}
            >
              Pending only
            </Button>
            <Button
              variant={statusFilter === 'all' ? 'default' : 'outline'}
              size="sm"
              onClick={() => setStatusFilter('all')}
            >
              All
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={!someSelected}
              onClick={onBulkApprove}
              aria-label={`Approve ${selected.size} selected returns`}
            >
              Bulk approve ({selected.size})
            </Button>
          </div>
        </CardHeader>
      </Card>

      <Card>
        <CardContent className="p-0">
          {visible.length === 0 ? (
            <div className="p-8 text-center text-sm text-ds-muted-foreground">
              {statusFilter === 'pending'
                ? 'No pending returns. Switch to "All" to see refunded / rejected / closed records.'
                : 'No returns yet.'}
            </div>
          ) : (
            <table className="w-full text-sm">
              <thead className="border-b border-ds-enterprise-line text-left text-xs uppercase tracking-wide text-ds-muted-foreground">
                <tr>
                  <th className="px-4 py-3 w-10">
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={toggleSelectAll}
                      aria-label="Select all visible returns"
                    />
                  </th>
                  <th className="px-4 py-3">Return</th>
                  <th className="px-4 py-3">Order</th>
                  <th className="px-4 py-3">Reason</th>
                  <th className="px-4 py-3">Items</th>
                  <th className="px-4 py-3">Status</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-ds-enterprise-line">
                {visible.map((r) => {
                  const row = rowStates[r.id] ?? { busy: false };
                  const isSelected = selected.has(r.id);
                  const canApprove = r.status === 'requested';
                  const canReject = r.status === 'requested' || r.status === 'approved';
                  const canReceive = r.status === 'approved';
                  const canRefund = r.status === 'received' || r.status === 'inspected';
                  return (
                    <tr key={r.id} className={isSelected ? 'bg-ds-brand-50' : ''}>
                      <td className="px-4 py-3">
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={() => toggleSelect(r.id)}
                          aria-label={`Select return ${r.id}`}
                        />
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">{r.id}</td>
                      <td className="px-4 py-3 font-mono text-xs">{r.orderId}</td>
                      <td className="px-4 py-3">{REASON_LABELS[r.reasonCategory]}</td>
                      <td className="px-4 py-3">{r.items.length}</td>
                      <td className="px-4 py-3">
                        <Badge color={STATUS_BADGE_COLOR[r.status]}>{r.status}</Badge>
                      </td>
                      <td className="px-4 py-3 text-right space-x-2">
                        {canApprove && (
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={row.busy}
                            onClick={() => onApprove(r.id)}
                          >
                            Approve
                          </Button>
                        )}
                        {canReject && (
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={row.busy}
                            onClick={() => onReject(r.id)}
                          >
                            Reject
                          </Button>
                        )}
                        {canReceive && (
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={row.busy}
                            onClick={() => onReceive(r)}
                          >
                            Mark received
                          </Button>
                        )}
                        {canRefund && (
                          <Button
                            size="sm"
                            variant="primary"
                            disabled={row.busy}
                            onClick={() => onRefund(r)}
                          >
                            Refund
                          </Button>
                        )}
                        {row.error && (
                          <p className="text-xs text-ds-status-fail mt-1 max-w-xs ml-auto">
                            {row.error}
                          </p>
                        )}
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
