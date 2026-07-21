import { ExportHubClient } from '@/components/export/export-hub-client';

/**
 * Export Hub
 *
 * Single page that lists every exportable entity (orders, customers,
 * inventory) and offers a one-click CSV export per entity. Closes the
 * audit's "no reporting/export" gap with a focused dedicated surface
 * rather than scattering export buttons across dashboard pages.
 *
 * Each card uses the canonical column spec from `lib/csv/specs.ts`,
 * so the CSV schema is stable across releases.
 */
export default function ExportHubPage() {
  return (
    <div className="container mx-auto p-6 max-w-6xl">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold tracking-tight">Export Hub</h1>
        <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
          One-click CSV export for orders, customers, and inventory. The column schemas are stable —
          automations can rely on consistent headers across releases.
        </p>
      </header>
      <ExportHubClient />
    </div>
  );
}
