// Component tests for the RMA Inbox.
//
// These exercise the per-row state-aware action gating (the core safety
// rule: only show actions the engine accepts from the row's current status,
// per the engine's return lifecycle) plus the pending-only filter and
// bulk-select interaction. Server-action calls are mocked.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

const approveSpy = vi.fn();
const rejectSpy = vi.fn();
const cancelSpy = vi.fn();
const trackingSpy = vi.fn();
const receivedSpy = vi.fn();
const completeSpy = vi.fn();
vi.mock('@/app/actions/commerce', () => ({
  approveReturn: (id: string) => approveSpy(id),
  rejectReturn: (id: string, reason: string) => rejectSpy(id, reason),
  cancelReturn: (id: string) => cancelSpy(id),
  addReturnTracking: (id: string, trackingNumber: string) => trackingSpy(id, trackingNumber),
  markReturnReceived: (id: string) => receivedSpy(id),
  completeReturn: (id: string) => completeSpy(id),
}));

import { RmaInboxClient } from '@/components/returns/rma-inbox-client';
import { RETURN_STATUSES, type ReturnStatus } from '@/lib/engine-lifecycles';
import type { Return } from '@/lib/types';

function fakeReturn(id: string, status: ReturnStatus): Return {
  return {
    id,
    orderId: `ORD-${id}`,
    customerId: `CUST-${id}`,
    status,
    items: [{ productId: 'p1', sku: 'A', name: 'A', quantity: 1, returnReason: 'damaged' }],
    reason: 'broken in shipping',
    reasonCategory: 'defective',
    refundAmount: 19.99,
    refundMethod: 'original',
    createdAt: '2026-05-07T12:00:00.000Z',
    updatedAt: '2026-05-07T12:00:00.000Z',
  };
}

const ALL_ACTION_LABELS = [
  'Approve',
  'Add tracking',
  'Mark received',
  'Complete',
  'Reject',
  'Cancel',
] as const;

/** Render one return (with the All filter on) and list its action buttons. */
function actionsFor(status: ReturnStatus): string[] {
  render(<RmaInboxClient initialReturns={[fakeReturn('R1', status)]} />);
  fireEvent.click(screen.getByRole('button', { name: 'All' }));
  const row = screen.getByText('R1').closest('tr')!;
  return within(row)
    .queryAllByRole('button')
    .map((button) => button.textContent ?? '')
    .filter((label) => (ALL_ACTION_LABELS as readonly string[]).includes(label));
}

beforeEach(() => {
  approveSpy.mockReset();
  rejectSpy.mockReset();
  cancelSpy.mockReset();
  trackingSpy.mockReset();
  receivedSpy.mockReset();
  completeSpy.mockReset();
});

describe('RmaInboxClient · row rendering', () => {
  it('renders one row per return', () => {
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'requested'), fakeReturn('R2', 'in_transit')]}
      />,
    );
    expect(screen.getByText('R1')).toBeInTheDocument();
    expect(screen.getByText('R2')).toBeInTheDocument();
    expect(screen.getByText('in transit')).toBeInTheDocument();
  });

  it('labels every engine reason code', () => {
    render(
      <RmaInboxClient
        initialReturns={[{ ...fakeReturn('R1', 'requested'), reasonCategory: 'no_longer_needed' }]}
      />,
    );
    expect(screen.getByText('No longer needed')).toBeInTheDocument();
  });

  it('shows the empty-state message when nothing is pending', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'completed'),
          fakeReturn('R2', 'rejected'),
          fakeReturn('R3', 'cancelled'),
        ]}
      />,
    );
    expect(screen.getByText(/No pending returns. Switch to "All"/i)).toBeInTheDocument();
  });
});

describe('RmaInboxClient · status-aware action gating (engine return lifecycle)', () => {
  // Expected buttons per engine status. Receive is only legal once the
  // return is in transit; rejection only before approval or during
  // inspection; there is no separate refund step (complete settles it).
  const EXPECTED: Record<ReturnStatus, string[]> = {
    requested: ['Approve', 'Reject', 'Cancel'],
    approved: ['Add tracking', 'Cancel'],
    in_transit: ['Mark received'],
    received: ['Complete'],
    inspecting: ['Complete', 'Reject'],
    completed: [],
    rejected: [],
    cancelled: [],
  };

  it('covers every engine return status', () => {
    expect(Object.keys(EXPECTED).sort()).toEqual([...RETURN_STATUSES].sort());
  });

  it.each(Object.entries(EXPECTED) as [ReturnStatus, string[]][])(
    'offers exactly the legal actions for %s',
    (status, expected) => {
      expect(actionsFor(status)).toEqual(expected);
    },
  );
});

