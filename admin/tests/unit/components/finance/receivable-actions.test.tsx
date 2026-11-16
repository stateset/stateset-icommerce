// Receivable action forms offer only what the engine accepts: issued, open
// invoices (disputed included — the engine applies payments, credits and
// write-offs to them) and credit memos with an unapplied balance.

import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>) => useEmbeddedDataMock(fetcher),
}));
vi.mock('@/app/actions/erp', () => ({
  getReceivableActionsData: vi.fn(),
  applyReceivablePayment: vi.fn(),
  applyReceivableCredit: vi.fn(),
  writeOffReceivable: vi.fn(),
  reverseReceivableWriteOff: vi.fn(),
}));

import ReceivableActions from '@/components/finance/receivable-actions';
import { CREDIT_MEMO_STATUSES, INVOICE_STATUSES } from '@/lib/engine-lifecycles';

function offered(select: HTMLElement): string[] {
  return within(select)
    .getAllByRole('option')
    .map((option) => (option as HTMLOptionElement).value)
    .filter(Boolean);
}

describe('ReceivableActions', () => {
  it('offers open invoices (disputed included) and memos with an unapplied balance', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        invoices: INVOICE_STATUSES.map((status) => ({
          id: `inv_${status}`,
          invoiceNumber: `INV-${status}`,
          status,
          currency: 'USD',
          balanceDue: '10.00',
        })),
        credits: CREDIT_MEMO_STATUSES.map((status) => ({
          id: `cm_${status}`,
          creditMemoNumber: `CM-${status}`,
          customerId: 'cus_1',
          status,
        })),
        writeOffs: [],
      },
      error: null,
      refetch: vi.fn(),
    });
    render(<ReceivableActions refresh={async () => {}} />);

    const invoiceSelects = screen.getAllByLabelText('Invoice');
    // Apply payment, apply credit and write off all share the invoice list.
    expect(invoiceSelects).toHaveLength(3);
    for (const select of invoiceSelects) {
      expect(offered(select)).toEqual([
        'inv_sent',
        'inv_viewed',
        'inv_partially_paid',
        'inv_overdue',
        'inv_disputed',
      ]);
    }
    expect(offered(screen.getByLabelText('Credit memo'))).toEqual([
      'cm_open',
      'cm_partially_applied',
    ]);
  });
});
