'use client';

import { useCallback, useState } from 'react';

import { Button } from '@/components/ui/button';
import { downloadCsv, rowsToCsv, type CsvColumn } from '@/lib/csv/csv';

interface CsvExportButtonProps<T> {
  /** Async function that returns the rows to export when clicked. */
  fetchRows: () => Promise<T[]>;
  /** Canonical column spec for this entity (see `lib/csv/specs.ts`). */
  columns: CsvColumn<T>[];
  /** Filename prefix; a timestamp is appended automatically. */
  filenamePrefix: string;
  /** Visible button label. */
  label?: string;
  /** ARIA label override; defaults to `Export ${label} as CSV`. */
  ariaLabel?: string;
  /**
   * Optional pre-fetched rows. When provided, no `fetchRows` call happens
   * on click — the export uses these synchronously. Useful when the parent
   * page already has the data.
   */
  rows?: T[];
}

/**
 * Reusable client-side CSV export button.
 *
 * Encapsulates the fetch → serialize → download flow so any admin page
 * can add CSV export with a single import. Disables the button while
 * the fetch is in flight; surfaces fetch errors inline below the button.
 *
 * Pairs with `lib/csv/csv.ts` (helpers) and `lib/csv/specs.ts` (canonical
 * column specs per entity type).
 */
export function CsvExportButton<T>({
  fetchRows,
  columns,
  filenamePrefix,
  label = 'Export CSV',
  ariaLabel,
  rows,
}: CsvExportButtonProps<T>) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onClick = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      const data = rows ?? (await fetchRows());
      const csv = rowsToCsv(data, columns);
      const ts = new Date().toISOString().replace(/[:.]/g, '-');
      downloadCsv(`${filenamePrefix}-${ts}.csv`, csv);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed');
    } finally {
      setBusy(false);
    }
  }, [columns, fetchRows, filenamePrefix, rows]);

  return (
    <div className="inline-flex flex-col items-stretch gap-1">
      <Button
        variant="outline"
        size="sm"
        onClick={onClick}
        disabled={busy}
        aria-label={ariaLabel ?? `Export ${label.toLowerCase()}`}
      >
        {busy ? 'Exporting…' : label}
      </Button>
      {error && (
        <p className="text-xs text-red-600 max-w-xs" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
