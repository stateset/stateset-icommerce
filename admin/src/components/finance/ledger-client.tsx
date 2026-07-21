'use client';

import { useCallback, useEffect, useState } from 'react';
import { Card, CardContent, Badge } from '@stateset/design';
import { ScaleIcon } from '@heroicons/react/24/outline';
import { getLedgerPageData } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';
import type { GlAccount, JournalEntry, TrialBalance } from '@/lib/embedded';

interface LedgerData {
  accounts: GlAccount[];
  trialBalance: TrialBalance;
  journalEntries: JournalEntry[];
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export default function LedgerClient() {
  const [asOfDate, setAsOfDate] = useState(todayIso);
  const [data, setData] = useState<LedgerData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (date: string) => {
    setIsLoading(true);
    setError(null);
    try {
      setData(await getLedgerPageData(date));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load ledger');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(asOfDate);
  }, [asOfDate, load]);

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="ledger-loading">
            <div className="h-6 w-48 rounded bg-ds-muted" />
            <div className="h-32 rounded bg-ds-muted" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load general ledger</p>
        </CardContent>
      </Card>
    );
  }

  const { accounts, trialBalance, journalEntries } = data;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">General Ledger</h1>
          <p className="text-sm text-ds-muted-foreground">
            Chart of accounts and trial balance from the embedded engine
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <span className="text-ds-muted-foreground">Trial balance as of</span>
          <input
            type="date"
            aria-label="Trial balance as-of date"
            value={asOfDate}
            max={todayIso()}
            onChange={(event) => setAsOfDate(event.target.value)}
            className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 text-sm"
          />
        </label>
      </div>

      <Card>
        <CardContent>
          <div className="flex flex-wrap items-center gap-6">
            <div className="flex items-center gap-2">
              <ScaleIcon className="h-5 w-5 text-ds-muted-foreground" aria-hidden="true" />
              <span className="text-sm font-medium">Trial Balance · {trialBalance.asOfDate}</span>
            </div>
            <div className="text-sm">
              <span className="text-ds-muted-foreground">Debits </span>
              <span className="font-mono">{formatMoney(trialBalance.totalDebits)}</span>
            </div>
            <div className="text-sm">
              <span className="text-ds-muted-foreground">Credits </span>
              <span className="font-mono">{formatMoney(trialBalance.totalCredits)}</span>
            </div>
            {trialBalance.isBalanced ? (
              <Badge variant="success">Balanced</Badge>
            ) : (
              <Badge variant="danger">Out of balance</Badge>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Chart of Accounts ({accounts.length})
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Account</th>
                  <th className="py-2 pr-4 font-medium">Name</th>
                  <th className="py-2 pr-4 font-medium">Type</th>
                  <th className="py-2 pr-4 font-medium">Status</th>
                  <th className="py-2 text-right font-medium">Balance</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((account) => (
                  <tr key={account.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{account.accountNumber}</td>
                    <td className="py-2 pr-4">{account.name}</td>
                    <td className="py-2 pr-4 capitalize">{account.accountType}</td>
                    <td className="py-2 pr-4 capitalize">{account.status}</td>
                    <td className="py-2 text-right font-mono">{formatMoney(account.balance)}</td>
                  </tr>
                ))}
                {accounts.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-ds-muted-foreground">
                      No GL accounts yet — initialize the chart of accounts from the engine.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Recent Journal Entries
          </h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                  <th className="py-2 pr-4 font-medium">Entry</th>
                  <th className="py-2 pr-4 font-medium">Date</th>
                  <th className="py-2 pr-4 font-medium">Description</th>
                  <th className="py-2 font-medium">Status</th>
                </tr>
              </thead>
              <tbody>
                {journalEntries.slice(0, 15).map((entry) => (
                  <tr key={entry.id} className="border-b border-ds-border/50">
                    <td className="py-2 pr-4 font-mono">{entry.entryNumber}</td>
                    <td className="py-2 pr-4">{entry.entryDate}</td>
                    <td className="py-2 pr-4">{entry.description}</td>
                    <td className="py-2">
                      <Badge variant={entry.status === 'posted' ? 'success' : 'outline'}>
                        {entry.status}
                      </Badge>
                    </td>
                  </tr>
                ))}
                {journalEntries.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-6 text-center text-ds-muted-foreground">
                      No journal entries recorded yet.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
