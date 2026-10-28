import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const actions = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
vi.mock('@/app/actions/finance', () => ({
  getGlPostingConfig: actions.get,
  setGlPostingConfig: actions.set,
}));
import PostingSettings from '@/components/finance/posting-settings';

const config = {
  id: 'config',
  configName: 'Store',
  cashAccountId: 'asset',
  accountsReceivableAccountId: 'asset',
  inventoryAccountId: 'asset',
  accountsPayableAccountId: 'liability',
  salesRevenueAccountId: 'revenue',
  cogsAccountId: 'expense',
  autoPostDepreciation: true,
  autoPostRevenueRecognition: true,
};
const accounts = ['asset', 'liability', 'revenue', 'expense'].map((kind) => ({
  id: kind,
  accountNumber: kind,
  name: kind,
  accountType: kind,
  status: 'active',
  balance: 0,
}));

describe('ledger posting settings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    actions.get.mockResolvedValue(config);
  });

  it('loads, edits and saves the selected tax account', async () => {
    actions.set.mockImplementation(async (input) => ({ ...input, id: 'updated' }));
    render(<PostingSettings accounts={accounts} />);
    await waitFor(() => expect(screen.getByDisplayValue('Store')).toBeInTheDocument());
    fireEvent.click(screen.getByText(/Automatic ledger posting/));
    fireEvent.change(screen.getByLabelText('Sales tax payable'), {
      target: { value: 'liability' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save posting settings' }));
    await screen.findByRole('status');
    expect(actions.set).toHaveBeenCalledWith(
      expect.objectContaining({
        salesTaxPayableAccountId: 'liability',
        autoPostDepreciation: true,
      }),
    );
  });

  it('shows a rejected save without reporting success', async () => {
    actions.set.mockRejectedValue(new Error('Posting account is inactive'));
    render(<PostingSettings accounts={accounts} />);
    await waitFor(() => expect(screen.getByDisplayValue('Store')).toBeInTheDocument());
    fireEvent.click(screen.getByText(/Automatic ledger posting/));
    fireEvent.click(screen.getByRole('button', { name: 'Save posting settings' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Posting account is inactive');
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});
