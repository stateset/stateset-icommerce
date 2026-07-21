import { getOrders } from '@/app/actions/commerce';
import { BulkOrdersClient } from '@/components/orders/bulk-orders-client';

/**
 * Bulk Orders — operational queue for multi-row order actions.
 *
 * Closes the "no bulk operations" UX gap from the audit. Operators can
 * select rows and bulk-cancel pending/confirmed orders, bulk-advance
 * confirmed → processing, or bulk-export the visible filter to CSV.
 *
 * Server Component fetches the initial list; Client Component owns
 * selection state and action dispatch.
 */
export default async function BulkOrdersPage() {
  const orders = await getOrders({ limit: 200 });

  return (
    <div className="container mx-auto p-6 max-w-6xl">
      <header className="mb-6 flex items-baseline justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Bulk Orders</h1>
          <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
            Multi-select orders to cancel, advance status, or export to CSV. Filter by status before
            selecting to avoid acting on the wrong rows.
          </p>
        </div>
      </header>
      <BulkOrdersClient initialOrders={orders} />
    </div>
  );
}
