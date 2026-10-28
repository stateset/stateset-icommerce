import { it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import ErpActionForm from '@/components/finance/erp-action-form';
it('reviews exact money before submission, disables duplicate clicks and retains errors', async () => {
  let reject!: (error: Error) => void;
  const submit = vi.fn(
    () =>
      new Promise<void>((_, fail) => {
        reject = fail;
      }),
  );
  render(
    <ErpActionForm
      title="Pay bill"
      description="Record a payment"
      fields={[{ name: 'amount', label: 'Amount' }]}
      submit={submit}
    />,
  );
  fireEvent.change(screen.getByLabelText('Amount'), { target: { value: '9007199254740993.01' } });
  fireEvent.click(screen.getByRole('button', { name: 'Review pay bill' }));
  expect(submit).not.toHaveBeenCalled();
  expect(screen.getByText('9007199254740993.01')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Confirm pay bill' }));
  expect(submit).toHaveBeenCalledExactlyOnceWith({ amount: '9007199254740993.01' });
  expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
  reject(new Error('Insufficient balance'));
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Insufficient balance'));
  expect(screen.getByRole('button', { name: 'Edit' })).toBeEnabled();
});
