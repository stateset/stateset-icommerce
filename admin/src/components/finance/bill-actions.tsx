'use client';
import ErpActionForm from './erp-action-form';
import { approveSupplierBill, paySupplierBill } from '@/app/actions/erp';
import { formatMoney } from '@/lib/finance/format';
import type { Bill } from '@/lib/embedded';

export default function BillActions({
  bills,
  refresh,
}: {
  bills: Bill[];
  refresh: () => Promise<void>;
}) {
  const payable = bills.filter((b) =>
    ['approved', 'partially_paid', 'overdue'].includes(
      b.status.toLowerCase().replace('partiallypaid', 'partially_paid'),
    ),
  );
  const options = (rows: Bill[]) =>
    rows.map((b) => ({
      value: b.id,
      label: `${b.billNumber} · ${formatMoney(b.amountDue, b.currency ?? 'USD')} due`,
    }));
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <ErpActionForm
        title="Approve bill"
        description="Approval checks the purchase order and accepted receipts. The bill's creator cannot approve it."
        fields={[
          {
            name: 'billId',
            label: 'Bill',
            options: options(bills.filter((b) => b.status.toLowerCase() === 'pending')),
          },
          {
            name: 'overrideReason',
            label: 'Match override reason',
            optional: true,
            hint: 'Only complete this when explicitly accepting a purchase or receipt mismatch.',
          },
        ]}
        submit={approveSupplierBill}
        onComplete={refresh}
      />
      <ErpActionForm
        title="Record supplier payment"
        description="Record a payment already made to the supplier. The bill determines the currency."
        fields={[
          { name: 'billId', label: 'Bill', options: options(payable) },
          { name: 'amount', label: 'Amount', hint: 'Exact amount, for example 125.50' },
          {
            name: 'paymentMethod',
            label: 'Method',
            options: ['ach', 'wire', 'check', 'cash', 'credit_card', 'other'].map((value) => ({
              value,
              label: value.replace('_', ' '),
            })),
          },
          { name: 'referenceNumber', label: 'Payment reference', optional: true },
        ]}
        submit={paySupplierBill}
        onComplete={refresh}
      />
    </div>
  );
}
