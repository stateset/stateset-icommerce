'use client';

import { useState } from 'react';

export type AffiliatePayout = {
  batch: string;
  affiliate: string;
  currency: string;
  amount: string;
  rail: 'external' | 'stripe';
  status: string;
  reference: string | null;
  settledAmount: string;
  reversedAmount: string;
  paidAt: string | null;
  bankPayouts: {
    id: string;
    status: string;
    currency: string;
    amount: string;
    arrivalDate: string;
    transactionId: string;
  }[];
  confirmedAt: string | null;
  reservedAt: string | null;
  lines: { orderId: string | null; amount: string; transferId?: string; batchId?: string }[] | null;
};
type Batch = {
  alreadyReserved?: boolean;
  batchId: string;
  rail: string;
  fingerprint: string;
  rows: { affiliate: string; currency: string; amount: string }[];
};
async function post(body: Record<string, unknown>) {
  const response = await fetch('/api/portal', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || 'Unable to update commissions');
  return result;
}
export function AffiliatePayouts({
  payouts,
  merchant,
  refresh,
}: {
  payouts: AffiliatePayout[];
  merchant: boolean;
  refresh: () => Promise<void>;
}) {
  const [preview, setPreview] = useState<Batch | null>(null);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState('');
  async function submit(fn: () => Promise<void>) {
    setBusy(true);
    setError('');
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Payout operation failed');
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-3">
      <h2 className="text-lg font-semibold">Commission payouts</h2>
      {merchant && (
        <form
          className="flex flex-wrap items-end gap-3"
          onChange={() => setPreview(null)}
          onSubmit={(e) => {
            e.preventDefault();
            const values = new FormData(e.currentTarget);
            void submit(async () => {
              const result = await post({
                action: 'reserve-commissions',
                batch: values.get('batch'),
                rail: values.get('rail'),
                affiliate: values.get('affiliate') || undefined,
                apply: Boolean(preview),
                fingerprint: preview?.fingerprint,
              });
              if (preview) {
                setPreview(null);
                await refresh();
              } else setPreview(result);
            });
          }}
        >
          <label>
            Batch ID
            <input
              className="block rounded border p-2"
              name="batch"
              required
              pattern="[a-z0-9][a-z0-9_\-]{0,47}"
            />
          </label>
          <label>
            Affiliate (optional)
            <input
              name="affiliate"
              className="block rounded border p-2"
              placeholder="All available affiliates"
            />
          </label>
          <label>
            Payment method
            <select className="block rounded border p-2" name="rail">
              <option value="external">External payment</option>
              <option value="stripe">Stripe Connect transfer</option>
            </select>
          </label>
          <button
            className="rounded border px-3 py-2"
            disabled={busy || Boolean(preview && (!preview.rows.length || preview.alreadyReserved))}
          >
            {preview?.alreadyReserved
              ? 'Batch already reserved'
              : preview
                ? 'Reserve reviewed commissions'
                : 'Preview payout batch'}
          </button>
        </form>
      )}
      {merchant && preview && (
        <div aria-live="polite">
          {preview.rows.length ? (
            preview.rows.map((row) => (
              <p key={row.affiliate}>
                {row.affiliate} · {row.currency} {row.amount}
              </p>
            ))
          ) : (
            <p>No commissions available for this batch.</p>
          )}
        </div>
      )}
      {error && <p role="alert">{error}</p>}
      {!payouts.length && <p>No commission payouts yet.</p>}
      {payouts.map((p) => (
        <details key={p.batch + ':' + p.affiliate} className="space-y-2 rounded border p-3">
          <summary>
            {p.batch} · {merchant && p.affiliate + ' · '}
            {p.currency} {p.amount} · {p.status}
          </summary>
          <p>
            {p.rail === 'stripe' ? 'Stripe connected balance' : 'External payment'}
            {p.reference && ' · ' + p.reference}
          </p>
          {p.paidAt && (
            <p>
              {p.rail === 'stripe' ? 'Transferred' : 'Paid'} {new Date(p.paidAt).toLocaleString()}
            </p>
          )}
          {p.confirmedAt && <p>Merchant confirmation {new Date(p.confirmedAt).toLocaleString()}</p>}
          {p.bankPayouts.map((line) => (
            <p key={line.transactionId}>
              Bank payout {line.id} · {line.status} · {line.currency} {line.amount} · expected
              arrival {line.arrivalDate}
            </p>
          ))}
          {merchant && p.rail === 'stripe' && p.status !== 'reserved' && (
            <button
              className="mr-2 underline"
              disabled={busy}
              onClick={() =>
                void submit(async () => {
                  await post({
                    action: 'sync-affiliate-bank',
                    affiliate: p.affiliate,
                    apply: true,
                  });
                  await refresh();
                })
              }
            >
              Check bank payout
            </button>
          )}
          {p.reversedAmount !== '0' && (
            <p>
              {p.currency} {p.reversedAmount} reversed · {p.currency} {p.settledAmount} transferred
              after reversals
            </p>
          )}
          {p.lines ? (
            <table className="text-sm">
              <thead>
                <tr>
                  <th className="p-2 text-left">Order or adjustment</th>
                  <th className="p-2">Commission ({p.currency})</th>
                </tr>
              </thead>
              <tbody>
                {p.lines.map((line) => (
                  <tr key={line.transferId || line.orderId || 'carry'}>
                    <td className="p-2">
                      {line.transferId
                        ? 'Transfer reversal from ' + line.batchId + ' · ' + line.transferId
                        : line.orderId || 'Earlier exports without order detail'}
                    </td>
                    <td className="p-2 text-right">{line.amount}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p>This historical export recorded the recipient total only.</p>
          )}
          {merchant && p.rail === 'stripe' && (
            <button
              className="rounded border px-3 py-2"
              disabled={busy}
              onClick={() =>
                void submit(async () => {
                  await post({
                    action: 'settle-commission',
                    batch: p.batch,
                    affiliate: p.affiliate,
                    rail: p.rail,
                    apply: true,
                  });
                  await refresh();
                })
              }
            >
              {p.status === 'reserved'
                ? 'Transfer ' + p.currency + ' ' + p.amount + ' to ' + p.affiliate
                : 'Reconcile Stripe transfer'}
            </button>
          )}
          {merchant && p.rail === 'external' && p.status === 'reserved' && (
            <form
              className="flex flex-wrap items-end gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                const values = new FormData(e.currentTarget);
                void submit(async () => {
                  await post({
                    action: 'settle-commission',
                    batch: p.batch,
                    affiliate: p.affiliate,
                    rail: p.rail,
                    reference: values.get('reference'),
                    paidAt: new Date(String(values.get('paidAt'))).toISOString(),
                    apply: true,
                  });
                  await refresh();
                });
              }}
            >
              <label>
                Payment reference
                <input
                  className="block rounded border p-2"
                  name="reference"
                  required
                  maxLength={160}
                />
              </label>
              <label>
                Paid at (local time)
                <input
                  className="block rounded border p-2"
                  name="paidAt"
                  type="datetime-local"
                  required
                />
              </label>
              <button className="rounded border px-3 py-2" disabled={busy}>
                Record completed payment
              </button>
            </form>
          )}
        </details>
      ))}
    </section>
  );
}