describe('RmaInboxClient · actions call the binding-shaped server actions', () => {
  it('Approve calls approveReturn with the row id', async () => {
    approveSpy.mockResolvedValue(fakeReturn('R1', 'approved'));
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'requested')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    await Promise.resolve();
    expect(approveSpy).toHaveBeenCalledWith('R1');
  });

  it('Add tracking prompts for a tracking number and moves the row to in transit', async () => {
    vi.spyOn(window, 'prompt').mockReturnValue(' 1Z999 ');
    trackingSpy.mockResolvedValue({ ...fakeReturn('R1', 'in_transit'), trackingNumber: '1Z999' });
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'approved')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Add tracking' }));
    expect(trackingSpy).toHaveBeenCalledWith('R1', '1Z999');
    // The row re-renders with the next legal action.
    expect(await screen.findByRole('button', { name: 'Mark received' })).toBeInTheDocument();
    expect(screen.getByText('1Z999')).toBeInTheDocument();
  });

  it('Add tracking does nothing when the prompt is dismissed', () => {
    vi.spyOn(window, 'prompt').mockReturnValue(null);
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'approved')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Add tracking' }));
    expect(trackingSpy).not.toHaveBeenCalled();
  });

  it('Mark received and Complete call their server actions', async () => {
    receivedSpy.mockResolvedValue(fakeReturn('R1', 'received'));
    completeSpy.mockResolvedValue(fakeReturn('R2', 'completed'));
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'in_transit'), fakeReturn('R2', 'inspecting')]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Mark received' }));
    expect(receivedSpy).toHaveBeenCalledWith('R1');
    const inspectingRow = screen.getByText('R2').closest('tr')!;
    fireEvent.click(within(inspectingRow).getByRole('button', { name: 'Complete' }));
    expect(completeSpy).toHaveBeenCalledWith('R2');
  });

  it('Cancel asks for confirmation first', () => {
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'approved')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(confirm).toHaveBeenCalled();
    expect(cancelSpy).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    cancelSpy.mockResolvedValue(fakeReturn('R1', 'cancelled'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(cancelSpy).toHaveBeenCalledWith('R1');
  });

  it('Reject requires a reason', () => {
    vi.spyOn(window, 'prompt').mockReturnValueOnce('   ').mockReturnValueOnce('wrong SKU');
    rejectSpy.mockResolvedValue(fakeReturn('R1', 'rejected'));
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'requested')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Reject' }));
    expect(rejectSpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Reject' }));
    expect(rejectSpy).toHaveBeenCalledWith('R1', 'wrong SKU');
  });

  it('shows the engine refusal on the row', async () => {
    receivedSpy.mockRejectedValue(new Error('Return cannot move from approved to received'));
    render(<RmaInboxClient initialReturns={[fakeReturn('R1', 'in_transit')]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Mark received' }));
    expect(
      await screen.findByText('Return cannot move from approved to received'),
    ).toBeInTheDocument();
  });
});

describe('RmaInboxClient · filter toggle', () => {
  it('Pending-only shows every non-terminal engine status', () => {
    render(
      <RmaInboxClient
        initialReturns={[
          fakeReturn('R1', 'requested'),
          fakeReturn('R2', 'approved'),
          fakeReturn('R3', 'in_transit'),
          fakeReturn('R4', 'received'),
          fakeReturn('R5', 'inspecting'),
          fakeReturn('R6', 'completed'),
          fakeReturn('R7', 'rejected'),
          fakeReturn('R8', 'cancelled'),
        ]}
      />,
    );
    for (const id of ['R1', 'R2', 'R3', 'R4', 'R5']) {
      expect(screen.getByText(id)).toBeInTheDocument();
    }
    for (const id of ['R6', 'R7', 'R8']) {
      expect(screen.queryByText(id)).not.toBeInTheDocument();
    }
  });

  it('All filter shows completed/rejected/cancelled records too', () => {
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'requested'), fakeReturn('R2', 'completed')]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'All' }));
    expect(screen.getByText('R1')).toBeInTheDocument();
    expect(screen.getByText('R2')).toBeInTheDocument();
  });
});

describe('RmaInboxClient · bulk approve', () => {
  it('header checkbox selects all visible rows and updates the bulk button label', () => {
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'requested'), fakeReturn('R2', 'requested')]}
      />,
    );
    const headerCheckbox = screen.getByLabelText(/select all visible returns/i);
    fireEvent.click(headerCheckbox);
    expect(screen.getByRole('button', { name: /Approve 2 selected returns/i })).toBeEnabled();

    fireEvent.click(headerCheckbox);
    expect(screen.getByRole('button', { name: /Approve 0 selected returns/i })).toBeDisabled();
  });

  it('is disabled when any selected return is past approval', () => {
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'requested'), fakeReturn('R2', 'approved')]}
      />,
    );
    fireEvent.click(screen.getByLabelText('Select return R1'));
    expect(screen.getByRole('button', { name: /Approve 1 selected returns/i })).toBeEnabled();
    fireEvent.click(screen.getByLabelText('Select return R2'));
    expect(screen.getByRole('button', { name: /Approve 2 selected returns/i })).toBeDisabled();
  });

  it('approves each selected return', async () => {
    approveSpy.mockImplementation(async (id: string) => fakeReturn(id, 'approved'));
    render(
      <RmaInboxClient
        initialReturns={[fakeReturn('R1', 'requested'), fakeReturn('R2', 'requested')]}
      />,
    );
    fireEvent.click(screen.getByLabelText(/select all visible returns/i));
    fireEvent.click(screen.getByRole('button', { name: /Approve 2 selected returns/i }));
    await screen.findAllByRole('button', { name: 'Add tracking' });
    expect(approveSpy).toHaveBeenCalledWith('R1');
    expect(approveSpy).toHaveBeenCalledWith('R2');
  });
});
