import { getReturns } from '@/app/actions/commerce';
import { RmaInboxClient } from '@/components/returns/rma-inbox-client';

/**
 * RMA Inbox — operational queue for processing pending returns.
 *
 * Closes the audit's #1 missing operational workflow: the existing
 * `returns/page.tsx` shows a dashboard view but didn't expose
 * actionable approve / reject / receive / refund controls.
 *
 * This page is a Server Component that fetches the initial returns list
 * server-side, then hands off to a Client Component for the interactive
 * processing UI (state per row, optimistic updates, bulk actions).
 */
export default async function RmaInboxPage() {
  const returns = await getReturns();

  return (
    <div className="container mx-auto p-6 max-w-6xl">
      <header className="mb-6 flex items-baseline justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">RMA Inbox</h1>
          <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
            Approve, reject, receive, or refund pending returns. Bulk-select
            to act on multiple at once.
          </p>
        </div>
      </header>
      <RmaInboxClient initialReturns={returns} />
    </div>
  );
}
