// Bill action forms offer only the bills the engine accepts for each action:
// approval from draft or pending (ap_match.rs), payment against approved,
// partially paid or overdue bills (ap_settlement.rs).

import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@/app/actions/erp', () => ({
  approveSupplierBill: vi.fn(),
  paySupplierBill: vi.fn(),
}));

import BillActions from '@/components/finance/bill-actions';
import { BILL_STATUSES, type BillStatus } from '@/lib/engine-lifecycles';
import type { Bill } from '@/lib/embedded';

const bill = (status: BillStatus): Bill => ({
  id: `bill_${status}`,
  billNumber: `BILL-${status}`,
  supplierId: 'sup_1',
  currency: 'USD',
  status,
  totalAmount: '100.00',
  amountPaid: '0',
  amountDue: '100.00',
  dueDate: '2026-08-01',
  createdAt: '2026-07-01T00:00:00.000Z',
});

function offered(select: HTMLElement): string[] {
  return within(select)
    .getAllByRole('option')
    .map((option) => (option as HTMLOptionElement).value)
    .filter(Boolean);
}

describe('BillActions', () => {
  it('offers draft and pending bills for approval and approved/partially paid/overdue for payment', () => {
    render(<BillActions bills={BILL_STATUSES.map(bill)} refresh={async () => {}} />);
    const [approveSelect, paySelect] = screen.getAllByLabelText('Bill');

    expect(offered(approveSelect)).toEqual(['bill_draft', 'bill_pending']);
    expect(offered(paySelect)).toEqual(['bill_approved', 'bill_partially_paid', 'bill_overdue']);
  });
});
