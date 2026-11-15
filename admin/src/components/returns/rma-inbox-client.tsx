'use client';

import { useCallback, useMemo, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import {
  addReturnTracking,
  approveReturn,
  cancelReturn,
  completeReturn,
  markReturnReceived,
  rejectReturn,
} from '@/app/actions/commerce';
import {
  RETURN_LIFECYCLE,
  canApplyReturnAction,
  isTerminal,
  type ReturnAction,
  type ReturnReason,
  type ReturnStatus,
} from '@/lib/engine-lifecycles';
import type { Return } from '@/lib/types';

/** One colour per engine return status (`ReturnStatus`). */
const STATUS_BADGE_COLOR: Record<ReturnStatus, React.ComponentProps<typeof Badge>['color']> = {
  requested: 'amber',
  approved: 'blue',
  in_transit: 'cyan',
  received: 'indigo',
  inspecting: 'purple',
  completed: 'emerald',
  rejected: 'red',
  cancelled: 'gray',
};

const STATUS_LABELS: Record<ReturnStatus, string> = {
  requested: 'requested',
  approved: 'approved',
  in_transit: 'in transit',
  received: 'received',
  inspecting: 'inspecting',
  completed: 'completed',
  rejected: 'rejected',
  cancelled: 'cancelled',
};

/** One label per engine return reason code (`ReturnReason`). */
const REASON_LABELS: Record<ReturnReason, string> = {
  defective: 'Defective',
  wrong_item: 'Wrong item',
  not_as_described: 'Not as described',
  changed_mind: 'Changed mind',
  better_price_found: 'Better price found',
  no_longer_needed: 'No longer needed',
  damaged: 'Damaged',
  other: 'Other',
};

/** Button label per binding action, in the order the row renders them. */
const ACTION_LABELS: [ReturnAction, string][] = [
  ['approve', 'Approve'],
  ['addTracking', 'Add tracking'],
  ['markReceived', 'Mark received'],
  ['complete', 'Complete'],
  ['reject', 'Reject'],
  ['cancel', 'Cancel'],
];

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
    // "pending" = every return the engine still lets an operator move:
    // requested, approved, in transit, received and inspecting.
    return returns.filter((r) => !isTerminal(RETURN_LIFECYCLE, r.status));
  }, [returns, statusFilter]);

  const replaceReturn = useCallback((next: Return) => {
    // The engine's reply may be a header without the line items; keep the
    // detail we already have for anything it leaves out.
    setReturns((prev) => prev.map((r) => (r.id === next.id ? { ...r, ...next } : r)));
  }, []);

  const setRow = useCallback((id: string, state: RowState) => {
    setRowStates((prev) => ({ ...prev, [id]: state }));
  }, []);

  /**
   * Wrap a server action in busy-row UI state. We don't optimistically
   * mutate the row because the engine can still refuse a move (validation,
   * permissions, a concurrent change); we disable the buttons until it settles.
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

  const onAction = useCallback(
    (r: Return, action: ReturnAction) => {
      switch (action) {
        case 'approve':
          void runAction(r.id, () => approveReturn(r.id));
          return;
        case 'reject': {
          const reason = window.prompt('Rejection reason?')?.trim();
          if (!reason) return;
          void runAction(r.id, () => rejectReturn(r.id, reason));
          return;
        }
        case 'cancel':
          if (!window.confirm(`Cancel return ${r.id}?`)) return;
          void runAction(r.id, () => cancelReturn(r.id));
          return;
        case 'addTracking': {
          const trackingNumber = window
            .prompt('Return shipment tracking number?', r.trackingNumber ?? '')
            ?.trim();
          if (!trackingNumber) return;
          void runAction(r.id, () => addReturnTracking(r.id, trackingNumber));
          return;
        }
        case 'markReceived':
          void runAction(r.id, () => markReturnReceived(r.id));
          return;
        case 'complete':
          void runAction(r.id, () => completeReturn(r.id));
          return;
      }
    },
    [runAction],
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

  // Bulk approve only when the engine would accept it for *every* selected
  // row (approval is legal from `requested` only); otherwise the operator
  // gets a partial-failure surprise.
  const canBulkApprove =
    selected.size > 0 &&
    returns.every((r) => !selected.has(r.id) || canApplyReturnAction(r.status, 'approve'));

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
              disabled={!canBulkApprove}
              onClick={onBulkApprove}
              aria-label={`Approve ${selected.size} selected returns`}
              title={
                selected.size > 0 && !canBulkApprove
                  ? 'Only requested returns can be approved'
                  : undefined
              }
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
                ? 'No pending returns. Switch to "All" to see completed / rejected / cancelled records.'
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
                  // Only the moves the engine's return lifecycle accepts
                  // from this row's current state.
                  const actions = ACTION_LABELS.filter(([action]) =>
                    canApplyReturnAction(r.status, action),
                  );
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
                      <td className="px-4 py-3">
                        {REASON_LABELS[r.reasonCategory] ?? r.reasonCategory}
                      </td>
                      <td className="px-4 py-3">{r.items.length}</td>
                      <td className="px-4 py-3">
                        <Badge color={STATUS_BADGE_COLOR[r.status] ?? 'gray'}>
                          {STATUS_LABELS[r.status] ?? r.status}
                        </Badge>
                        {r.trackingNumber && (
                          <span className="ml-2 font-mono text-xs text-ds-muted-foreground">
                            {r.trackingNumber}
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-right space-x-2">
                        {actions.map(([action, label]) => (
                          <Button
                            key={action}
                            size="sm"
                            variant={action === 'complete' ? 'primary' : 'outline'}
                            disabled={row.busy}
                            onClick={() => onAction(r, action)}
                          >
                            {label}
                          </Button>
                        ))}
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
