'use client';

import { useState } from 'react';
import { Card, CardContent, Badge, Button } from '@stateset/design';
import { ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGlPeriods, closeMonthDryRun, runCloseMonth } from '@/app/actions/finance';
import { formatMoney } from '@/lib/finance/format';
import type { CloseMonthReport, CloseMonthStep, GlPeriod } from '@/lib/embedded';

type DsBadgeVariant = 'default' | 'primary' | 'accent' | 'success' | 'warning' | 'danger' | 'outline';

const periodBadgeVariants: Record<string, DsBadgeVariant> = {
  open: 'primary',
  closed: 'success',
  locked: 'outline',
  future: 'default',
};

const stepBadgeVariants: Record<string, DsBadgeVariant> = {
  executed: 'success',
  dry_run: 'accent',
  skipped: 'outline',
};

const STEP_ROWS: { key: keyof Pick<CloseMonthReport, 'depreciation' | 'revenueRecognition' | 'fxRevaluation' | 'periodClose'>; label: string }[] = [
  { key: 'depreciation', label: 'Depreciation' },
  { key: 'revenueRecognition', label: 'Revenue recognition' },
  { key: 'fxRevaluation', label: 'FX revaluation' },
  { key: 'periodClose', label: 'Period close' },
];

function StepTable({ report }: { report: CloseMonthReport }) {
  return (
    <div className="overflow-x-auto">
      <table className="min-w-full text-sm">
        <thead>
          <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
            <th className="py-2 pr-4 font-medium">Step</th>
            <th className="py-2 pr-4 font-medium">Status</th>
            <th className="py-2 pr-4 text-right font-medium">Entries</th>
            <th className="py-2 pr-4 text-right font-medium">Total amount</th>
            <th className="py-2 font-medium">Warnings</th>
          </tr>
        </thead>
        <tbody>
          {STEP_ROWS.map(({ key, label }) => {
            const step: CloseMonthStep = report[key];
            return (
              <tr key={key} className="border-b border-ds-border/50 align-top">
                <td className="py-2 pr-4">{label}</td>
                <td className="py-2 pr-4">
                  <Badge variant={stepBadgeVariants[step.status] || 'default'}>
                    {step.status.replace('_', ' ')}
                  </Badge>
                </td>
                <td className="py-2 pr-4 text-right font-mono">{step.entryCount}</td>
                <td className="py-2 pr-4 text-right font-mono">{formatMoney(step.totalAmount)}</td>
                <td className="py-2">
                  {step.warnings.length === 0 ? (
                    <span className="text-ds-muted-foreground">—</span>
                  ) : (
                    <ul className="list-disc space-y-1 pl-4 text-ds-status-warn">
                      {step.warnings.map((warning) => (
                        <li key={warning}>{warning}</li>
                      ))}
                    </ul>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default function CloseClient() {
  const { data: periods, isLoading, error, refetch } = useEmbeddedData(() => getGlPeriods());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [report, setReport] = useState<CloseMonthReport | null>(null);
  const [confirmText, setConfirmText] = useState('');
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const selectPeriod = (id: string) => {
    setSelectedId(id);
    setReport(null);
    setConfirmText('');
    setActionError(null);
  };

  const runDryRun = async () => {
    if (!selectedId) return;
    setBusy(true);
    setActionError(null);
    try {
      setReport(await closeMonthDryRun(selectedId));
      setConfirmText('');
    } catch (err) {
      setActionError(err instanceof Error ? err.message : 'Dry run failed');
    } finally {
      setBusy(false);
    }
  };

  const runRealClose = async () => {
    if (!selectedId || !report || !report.dryRun) return;
    setBusy(true);
    setActionError(null);
    try {
      setReport(await runCloseMonth(selectedId));
      setConfirmText('');
      await refetch();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : 'Close failed');
    } finally {
      setBusy(false);
    }
  };

  if (isLoading && !periods) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4" data-testid="close-loading">
            <div className="h-6 w-48 rounded bg-ds-muted" />
            <div className="h-32 rounded bg-ds-muted" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !periods) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load accounting periods</p>
        </CardContent>
      </Card>
    );
  }

  const selectedPeriod: GlPeriod | undefined = periods.find((period) => period.id === selectedId);
  const canDryRun = Boolean(selectedPeriod && selectedPeriod.status === 'open' && !busy);
  const confirmMatches = Boolean(selectedPeriod && confirmText === selectedPeriod.periodName);
  const canRealClose = Boolean(
    selectedPeriod && report && report.dryRun && report.periodId === selectedPeriod.id && confirmMatches && !busy
  );

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">Month-End Close</h1>
        <p className="text-sm text-ds-muted-foreground">
          Dry-run the close to review every step before posting anything.
        </p>
      </div>

      <Card>
        <CardContent>
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
            Accounting Periods
          </h2>
          {periods.length === 0 ? (
            <p className="py-4 text-sm text-ds-muted-foreground">
              No accounting periods yet — create one with generalLedger.createPeriod().
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="min-w-full text-sm">
                <thead>
                  <tr className="border-b border-ds-border text-left text-ds-muted-foreground">
                    <th className="py-2 pr-4 font-medium">Select</th>
                    <th className="py-2 pr-4 font-medium">Period</th>
                    <th className="py-2 pr-4 font-medium">Range</th>
                    <th className="py-2 pr-4 font-medium">Status</th>
                    <th className="py-2 font-medium">Closed by</th>
                  </tr>
                </thead>
                <tbody>
                  {periods.map((period) => (
                    <tr key={period.id} className="border-b border-ds-border/50">
                      <td className="py-2 pr-4">
                        <input
                          type="radio"
                          name="close-period"
                          aria-label={`Select period ${period.periodName}`}
                          checked={selectedId === period.id}
                          disabled={period.status !== 'open'}
                          onChange={() => selectPeriod(period.id)}
                        />
                      </td>
                      <td className="py-2 pr-4 font-mono">{period.periodName}</td>
                      <td className="py-2 pr-4">
                        {period.startDate} → {period.endDate}
                      </td>
                      <td className="py-2 pr-4">
                        <Badge variant={periodBadgeVariants[period.status] || 'default'}>
                          {period.status}
                        </Badge>
                      </td>
                      <td className="py-2">{period.closedBy || '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <div className="mt-4 flex items-center gap-3">
            <Button onClick={runDryRun} disabled={!canDryRun}>
              {busy ? 'Working…' : 'Run dry run'}
            </Button>
            {selectedPeriod && (
              <span className="text-sm text-ds-muted-foreground">
                Selected: {selectedPeriod.periodName}
              </span>
            )}
          </div>
          {actionError && <p className="mt-3 text-sm text-ds-status-fail">{actionError}</p>}
        </CardContent>
      </Card>

      {report && (
        <Card>
          <CardContent>
            <div className="mb-3 flex flex-wrap items-center gap-3">
              <h2 className="text-sm font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
                Close report · {report.periodName}
              </h2>
              <Badge variant={report.dryRun ? 'accent' : 'success'}>
                {report.dryRun ? 'Dry run — nothing was written' : 'Close executed'}
              </Badge>
              <Badge variant={periodBadgeVariants[report.periodStatus] || 'default'}>
                period {report.periodStatus}
              </Badge>
            </div>
            <StepTable report={report} />

            {!report.dryRun && report.closingEntry && (
              <p className="mt-3 text-sm text-ds-muted-foreground">
                Closing entry {report.closingEntry.entryNumber} posted on{' '}
                {report.closingEntry.entryDate}.
              </p>
            )}

            {report.dryRun && selectedPeriod && (
              <div className="mt-6 rounded-lg border border-ds-status-warn/40 bg-ds-status-warn/10 p-4">
                <div className="flex items-center gap-2">
                  <ExclamationTriangleIcon
                    className="h-5 w-5 text-ds-status-warn"
                    aria-hidden="true"
                  />
                  <p className="text-sm font-semibold">
                    Run the real close for {selectedPeriod.periodName}?
                  </p>
                </div>
                <p className="mt-1 text-sm text-ds-muted-foreground">
                  This posts closing entries and closes the period. It cannot be undone from the
                  admin. Type <span className="font-mono">{selectedPeriod.periodName}</span> to
                  confirm.
                </p>
                <div className="mt-3 flex items-center gap-3">
                  <input
                    type="text"
                    aria-label="Type the period name to confirm close"
                    placeholder={selectedPeriod.periodName}
                    value={confirmText}
                    onChange={(event) => setConfirmText(event.target.value)}
                    className="rounded-md border border-ds-border bg-ds-background px-2 py-1.5 font-mono text-sm"
                  />
                  <Button variant="danger" onClick={runRealClose} disabled={!canRealClose}>
                    {busy ? 'Closing…' : 'Run close'}
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
