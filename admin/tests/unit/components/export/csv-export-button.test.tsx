// Component test for the reusable CSV export button. We exercise the
// fetch → serialize → download flow by stubbing the helpers in
// `@/lib/csv/csv` so we don't have to drive the real DOM Blob/anchor
// machinery from jsdom.

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const downloadCsvSpy = vi.fn();
const rowsToCsvSpy = vi.fn((rows: unknown[]) => `csv-of-${rows.length}-rows`);

vi.mock('@/lib/csv/csv', () => ({
  downloadCsv: (filename: string, csv: string) => downloadCsvSpy(filename, csv),
  rowsToCsv: (rows: unknown[], cols: unknown[]) => rowsToCsvSpy(rows, cols),
}));

import { CsvExportButton } from '@/components/export/csv-export-button';

const COLUMNS = [{ header: 'Id', value: (r: { id: string }) => r.id }];

beforeEach(() => {
  downloadCsvSpy.mockClear();
  rowsToCsvSpy.mockClear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('CsvExportButton', () => {
  it('renders the default label and aria attributes', () => {
    render(
      <CsvExportButton
        fetchRows={async () => []}
        columns={COLUMNS}
        filenamePrefix="orders"
      />,
    );
    const btn = screen.getByRole('button', { name: /export export csv/i });
    expect(btn).toBeInTheDocument();
    expect(btn).toHaveTextContent('Export CSV');
  });

  it('uses pre-fetched rows synchronously when `rows` is provided', async () => {
    const fetchRows = vi.fn();
    render(
      <CsvExportButton
        fetchRows={fetchRows}
        columns={COLUMNS}
        filenamePrefix="orders"
        rows={[{ id: 'a' }, { id: 'b' }]}
      />,
    );
    fireEvent.click(screen.getByRole('button'));
    await waitFor(() => expect(downloadCsvSpy).toHaveBeenCalledTimes(1));
    expect(fetchRows).not.toHaveBeenCalled();
    expect(rowsToCsvSpy).toHaveBeenCalledWith([{ id: 'a' }, { id: 'b' }], COLUMNS);
    const [filename, csv] = downloadCsvSpy.mock.calls[0];
    expect(filename).toMatch(/^orders-/);
    expect(filename).toMatch(/\.csv$/);
    expect(csv).toBe('csv-of-2-rows');
  });

  it('calls fetchRows when no pre-fetched rows are provided', async () => {
    const fetchRows = vi.fn().mockResolvedValue([{ id: 'x' }]);
    render(
      <CsvExportButton
        fetchRows={fetchRows}
        columns={COLUMNS}
        filenamePrefix="returns"
      />,
    );
    fireEvent.click(screen.getByRole('button'));
    await waitFor(() => expect(downloadCsvSpy).toHaveBeenCalledTimes(1));
    expect(fetchRows).toHaveBeenCalledTimes(1);
  });

  it('renders an inline error message when the fetch throws', async () => {
    const fetchRows = vi.fn().mockRejectedValue(new Error('no permission'));
    render(
      <CsvExportButton
        fetchRows={fetchRows}
        columns={COLUMNS}
        filenamePrefix="audit"
      />,
    );
    fireEvent.click(screen.getByRole('button'));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('no permission');
    expect(downloadCsvSpy).not.toHaveBeenCalled();
  });

  it('falls back to a generic message when the thrown value is not an Error', async () => {
    const fetchRows = vi.fn().mockRejectedValue('boom');
    render(
      <CsvExportButton
        fetchRows={fetchRows}
        columns={COLUMNS}
        filenamePrefix="audit"
      />,
    );
    fireEvent.click(screen.getByRole('button'));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Export failed');
  });

  it('respects the explicit aria-label override', () => {
    render(
      <CsvExportButton
        fetchRows={async () => []}
        columns={COLUMNS}
        filenamePrefix="x"
        ariaLabel="Download orders.csv"
      />,
    );
    expect(screen.getByRole('button', { name: 'Download orders.csv' })).toBeInTheDocument();
  });
});
