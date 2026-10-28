'use client';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import {
  getReceivableActionsData,
  applyReceivablePayment,
  applyReceivableCredit,
  writeOffReceivable,
  reverseReceivableWriteOff,
} from '@/app/actions/erp';
import { formatMoney } from '@/lib/finance/format';
import ErpActionForm, { type ErpField } from './erp-action-form';

export default function ReceivableActions({ refresh }: { refresh: () => Promise<void> }) {
  const { data, error, refetch } = useEmbeddedData(getReceivableActionsData);
  if (error) return <p role="alert">Receivable actions unavailable: {error.message}</p>;
  if (!data) return <p>Loading receivable actions…</p>;
  const invoice: ErpField = {
    name: 'invoiceId',
    label: 'Invoice',
    options: data.invoices
      .filter((i) => ['sent', 'viewed', 'partially_paid', 'overdue'].includes(i.status))
      .map((i) => ({
        value: i.id,
        label: `${i.invoiceNumber} · ${formatMoney(i.balanceDue, i.currency)} due`,
      })),
  };
  const amount: ErpField = {
    name: 'amount',
    label: 'Amount',
    hint: 'Exact decimal amount in the invoice currency.',
  };
  async function reload() {
    await Promise.all([refresh(), refetch()]);
  }
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <ErpActionForm
        title="Apply customer payment"
        description="Allocate an existing completed payment. Customer, currency and available funds must match."
        fields={[invoice, { name: 'paymentId', label: 'Completed payment ID' }, amount]}
        submit={applyReceivablePayment}
        onComplete={reload}
      />
      <ErpActionForm
        title="Apply customer credit"
        description="Reduce an issued invoice with an available credit memo for the same customer and currency."
        fields={[
          invoice,
          {
            name: 'creditMemoId',
            label: 'Credit memo',
            options: data.credits
              .filter((c) => !['voided', 'applied'].includes(c.status.toLowerCase()))
              .map((c) => ({
                value: c.id,
                label: `${c.creditMemoNumber} · customer ${c.customerId}`,
              })),
          },
          amount,
        ]}
        submit={applyReceivableCredit}
        onComplete={reload}
      />
      <ErpActionForm
        title="Write off receivable"
        description="Write off only the stated amount. Any remaining balance stays collectible."
        fields={[
          invoice,
          amount,
          {
            name: 'reason',
            label: 'Reason',
            options: [
              'uncollectible',
              'bankruptcy',
              'customer_dispute',
              'small_balance',
              'account_closed',
              'deceased',
              'other',
            ].map((value) => ({ value, label: value.replaceAll('_', ' ') })),
          },
          { name: 'notes', label: 'Notes', optional: true },
        ]}
        submit={writeOffReceivable}
        onComplete={reload}
      />
      <ErpActionForm
        title="Reverse write-off"
        description="Restore the written-off amount and reverse its bad-debt record."
        fields={[
          {
            name: 'writeOffId',
            label: 'Write-off',
            options: data.writeOffs.map((w) => ({
              value: w.id,
              label: `${w.invoiceId} · ${w.amount} · ${w.reason}`,
            })),
          },
        ]}
        submit={reverseReceivableWriteOff}
        onComplete={reload}
      />
    </div>
  );
}
