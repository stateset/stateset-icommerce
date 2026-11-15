// Component tests for the bulk-orders client.
//
// Heart of these tests: the cross-status enable/disable rules, taken from
// the engine's order lifecycle (`ORDER_LIFECYCLE`). Bulk Confirm requires
// every selected row to be `pending`; Bulk Move-to-processing requires every
// selected row to be `confirmed`; Bulk Cancel requires every selected row to
// be pending, confirmed or processing (the engine refuses to cancel shipped,
// delivered or refunded orders). Mixing rows of different statuses must
// disable each action that doesn't apply uniformly.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

const cancelSpy = vi.fn();
const updateSpy = vi.fn();
vi.mock('@/app/actions/commerce', () => ({
  cancelOrder: (id: string, reason?: string) => cancelSpy(id, reason),
  updateOrderStatus: (id: string, status: string) => updateSpy(id, status),
}));

// Mock the DOM-only download path so jsdom doesn't try to invoke it.
const downloadCsvSpy = vi.fn();
vi.mock('@/lib/orders/csv', async () => {
  const actual = await vi.importActual<typeof import('@/lib/orders/csv')>(
    '@/lib/orders/csv',
  );
  return { ...actual, downloadCsv: (n: string, c: string) => downloadCsvSpy(n, c) };
});

import { BulkOrdersClient } from '@/components/orders/bulk-orders-client';
import type { Order } from '@/lib/types';

function fakeOrder(id: string, status: Order['status']): Order {
  return {
    id,
    customerId: `CUST-${id}`,
    status,
    items: [
      { productId: 'p1', sku: 'A', name: 'A', quantity: 1, unitPrice: 10, totalPrice: 10 },
    ],
    totalAmount: 10,
    currency: 'USD',
    createdAt: '2026-05-07T12:00:00.000Z',
    updatedAt: '2026-05-07T12:00:00.000Z',
  };
}

describe('BulkOrdersClient · empty + filter', () => {
  beforeEach(() => {
    cancelSpy.mockReset();
    updateSpy.mockReset();
    downloadCsvSpy.mockReset();
  });

  it('renders all orders when filter is "all"', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'cancelled')]}
      />,
    );
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(screen.getByText('B')).toBeInTheDocument();
  });

  it('filter chip narrows the visible set', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'shipped')]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'pending' }));
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(screen.queryByText('B')).not.toBeInTheDocument();
  });

  it('shows the empty-state message when no rows match the filter', () => {
    render(<BulkOrdersClient initialOrders={[fakeOrder('A', 'pending')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'cancelled' }));
    expect(screen.getByText(/No orders match the current filter/i)).toBeInTheDocument();
  });
});

describe('BulkOrdersClient · select state', () => {
  it('header checkbox selects every visible row', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'pending')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    // Each Bulk button label includes the selection count
    const selectedBadge = screen.getByText('2 selected');
    expect(selectedBadge).toBeInTheDocument();
  });

  it('selected count updates as per-row checkboxes toggle', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'pending')]}
      />,
    );
    fireEvent.click(screen.getByLabelText('Select order A'));
    expect(screen.getByText('1 selected')).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Select order B'));
    expect(screen.getByText('2 selected')).toBeInTheDocument();
  });
});

describe('BulkOrdersClient · cross-status action gating', () => {
  it('Confirm enabled when all selected are pending; Move-to-processing disabled', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'pending')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    expect(screen.getByRole('button', { name: /^Confirm \(2\)$/ })).not.toBeDisabled();
    expect(
      screen.getByRole('button', { name: /^Move to processing \(2\)$/ }),
    ).toBeDisabled();
  });

  it('Move-to-processing enabled when all selected are confirmed; Confirm disabled', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'confirmed'), fakeOrder('B', 'confirmed')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    expect(screen.getByRole('button', { name: /^Confirm \(2\)$/ })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: /^Move to processing \(2\)$/ }),
    ).not.toBeDisabled();
  });

  it('mixed-status selection disables every status-advance button', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'confirmed')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    expect(screen.getByRole('button', { name: /^Confirm \(2\)$/ })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: /^Move to processing \(2\)$/ }),
    ).toBeDisabled();
    // Cancel is still allowed because none of the selected are terminal.
    expect(screen.getByRole('button', { name: /^Cancel \(2\)$/ })).not.toBeDisabled();
  });

  it('Cancel disabled when any selected row is in a terminal status', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'cancelled')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    expect(screen.getByRole('button', { name: /^Cancel \(2\)$/ })).toBeDisabled();
  });

  it('Cancel disabled when any selected row is delivered', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'shipped'), fakeOrder('B', 'delivered')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible orders/i));
    expect(screen.getByRole('button', { name: /^Cancel \(2\)$/ })).toBeDisabled();
  });

  // The engine's order lifecycle allows cancel from pending, confirmed and
  // processing only; shipped (incl. partially), delivered, refunded and
  // cancelled orders are refused.
  it.each([
    ['pending', true],
    ['confirmed', true],
    ['processing', true],
    ['partially_shipped', false],
    ['shipped', false],
    ['delivered', false],
    ['cancelled', false],
    ['refunded', false],
  ] as [Order['status'], boolean][])('Cancel for a lone %s order: enabled=%s', (status, enabled) => {
    render(<BulkOrdersClient initialOrders={[fakeOrder('A', status)]} />);
    fireEvent.click(screen.getByLabelText('Select order A'));
    const cancel = screen.getByRole('button', { name: /^Cancel \(1\)$/ });
    if (enabled) expect(cancel).not.toBeDisabled();
    else expect(cancel).toBeDisabled();
  });

  it('offers a filter chip for every engine order status', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'partially_shipped'), fakeOrder('B', 'refunded')]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'partially shipped' }));
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(screen.queryByText('B')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'refunded' }));
    expect(screen.getByText('B')).toBeInTheDocument();
  });

  it('all bulk actions disabled when no rows are selected', () => {
    render(<BulkOrdersClient initialOrders={[fakeOrder('A', 'pending')]} />);
    expect(screen.getByRole('button', { name: /^Confirm \(0\)$/ })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: /^Move to processing \(0\)$/ }),
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: /^Cancel \(0\)$/ })).toBeDisabled();
  });
});

describe('BulkOrdersClient · CSV export', () => {
  it('export-CSV button label switches between selected count and visible count', () => {
    render(
      <BulkOrdersClient
        initialOrders={[fakeOrder('A', 'pending'), fakeOrder('B', 'pending')]}
      />,
    );
    // Nothing selected → label shows visible count (2)
    expect(
      screen.getByRole('button', { name: /^Export CSV \(2\)$/ }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Select order A'));
    // 1 selected → label switches to "(1)"
    expect(
      screen.getByRole('button', { name: /^Export CSV \(1\)$/ }),
    ).toBeInTheDocument();
  });

  it('clicking Export CSV calls the download helper', () => {
    render(<BulkOrdersClient initialOrders={[fakeOrder('A', 'pending')]} />);
    fireEvent.click(screen.getByRole('button', { name: /^Export CSV/ }));
    expect(downloadCsvSpy).toHaveBeenCalledTimes(1);
    const [filename, csv] = downloadCsvSpy.mock.calls[0]!;
    expect(filename).toMatch(/^orders-.*\.csv$/);
    // CSV body must include the test row's id
    expect(csv).toContain('"A"');
  });
});
