'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getEdiPageData } from '@/app/actions/edi';
import type { EdiDocument } from '@/lib/embedded';

type DsBadgeVariant =
  | 'default'
  | 'primary'
  | 'accent'
  | 'success'
  | 'warning'
  | 'danger'
  | 'outline';

const statusBadgeVariants: Record<string, DsBadgeVariant> = {
  pending: 'warning',
  sent: 'primary',
  acknowledged: 'accent',
  processed: 'success',
  error: 'danger',
};

const directionBadgeVariants: Record<string, DsBadgeVariant> = {
  inbound: 'primary',
  outbound: 'accent',
};

// Human labels for the common X12 transaction sets.
const documentTypeLabels: Record<string, string> = {
  '850': 'Purchase Order',
  '855': 'PO Acknowledgment',
  '856': 'Advance Ship Notice',
  '810': 'Invoice',
  '860': 'PO Change',
  '997': 'Functional Ack',
};

const SUMMARY_STATUSES = ['pending', 'sent', 'acknowledged', 'processed', 'error'] as const;

function formatTimestamp(value: string): string {
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) {
    return value;
  }
  return new Date(parsed).toLocaleString();
}

export default function EdiClient() {
  const { data, isLoading, error } = useEmbeddedData(() => getEdiPageData(), {
    refreshInterval: 60000,
  });
  const [statusFilter, setStatusFilter] = useState('all');

  const statuses = useMemo(() => {
    const unique = new Set((data?.documents || []).map((doc) => doc.status));
    return ['all', ...Array.from(unique).sort()];
  }, [data]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="edi-loading">
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
          <p className="text-sm text-ds-status-fail">Failed to load EDI documents</p>
        </CardContent>
      </Card>
    );
  }

  const statusCounts = new Map(data.summary.byStatus.map((entry) => [entry.key, entry.count]));
  const documents: EdiDocument[] =
    statusFilter === 'all'
      ? data.documents
      : data.documents.filter((doc) => doc.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">EDI Documents</h1>
          <p className="text-sm text-ds-muted-foreground">
            Trading-partner documents — purchase orders, acknowledgments, ASNs, and invoices
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Status</span>
          <select
            aria-label="Filter EDI documents by status"
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

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
        <Card className="border-ds-primary/40">
          <CardContent>
            <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">Total</p>
            <p className="mt-1 font-mono text-lg font-semibold">{data.summary.total}</p>
          </CardContent>
        </Card>
        {SUMMARY_STATUSES.map((status) => (
          <Card key={status}>
            <CardContent>
              <p className="text-xs uppercase tracking-ds-kicker text-ds-muted-foreground">
                {status}
              </p>
              <p className="mt-1 font-mono text-lg font-semibold">
                {statusCounts.get(status) || 0}
              </p>
            </CardContent>
          </Card>
        ))}
      </div>

      {data.summary.byType.length > 0 && (
        <Card>
          <CardContent>
            <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
              By Document Type
            </h2>
            <div className="flex flex-wrap gap-4">
              {data.summary.byType.map((entry) => (
                <div
                  key={entry.key}
                  className="rounded-md border border-ds-border px-3 py-2 text-sm"
                  data-testid={`edi-type-${entry.key}`}
                >
                  <span className="font-mono font-semibold">{entry.key}</span>
                  <span className="ml-2 text-ds-muted-foreground">
                    {documentTypeLabels[entry.key] || 'Document'}
                  </span>
                  <span className="ml-2 font-mono">{entry.count}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Documents ({documents.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Type</th>
                  <th className="py-2 pr-4 font-medium">Direction</th>
                  <th className="py-2 pr-4 font-medium">Partner</th>
                  <th className="py-2 pr-4 font-medium">Reference</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 font-medium">Received / sent at</th>
                </tr>
              </thead>
              <tbody>
                {documents.map((doc) => (
                  <tr key={doc.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4">
                      <span className="font-mono font-semibold">{doc.documentType}</span>
                      <span className="ml-2 text-ds-muted-foreground">
                        {documentTypeLabels[doc.documentType] || ''}
                      </span>
                    </td>
                    <td className="py-2 pr-4">
                      <Badge variant={directionBadgeVariants[doc.direction] || 'default'}>
                        {doc.direction}
                      </Badge>
                    </td>
                    <td className="py-2 pr-4 font-mono">{doc.partner || '—'}</td>
                    <td className="py-2 pr-4 font-mono">{doc.reference || '—'}</td>
                    <td className="py-2 pr-4">
                      <Badge variant={statusBadgeVariants[doc.status] || 'default'}>
                        {doc.status}
                      </Badge>
                      {doc.status === 'error' && doc.errorMessage && (
                        <p className="mt-1 text-xs text-ds-status-fail">{doc.errorMessage}</p>
                      )}
                    </td>
                    <td className="py-2">{formatTimestamp(doc.createdAt)}</td>
                  </tr>
                ))}
                {documents.length === 0 && (
                  <tr>
                    <td colSpan={6} className="py-6 text-center text-ds-muted-foreground">
                      {data.documents.length === 0
                        ? 'No EDI documents yet — documents appear here as they are exchanged with trading partners.'
                        : 'No EDI documents match this filter.'}
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
