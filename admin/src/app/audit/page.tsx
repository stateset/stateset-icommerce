'use client';

import dynamic from 'next/dynamic';

// Client-only because EventSource and rolling-buffer state are inherently
// browser-side; SSR adds nothing for a live event feed.
const AuditLogClient = dynamic(
  () => import('@/components/operations/audit-log-client').then((m) => m.AuditLogClient),
  {
    ssr: false,
    loading: () => (
      <div className="flex items-center justify-center min-h-[200px]">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500" />
      </div>
    ),
  },
);

/**
 * Audit Log
 *
 * Live, append-only view of every domain event the engine emits — order
 * lifecycle, inventory adjustments, returns, payments, agent decisions,
 * policy denials. Backed by the engine's existing
 * `/api/v1/events/stream` Server-Sent Events feed (no extra backend
 * surface needed).
 */
export default function AuditLogPage() {
  return (
    <div className="container mx-auto p-6 max-w-6xl">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">Audit Log</h1>
        <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
          Live stream of domain events. Filter by event-type prefix (e.g. <code>order.*</code>,{' '}
          <code>inventory.*</code>). Pause to inspect or export the current buffer to CSV.
        </p>
      </header>
      <AuditLogClient />
    </div>
  );
}
