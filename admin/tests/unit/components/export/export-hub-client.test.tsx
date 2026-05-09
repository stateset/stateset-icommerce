/**
 * Component tests for `<ExportHubClient />`.
 *
 * The Export Hub is a static layout that renders three EntityCards (Orders,
 * Customers, Inventory) — each card pairs a description with a
 * <CsvExportButton /> wired to the relevant server action. There's no
 * interactive state inside ExportHubClient itself; the meaningful coverage
 * is "every entity is rendered with its title, description, column count
 * badge, and export button".
 *
 * We mock the server actions and the CsvExportButton so the test focuses
 * on layout + wiring, not the CSV serializer (which is tested separately
 * in `tests/unit/lib/csv/`).
 */

import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  CUSTOMER_CSV_COLUMNS,
  INVENTORY_CSV_COLUMNS,
  ORDER_CSV_COLUMNS,
} from '@/lib/csv/specs';

// Mock the server actions so the client-side render doesn't try to invoke
// "use server" boundaries from jsdom.
vi.mock('@/app/actions/commerce', () => ({
  getOrders: vi.fn().mockResolvedValue([]),
  getCustomers: vi.fn().mockResolvedValue([]),
  getInventory: vi.fn().mockResolvedValue([]),
}));

// Stub the CsvExportButton with a label-passing button so we can verify
// the prop wiring without exercising the download path.
vi.mock('@/components/export/csv-export-button', () => ({
  CsvExportButton: ({
    label,
    filenamePrefix,
  }: {
    label?: string;
    filenamePrefix: string;
  }) => (
    <button data-testid={`csv-${filenamePrefix}`} aria-label={label ?? `Export ${filenamePrefix}`}>
      {label ?? `Export ${filenamePrefix}`}
    </button>
  ),
}));

import { ExportHubClient } from '@/components/export/export-hub-client';

afterEach(() => {
  cleanup();
});

describe('<ExportHubClient />', () => {
  it('renders three entity cards: Orders, Customers, Inventory', () => {
    render(<ExportHubClient />);

    expect(screen.getByRole('heading', { name: 'Orders' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Customers' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Inventory' })).toBeInTheDocument();
  });

  it('shows the column-count badge for each entity', () => {
    render(<ExportHubClient />);

    expect(
      screen.getByText(`${ORDER_CSV_COLUMNS.length} cols`),
    ).toBeInTheDocument();
    expect(
      screen.getByText(`${CUSTOMER_CSV_COLUMNS.length} cols`),
    ).toBeInTheDocument();
    expect(
      screen.getByText(`${INVENTORY_CSV_COLUMNS.length} cols`),
    ).toBeInTheDocument();
  });

  it('wires each entity to its correct CSV export button', () => {
    render(<ExportHubClient />);

    expect(screen.getByTestId('csv-orders')).toHaveAccessibleName(
      'Export orders CSV',
    );
    expect(screen.getByTestId('csv-customers')).toHaveAccessibleName(
      'Export customers CSV',
    );
    expect(screen.getByTestId('csv-inventory')).toHaveAccessibleName(
      'Export inventory CSV',
    );
  });

  it('renders the descriptive copy for each entity', () => {
    render(<ExportHubClient />);

    expect(
      screen.getByText(/Status, totals, item counts, timestamps/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Lifetime spend, order count, tags/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/On-hand, reserved, available, reorder points/),
    ).toBeInTheDocument();
  });
});
