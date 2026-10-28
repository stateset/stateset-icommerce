'use client';

import { useEffect, useState } from 'react';
import { Card, CardContent } from '@stateset/design';
import { getGlPostingConfig, setGlPostingConfig } from '@/app/actions/finance';
import type { AutoPostingConfigInput, GlAccount } from '@/lib/embedded';

const accountsToConfigure = [
  ['cashAccountId', 'Cash', 'asset', true],
  ['accountsReceivableAccountId', 'Accounts receivable', 'asset', true],
  ['inventoryAccountId', 'Inventory', 'asset', true],
  ['accountsPayableAccountId', 'Accounts payable', 'liability', true],
  ['salesRevenueAccountId', 'Sales revenue', 'revenue', true],
  ['cogsAccountId', 'Cost of goods sold', 'expense', true],
  ['salesTaxPayableAccountId', 'Sales tax payable', 'liability', false],
  ['shippingRevenueAccountId', 'Shipping revenue', 'revenue', false],
  ['goodsReceivedNotInvoicedAccountId', 'Goods received, not invoiced', 'liability', false],
  ['inventoryAdjustmentAccountId', 'Inventory adjustments', 'expense', false],
  ['workInProgressAccountId', 'Work in progress', 'asset', false],
  ['unearnedRevenueAccountId', 'Unearned revenue', 'liability', false],
  ['badDebtExpenseAccountId', 'Bad debt expense', 'expense', false],
  ['fxGainLossAccountId', 'Foreign exchange gain or loss', 'income', false],
] as const;

const empty: AutoPostingConfigInput = {
  configName: 'Default',
  cashAccountId: '',
  accountsReceivableAccountId: '',
  inventoryAccountId: '',
  accountsPayableAccountId: '',
  salesRevenueAccountId: '',
  cogsAccountId: '',
  autoPostDepreciation: true,
  autoPostRevenueRecognition: true,
};

export default function PostingSettings({ accounts }: { accounts: GlAccount[] }) {
  const [input, setInput] = useState<AutoPostingConfigInput>(empty);
  const [configured, setConfigured] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let active = true;
    getGlPostingConfig()
      .then((config) => {
        if (!active) return;
        if (config) {
          setInput(config);
          setConfigured(true);
        }
      })
      .catch((err: unknown) => {
        if (active)
          setError(err instanceof Error ? err.message : 'Unable to load posting settings');
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <Card>
      <CardContent>
        <details>
          <summary className="cursor-pointer text-base font-semibold">
            Automatic ledger posting ·{' '}
            {loading ? 'Loading' : configured ? 'Configured' : 'Not configured'}
          </summary>
          <p className="mt-3 text-sm text-ds-muted-foreground">
            Choose the accounts used by operational postings. Tax, shipping, receipts, and
            adjustments require their corresponding accounts. Month-end close requires posting
            settings.
          </p>
          <form
            className="mt-4 space-y-4"
            onSubmit={async (event) => {
              event.preventDefault();
              setSaving(true);
              setError(null);
              setSaved(false);
              try {
                const result = await setGlPostingConfig(input);
                setInput(result);
                setConfigured(true);
                setSaved(true);
              } catch (err) {
                setError(err instanceof Error ? err.message : 'Unable to save posting settings');
              } finally {
                setSaving(false);
              }
            }}
          >
            <fieldset disabled={loading || saving} className="space-y-4">
              <label className="block text-sm">
                Configuration name
                <input
                  required
                  value={input.configName}
                  onChange={(e) => setInput({ ...input, configName: e.target.value })}
                  className="mt-1 block rounded border border-ds-border bg-ds-background p-2"
                />
              </label>
              <div className="grid gap-4 md:grid-cols-2">
                {accountsToConfigure.map(([key, label, kind, required]) => (
                  <label key={key} className="block text-sm">
                    {label}
                    {required ? ' *' : ''}
                    <select
                      required={required}
                      value={input[key] ?? ''}
                      onChange={(e) => setInput({ ...input, [key]: e.target.value || undefined })}
                      className="mt-1 block w-full rounded border border-ds-border bg-ds-background p-2"
                    >
                      <option value="">Select account</option>
                      {accounts
                        .filter(
                          (a) =>
                            a.status.toLowerCase() === 'active' &&
                            !a.isHeader &&
                            a.isPosting !== false &&
                            (a.accountType.toLowerCase() === kind ||
                              (kind === 'income' &&
                                ['revenue', 'expense'].includes(a.accountType.toLowerCase()))),
                        )
                        .map((a) => (
                          <option key={a.id} value={a.id}>
                            {a.accountNumber} · {a.name}
                          </option>
                        ))}
                    </select>
                  </label>
                ))}
              </div>
              <label className="flex gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={input.autoPostDepreciation}
                  onChange={(e) => setInput({ ...input, autoPostDepreciation: e.target.checked })}
                />
                Post depreciation to the ledger
              </label>
              <label className="flex gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={input.autoPostRevenueRecognition}
                  onChange={(e) =>
                    setInput({ ...input, autoPostRevenueRecognition: e.target.checked })
                  }
                />
                Post revenue recognition to the ledger
              </label>
              <button
                type="submit"
                className="rounded bg-ds-primary px-4 py-2 text-sm text-ds-primary-foreground"
              >
                {saving ? 'Saving…' : 'Save posting settings'}
              </button>
            </fieldset>
            {error && (
              <p role="alert" className="text-sm text-ds-status-fail">
                {error}
              </p>
            )}
            {saved && (
              <p role="status" className="text-sm">
                Posting settings saved.
              </p>
            )}
          </form>
        </details>
      </CardContent>
    </Card>
  );
}
