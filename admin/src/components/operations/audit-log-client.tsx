'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader } from '@/components/ui/card';

/** One line of the audit feed. */
export interface AuditEvent {
  id: string;
  receivedAt: string;
  type: string;
  data: unknown;
}

/** Hard cap on the rolling buffer. Older events drop off the bottom. */
const MAX_BUFFER = 500;

/**
 * Pick a Badge color for an event type. The mapping is conservative —
 * unknown prefixes fall back to gray. Exhaustive matching is intentionally
 * avoided so new domain events render gracefully.
 */
function badgeColorFor(type: string): React.ComponentProps<typeof Badge>['color'] {
  if (type.startsWith('order')) return 'blue';
  if (type.startsWith('inventory')) return 'amber';
  if (type.startsWith('return')) return 'rose';
  if (type.startsWith('payment') || type.startsWith('x402')) return 'emerald';
  if (type.startsWith('cart')) return 'cyan';
  if (type.startsWith('subscription')) return 'indigo';
  if (type.startsWith('agent') || type.startsWith('policy')) return 'purple';
  return 'gray';
}

export function eventMatchesFilter(type: string, filter: string): boolean {
  if (!filter) return true;
  // Match exact, or `prefix.*` wildcard, or substring fallback.
  const pat = filter.trim();
  if (!pat) return true;
  if (pat.endsWith('.*')) {
    const prefix = pat.slice(0, -2);
    return type === prefix || type.startsWith(`${prefix}.`) || type.startsWith(`${prefix}_`);
  }
  if (pat.includes('*')) {
    // Escape every regex metacharacter (including backslash), then re-enable
    // `*` as a glob wildcard. Escaping only `.` left other metacharacters
    // (`\`, `(`, `[`, `+`, …) live, so a filter could inject a regex / ReDoS
    // (CodeQL js/incomplete-sanitization).
    const escaped = pat.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(`^${escaped.replace(/\\\*/g, '.*')}$`);
    return re.test(type);
  }
  return type === pat || type.includes(pat);
}

function toCsvCell(value: unknown): string {
  if (value === null || value === undefined) return '';
  const s = typeof value === 'string' ? value : JSON.stringify(value);
  // Wrap in quotes; escape inner quotes by doubling.
  return `"${s.replace(/"/g, '""')}"`;
}

export function bufferToCsv(events: AuditEvent[]): string {
  const header = ['received_at', 'type', 'data'].join(',');
  const rows = events.map((e) =>
    [toCsvCell(e.receivedAt), toCsvCell(e.type), toCsvCell(e.data)].join(','),
  );
  return [header, ...rows].join('\n');
}

function downloadCsv(filename: string, csv: string) {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export function AuditLogClient() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [filter, setFilter] = useState('');
  const [paused, setPaused] = useState(false);
  const [connection, setConnection] = useState<'connecting' | 'open' | 'closed' | 'error'>(
    'connecting',
  );
  const sourceRef = useRef<EventSource | null>(null);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Open the SSE connection once on mount; close on unmount. The engine's
  // event stream is at `/api/v1/events/stream` on the StateSet HTTP service.
  // We rely on Next.js to proxy or env var to point at the right host.
  useEffect(() => {
    const apiBase = process.env.NEXT_PUBLIC_STATESET_API_URL || '';
    const url = `${apiBase}/api/v1/events/stream`;
    const source = new EventSource(url, { withCredentials: true });
    sourceRef.current = source;

    source.onopen = () => setConnection('open');
    source.onerror = () => setConnection('error');

    source.onmessage = (e) => {
      if (pausedRef.current) return;
      let parsed: { type?: string; [k: string]: unknown } = {};
      try {
        parsed = typeof e.data === 'string' ? JSON.parse(e.data) : {};
      } catch {
        parsed = { type: 'unknown', raw: e.data };
      }
      const next: AuditEvent = {
        id: e.lastEventId || `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        receivedAt: new Date().toISOString(),
        type: typeof parsed.type === 'string' ? parsed.type : 'unknown',
        data: parsed,
      };
      setEvents((prev) => [next, ...prev].slice(0, MAX_BUFFER));
    };

    return () => {
      source.close();
      sourceRef.current = null;
      setConnection('closed');
    };
  }, []);

  const filtered = useMemo(
    () => events.filter((e) => eventMatchesFilter(e.type, filter)),
    [events, filter],
  );

  const onExport = useCallback(() => {
    const csv = bufferToCsv(filtered);
    const ts = new Date().toISOString().replace(/[:.]/g, '-');
    downloadCsv(`audit-log-${ts}.csv`, csv);
  }, [filtered]);

  const onClear = useCallback(() => setEvents([]), []);

  const connectionBadge = useMemo(() => {
    switch (connection) {
      case 'open':
        return { color: 'emerald' as const, label: 'Connected' };
      case 'connecting':
        return { color: 'amber' as const, label: 'Connecting…' };
      case 'error':
        return { color: 'red' as const, label: 'Connection error' };
      case 'closed':
        return { color: 'gray' as const, label: 'Closed' };
    }
  }, [connection]);

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <Badge color={connectionBadge.color}>{connectionBadge.label}</Badge>
            <span className="text-sm text-ds-muted-foreground">
              {filtered.length} of {events.length} events shown
            </span>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPaused((p) => !p)}
              aria-label={paused ? 'Resume stream' : 'Pause stream'}
            >
              {paused ? 'Resume' : 'Pause'}
            </Button>
            <Button variant="outline" size="sm" onClick={onClear}>
              Clear
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={onExport}
              disabled={filtered.length === 0}
            >
              Export CSV
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <input
            type="text"
            placeholder="Filter by event type (e.g. order.* or inventory_adjusted)"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="w-full px-3 py-2 text-sm rounded-md border border-ds-enterprise-line bg-ds-card text-ds-foreground"
            aria-label="Event type filter"
          />
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-0">
          {filtered.length === 0 ? (
            <div className="p-8 text-center text-sm text-ds-muted-foreground">
              {paused
                ? 'Stream paused. Resume to receive new events.'
                : connection === 'open'
                  ? 'Waiting for events…'
                  : connection === 'error'
                    ? 'Cannot connect to the event stream.'
                    : 'Connecting to the event stream…'}
            </div>
          ) : (
            <ul role="list" className="divide-y divide-ds-enterprise-line">
              {filtered.map((e) => (
                <li key={e.id} className="px-4 py-3 flex items-start gap-4 text-sm">
                  <time
                    dateTime={e.receivedAt}
                    className="font-mono text-xs text-ds-muted-foreground shrink-0 w-32"
                  >
                    {e.receivedAt.slice(11, 23)}
                  </time>
                  <Badge color={badgeColorFor(e.type)} className="shrink-0">
                    {e.type}
                  </Badge>
                  <pre className="font-mono text-xs text-ds-foreground truncate flex-1">
                    {JSON.stringify(e.data)}
                  </pre>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
