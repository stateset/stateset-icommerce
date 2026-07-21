'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getTraceabilityPageData } from '@/app/actions/operations';
import type { Lot, Receipt, SerialNumber } from '@/lib/embedded';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const lotStatusVariants: Record<string, DsBadgeVariant> = {
  active: 'success',
  quarantined: 'warning',
  expired: 'danger',
  consumed: 'outline',
};

const serialStatusVariants: Record<string, DsBadgeVariant> = {
  available: 'success',
  allocated: 'primary',
  sold: 'accent',
  quarantined: 'warning',
};

const receiptStatusVariants: Record<string, DsBadgeVariant> = {
  pending: 'outline',
  receiving: 'primary',
  completed: 'success',
  cancelled: 'outline',
};

const NEAR_EXPIRY_DAYS = 30;
const DAY_MS = 24 * 60 * 60 * 1000;

/** `expired` | `near` | `ok` for a lot's expiration date (display only). */
function expiryState(expirationDate: string | undefined, now: number): 'expired' | 'near' | 'ok' {
  if (!expirationDate) {
    return 'ok';
  }
  const parsed = Date.parse(expirationDate);
  if (Number.isNaN(parsed)) {
    return 'ok';
  }
  if (parsed < now) {
    return 'expired';
  }
  return parsed - now <= NEAR_EXPIRY_DAYS * DAY_MS ? 'near' : 'ok';
}

export default function TraceabilityClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getTraceabilityPageData(), {
    refreshInterval: 60000,
  });
  const [lotStatusFilter, setLotStatusFilter] = useState('all');
  const [serialStatusFilter, setSerialStatusFilter] = useState('all');

  const lotStatuses = useMemo(() => {
    const unique = new Set((data?.lots || []).map((lot) => lot.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const serialStatuses = useMemo(() => {
    const unique = new Set((data?.serials || []).map((serial) => serial.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  const lotNumbers = useMemo(() => {
    const numbers = new Map<string, string>();
    for (const lot of data?.lots || []) {
      numbers.set(lot.id, lot.lotNumber);
    }
    return numbers;
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="traceability-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load traceability data</p>
        </CardContent>
      </Card>
    );
  }

  const now = Date.now();

  const lots: Lot[] =
    lotStatusFilter === 'all'
      ? data.lots
      : data.lots.filter((lot) => lot.status === lotStatusFilter);

  const serials: SerialNumber[] =
    serialStatusFilter === 'all'
      ? data.serials
      : data.serials.filter((serial) => serial.status === serialStatusFilter);

  const receipts: Receipt[] = data.receipts;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Traceability</h1>
          <p className="text-sm text-ds-muted-foreground">
            Lot genealogy, serial numbers, and inbound receipts (read-only)
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Lot status</span>
            <select
              aria-label="Filter lots by status"
              value={lotStatusFilter}
              onChange={(event) => setLotStatusFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              {lotStatuses.map((status) => (
                <option key={status} value={status}>
                  {status === 'all' ? 'All statuses' : status}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-2 text-sm">
            <span className="text-ds-muted-foreground">Serial status</span>
            <select
              aria-label="Filter serials by status"
              value={serialStatusFilter}
              onChange={(event) => setSerialStatusFilter(event.target.value)}
              className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
            >
              {serialStatuses.map((status) => (
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
            Lots ({lots.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Lot</th>
                  <th className="py-2 pr-4 font-medium">SKU</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Produced on</th>
                  <th className="py-2 pr-4 font-medium">Expires</th>
                  <th className="py-2 pr-4 text-right font-medium">Produced</th>
                  <th className="py-2 pr-4 text-right font-medium">Available</th>
                  <th className="py-2 text-right font-medium">Reserved</th>
                </tr>
              </thead>
              <tbody>
                {lots.map((lot) => {
                  const state = expiryState(lot.expirationDate, now);
                  return (
                    <tr key={lot.id} className="border-b border-ds-border/50">
                      <td className="py-2 pr-4 font-mono">{lot.lotNumber}</td>
                      <td className="py-2 pr-4 font-mono">{lot.sku}</td>
                      <td className="py-2 pr-4">
                        <Badge variant={lotStatusVariants[lot.status] || 'default'}>
                          {lot.status}
                        </Badge>
                      </td>
                      <td className="py-2 pr-4">{lot.productionDate || '—'}</td>
                      <td className="py-2 pr-4">
                        <span
                          data-expiry={state}
                          className={
                            state === 'expired'
                              ? 'font-medium text-ds-status-fail'
                              : state === 'near'
                                ? 'font-medium text-ds-status-warn'
                                : undefined
                          }
                        >
                          {lot.expirationDate || '—'}
                          {state === 'near' && ' (soon)'}
                          {state === 'expired' && ' (expired)'}
                        </span>
                      </td>
                      <td className="py-2 pr-4 text-right font-mono">{lot.quantityProduced}</td>
                      <td className="py-2 pr-4 text-right font-mono">{lot.quantityAvailable}</td>
                      <td className="py-2 text-right font-mono">{lot.quantityReserved}</td>
                    </tr>
                  );
                })}
                {lots.length === 0 && (
                  <tr>
                    <td colSpan={8} className="py-6 text-center text-ds-muted-foreground">
                      No lots match this filter.
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
            Serial numbers ({serials.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Serial</th>
                  <th className="py-2 pr-4 font-medium">SKU</th>
                  <th className="py-2 pr-4 font-medium">Lot</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Owner</th>
                  <th className="py-2 font-medium">Location</th>
                </tr>
              </thead>
              <tbody>
                {serials.map((serial) => (
                  <tr key={serial.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{serial.serial}</td>
                    <td className="py-2 pr-4 font-mono">{serial.sku}</td>
                    <td className="py-2 pr-4">
                      {serial.lotId ? lotNumbers.get(serial.lotId) || serial.lotId : '—'}
                    </td>
                    <td className="py-2 pr-4">
                      <Badge variant={serialStatusVariants[serial.status] || 'default'}>
                        {serial.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{serial.ownerId || '—'}</td>
                    <td className="py-2">{serial.locationId ?? '—'}</td>
                  </tr>
                ))}
                {serials.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-ds-muted-foreground">
                      No serial numbers match this filter.
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
            Receipts ({receipts.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Receipt</th>
                  <th className="py-2 pr-4 font-medium">Type</th>
                  <th className="py-2 pr-4 font-medium">Warehouse</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 pr-4 font-medium">Carrier</th>
                  <th className="py-2 font-medium">Tracking</th>
                </tr>
              </thead>
              <tbody>
                {receipts.map((receipt) => (
                  <tr key={receipt.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{receipt.receiptNumber}</td>
                    <td className="py-2 pr-4">{receipt.receiptType}</td>
                    <td className="py-2 pr-4">{receipt.warehouseId}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={receiptStatusVariants[receipt.status] || 'default'}>
                        {receipt.status}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4">{receipt.carrier || '—'}</td>
                    <td className="py-2 font-mono">{receipt.trackingNumber || '—'}</td>
                  </tr>
                ))}
                {receipts.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-ds-muted-foreground">
                      No receipts found.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <p className="text-xs text-ds-muted-foreground">
        Put-away task detail is not yet exposed by the engine binding (`commerce.receiving` provides
        receipt headers only).
      </p>
    </div>
  );
}
