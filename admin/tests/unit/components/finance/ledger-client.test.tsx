// Component test for the Finance > Ledger page client. Drives the
// action-backed loader through its data / error branches.

import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

// Enumerate named exports explicitly (Proxy mocks hang vitest imports).
const getLedgerPageData = vi.fn();
vi.mock('@/app/actions/finance', () => ({
  getLedgerPageData: (...args: unknown[]) => getLedgerPageData(...args),
}));

import LedgerClient from '@/components/finance/ledger-client';

afterEach(() => {
  vi.clearAllMocks();
});

const ledgerData = {
  accounts: [
    {
      id: 'gl_1000',
      accountNumber: '1000',
      name: 'Cash',
      accountType: 'asset',
      balance: 182450.25,
      status: 'active',
    },
  ],
  trialBalance: {
    asOfDate: '2026-07-20',
    totalDebits: 439745.95,
    totalCredits: 439745.95,
    isBalanced: true,
  },
  journalEntries: [
    {
      id: 'je_1',
      entryNumber: 'JE-2000',
      entryDate: '2026-07-18',
      description: 'Daily sales posting',
      status: 'posted',
      createdAt: '2026-07-18T00:00:00.000Z',
    },
  ],
};

describe('LedgerClient', () => {
  it('renders accounts, trial balance, and the balanced badge', async () => {
    getLedgerPageData.mockResolvedValue(ledgerData);
    render(<LedgerClient />);

    expect(await screen.findByText('General Ledger')).toBeInTheDocument();
    expect(screen.getByText('Cash')).toBeInTheDocument();
    expect(screen.getByText('Balanced')).toBeInTheDocument();
    // Exact decimal formatting, twice (debits + credits).
    expect(screen.getAllByText('$439,745.95')).toHaveLength(2);
    expect(screen.getByText('JE-2000')).toBeInTheDocument();
    // Called with an ISO as-of date.
    expect(getLedgerPageData).toHaveBeenCalledWith(
      expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/)
    );
  });

  it('shows the out-of-balance badge when the engine reports imbalance', async () => {
    getLedgerPageData.mockResolvedValue({
      ...ledgerData,
      trialBalance: { ...ledgerData.trialBalance, isBalanced: false },
    });
    render(<LedgerClient />);

    expect(await screen.findByText('Out of balance')).toBeInTheDocument();
  });

  it('renders the error card when the action fails', async () => {
    getLedgerPageData.mockRejectedValue(new Error('engine down'));
    render(<LedgerClient />);

    await waitFor(() =>
      expect(screen.getByText('Failed to load general ledger')).toBeInTheDocument()
    );
  });
});
