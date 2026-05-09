// Component tests for the RMA Inbox.
//
// These exercise the per-row state-aware action gating (the core safety
// rule: only show actions valid for the current row status) plus the
// pending-only filter and bulk-select interaction. Server-action calls
// are mocked.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

const approveSpy = vi.fn();
const rejectSpy = vi.fn();
const receiveSpy = vi.fn();
const refundSpy = vi.fn();
vi.mock('@/app/actions/commerce', () => ({
  approveReturn: (id: string) => approveSpy(id),
  rejectReturn: (id: string, reason: string) => rejectSpy(id, reason),
  receiveReturn: (id: string, items: unknown) => receiveSpy(id, items),
  processRefund: (id: string, method: string) => refundSpy(id, method),
}));

import { RmaInboxClient } from '@/components/returns/rma-inbox-client';
import type { Return } from '@/lib/types';

function fakeReturn(id: string, status: Return['status']): Return {
  return {
    id,
    orderId: `ORD-${id}`,
    customerId: `CUST-${id}`,
    status,
    items: [
      { productId: 'p1', sku: 'A', name: 'A', quantity: 1, returnReason: 'damaged' },
    ],
    reason: 'broken in shipping',
    reasonCategory: 'defective',
    refundAmount: 19.99,
    refundMethod: 'original',
    createdAt: '2026-05-07T12:00:00.000Z',
    updatedAt: '2026-05-07T12:00:00.000Z',
  };
}

describe('RmaInboxClient · row rendering', () => {
  beforeEach(() => {
    approveSpy.mockReset();
    rejectSpy.mockReset();
    receiveSpy.mockReset();
    refundSpy.mockReset();
  });

  it('renders one row per return', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'approved'),
        ]}
      />,
    );
    // Body rows; the header row isn't a return so look for the IDs.
    expect(screen.getByText('R1')).toBeInTheDocument();
    expect(screen.getByText('R2')).toBeInTheDocument();
  });

  it('shows the empty-state message when nothing is pending', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'refunded'),
          fakeReturn('R2', 'closed'),
        ]}
      />,
    );
    expect(
      screen.getByText(/No pending returns. Switch to "All"/i),
    ).toBeInTheDocument();
  });
});

describe('RmaInboxClient · status-aware action gating', () => {
  it('shows Approve + Reject when status is requested', () => {
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'requested')]} />);
    const row = screen.getByText('R1').closest('tr')!;
    const utils = within(row);
    expect(utils.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
    expect(utils.getByRole('button', { name: 'Reject' })).toBeInTheDocument();
    expect(utils.queryByRole('button', { name: /received/i })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: 'Refund' })).not.toBeInTheDocument();
  });

  it('shows Reject + Mark received when status is approved', () => {
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'approved')]} />);
    const row = screen.getByText('R1').closest('tr')!;
    const utils = within(row);
    expect(utils.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
    expect(utils.getByRole('button', { name: 'Reject' })).toBeInTheDocument();
    expect(utils.getByRole('button', { name: /Mark received/i })).toBeInTheDocument();
    expect(utils.queryByRole('button', { name: 'Refund' })).not.toBeInTheDocument();
  });

  it('shows only Refund when status is received', () => {
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'received')]} />);
    const row = screen.getByText('R1').closest('tr')!;
    const utils = within(row);
    expect(utils.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: 'Reject' })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: /Mark received/i })).not.toBeInTheDocument();
    expect(utils.getByRole('button', { name: 'Refund' })).toBeInTheDocument();
  });

  it('shows no actions when status is refunded', () => {
    // Need to flip filter to All so refunded rows render at all.
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'refunded')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'All' }));
    const row = screen.getByText('R1').closest('tr')!;
    const utils = within(row);
    expect(utils.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: 'Reject' })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: /Mark received/i })).not.toBeInTheDocument();
    expect(utils.queryByRole('button', { name: 'Refund' })).not.toBeInTheDocument();
  });
});

describe('RmaInboxClient · filter toggle', () => {
  it('Pending-only is selected by default and shows only open statuses', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'refunded'),
        ]}
      />,
    );
    expect(screen.getByText('R1')).toBeInTheDocument();
    expect(screen.queryByText('R2')).not.toBeInTheDocument();
  });

  it('All filter shows refunded/rejected/closed records too', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'refunded'),
        ]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'All' }));
    expect(screen.getByText('R1')).toBeInTheDocument();
    expect(screen.getByText('R2')).toBeInTheDocument();
  });
});

describe('RmaInboxClient · bulk select', () => {
  it('header checkbox selects all visible rows and updates the bulk button label', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'approved'),
        ]}
      />,
    );
    const headerCheckbox = screen.getByLabelText(/select all visible returns/i);
    fireEvent.click(headerCheckbox);
    expect(
      screen.getByRole('button', { name: /Approve 2 selected returns/i }),
    ).toBeInTheDocument();

    // Toggle off — unselects everything.
    fireEvent.click(headerCheckbox);
    expect(
      screen.getByRole('button', { name: /Approve 0 selected returns/i }),
    ).toBeInTheDocument();
  });

  it('per-row checkbox toggles the bulk count', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'approved'),
        ]}
      />,
    );
    fireEvent.click(screen.getByLabelText('Select return R1'));
    expect(
      screen.getByRole('button', { name: /Approve 1 selected returns/i }),
    ).toBeInTheDocument();
  });
});

describe('RmaInboxClient · approve action', () => {
  it('clicking Approve calls approveReturn with the row id', async () => {
    approveSpy.mockResolvedValue(fakeReturn('R1', 'approved'));
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'requested')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    // Microtask flush
    await Promise.resolve();
    expect(approveSpy).toHaveBeenCalledWith('R1');
  });
});
