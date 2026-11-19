'use client';

import { useEffect, useState } from 'react';
import { AffiliatePayouts, type AffiliatePayout } from '@/components/commerce/AffiliatePayouts';
import { CampaignEditor, type Campaign } from '@/components/commerce/CampaignEditor';
import { LaunchSetup, type Pricing } from '@/components/commerce/LaunchSetup';

type Row = Record<string, string | number>;
type BankPayout = {
  id: string;
  currency: string;
  amountMinor: string;
  status: string;
  reconciliation?: { netMinor: string; differenceMinor: string; transactions: Row[] };
};
interface Statement extends Record<string, unknown> {
  seller: string;
  currency: string;
  payable: string;
  transferred: string;
  outstanding: string;
  orders: Row[];
  bankPayouts: BankPayout[];
}
interface Report {
  role: string;
  subject: string;
  balance?: Row;
  orders?: Row[];
  referrals?: { campaign: string; url: string }[];
  catalog?: Campaign[];
  payouts?: AffiliatePayout[];
  payoutAccountConfigured?: boolean;
  recoveryEnabled?: boolean;
  renewalReceipts?: Row[];
  launch?: Row[];
  pricing?: Pricing;
  testMode?: boolean;
  stripeConfigured?: boolean;
  routingConfigured?: boolean;
  drills?: Row[];
  integrations?: { name: string; configured: boolean; installedAt: string | null }[];
  campaignOrders?: {
    orderId: string;
    number: string;
    campaign: string;
    revision: number;
    variant: string;
    source: string;
    charged: string;
    net: string;
    refunds: { id: string; amount: string }[];
  }[];
  statements?: Statement[];
  sellers?: Statement[];
  campaigns?: Row[];
  affiliates?: Row[];
  deliveries?: Row[];
}
function Records({
  title,
  columns,
  rows = [],
}: {
  title: string;
  columns: [string, string][];
  rows?: Row[];
}) {
  return (
    <section className="space-y-2">
      <h2 className="text-lg font-semibold">{title}</h2>
      {rows.length ? (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr>
                {columns.map(([key, name]) => (
                  <th className="p-2 text-left" key={key} scope="col">
                    {name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={index} className="border-t">
                  {columns.map(([key]) => (
                    <td className="p-2" key={key}>
                      {String(row[key] ?? '–')}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p>No records yet.</p>
      )}
    </section>
  );
}
export default function Portal() {
  const [report, setReport] = useState<Report>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [deliveryNotice, setDeliveryNotice] = useState('');
  const [deliveryHistory, setDeliveryHistory] = useState<{
    sourceId: string;
    provider: string;
    history: Row[];
  }>();
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [control, setControl] = useState('');
  const [cohort, setCohort] = useState('');
  const [renewals, setRenewals] = useState<{
    invoices: Row[];
    recoveryPercent: Record<string, string>;
  }>();
  const [verification, setVerification] = useState<{
    enabled: boolean;
    required: string[];
    pastDue: string[];
    reason?: string;
  }>();
  async function refresh() {
    const response = await fetch('/api/portal?' + new URLSearchParams({ from, to, control }), {
      cache: 'no-store',
    });
    if (!response.ok) throw new Error('Open a current portal link from your merchant.');
    setReport(await response.json());
  }
  async function checkDelivery(row: Row, action: 'check' | 'retry' | 'release' | 'history') {
    if (
      action === 'release' &&
      !window.confirm(
        'Retry the missing ' +
          row.provider +
          ' request? If the original arrives late, this can duplicate a ticket or update an existing shipping order.',
      )
    )
      return;
    setBusy(true);
    setError('');
    setDeliveryNotice('');
    try {
      if (action === 'history') {
        const res = await fetch(
          '/api/portal?' + new URLSearchParams({ view: 'delivery', key: String(row.key) }),
          { cache: 'no-store' },
        );
        if (!res.ok) throw new Error('Unable to load delivery history');
        setDeliveryHistory(await res.json());
        return;
      }
      const res = await fetch('/api/portal', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          delivery: row.key,
          check: action === 'check',
          ...(action === 'release' && { retryClaim: row.claimId }),
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Unable to check the delivery');
      const current = data[0];
      setDeliveryHistory(current);
      setDeliveryNotice(
        current.provider +
          ' · ' +
          current.status +
          ' · ' +
          (current.externalId || current.message || current.sourceId),
      );
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to check the delivery');
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    async function open() {
      const token = window.location.hash.slice(1);
      if (token) {
        window.history.replaceState(null, '', window.location.pathname);
        const response = await fetch('/api/portal', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token }),
        });
        if (!response.ok) throw new Error('This portal link has expired or was revoked.');
      }
      await refresh();
    }
    // A portal invitation may open on the same path as another account.
    // Start a fresh document so its requests and form state belong to that grant.
    function followInvitation() {
      if (window.location.hash) window.location.reload();
    }
    window.addEventListener('hashchange', followInvitation);
    void open().catch((e) => setError(e.message));
    return () => window.removeEventListener('hashchange', followInvitation);
  }, []);
  const balances: [string, string][] = [
    ['currency', 'Currency'],
    ['earned', 'Earned'],
    ['paid', 'Paid or transferred'],
    ['reserved', 'In payout batches'],
    ['available', 'Available'],
  ];
  const cohortKey = (row: {
    campaign?: unknown;
    revision?: unknown;
    variant?: unknown;
    source?: unknown;
  }) => JSON.stringify([row.campaign, row.revision, row.variant, row.source || '']);
  const campaignRows = report?.campaigns?.filter((row) => !cohort || cohortKey(row) === cohort);
  const campaignOrders = report?.campaignOrders?.filter(
    (row) => !cohort || cohortKey(row) === cohort,
  );
  return (
    <main className="mx-auto max-w-5xl space-y-7 p-6">
      <h1 className="text-2xl font-bold">
        {report ? report.subject + ' · ' + report.role : 'Merchant portal'}
      </h1>
      {error && <p role="alert">{error}</p>}
      {!report && !error && <p role="status">Loading account…</p>}
      {report?.pricing && (
        <LaunchSetup
          key={report.pricing.revision}
          pricing={report.pricing}
          testMode={Boolean(report.testMode)}
          stripeConfigured={Boolean(report.stripeConfigured)}
          routingConfigured={Boolean(report.routingConfigured)}
          refresh={refresh}
        />
      )}
      {!!report?.drills?.length && (
        <Records
          title="Verified test sales"
          columns={[
            ['drill', 'Drill'],
            ['orderId', 'Order'],
            ['shipmentId', 'Shipment'],
            ['refundId', 'Refund'],
            ['wallet', 'Verified wallet'],
            ['chargeId', 'Wallet charge'],
          ]}
          rows={report.drills}
        />
      )}
      {report?.catalog && <CampaignEditor catalog={report.catalog} onPublish={refresh} />}
      {report?.launch && (
        <Records
          title="Launch readiness"
          columns={[
            ['id', 'Check'],
            ['status', 'Status'],
            ['message', 'Result'],
            ['action', 'Next step'],
          ]}
          rows={report.launch}
        />
      )}
      {report?.integrations && (
        <section className="space-y-2">
          <h2 className="text-lg font-semibold">Integrations</h2>
          {report.integrations.map((item) => (
            <div key={item.name}>
              {item.name} ·{' '}
              {item.installedAt
                ? 'Connected'
                : item.configured
                  ? 'Ready to connect'
                  : 'Add provider credentials on the server'}
              {item.configured && (
                <button
                  disabled={busy}
                  className="ml-3 underline"
                  onClick={async () => {
                    setBusy(true);
                    setError('');
                    try {
                      const response = await fetch('/api/portal', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ integration: item.name, apply: true }),
                      });
                      if (!response.ok)
                        throw new Error(
                          (await response.json()).error || 'Unable to verify this integration',
                        );
                      await refresh();
                    } catch (e) {
                      setError(e instanceof Error ? e.message : 'Connection failed');
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  {item.installedAt ? 'Verify connection' : 'Verify and install'}
                </button>
              )}
            </div>
          ))}
        </section>
      )}
      {!!report?.renewalReceipts?.length && (
        <Records
          title="Membership renewal payments"
          rows={report.renewalReceipts}
          columns={[
            ['invoice', 'Stripe invoice'],
            ['subscription', 'Membership'],
            ['payment', 'StateSet payment'],
            ['periodStart', 'Period start'],
            ['periodEnd', 'Period end'],
            ['currency', 'Currency'],
            ['collected', 'Collected'],
            ['tax', 'Tax included'],
            ['refunded', 'Refunded'],
            ['netCollected', 'Net collected'],
            ['sellerAllocation', 'Seller allocation'],
          ]}
        />
      )}
      {report?.recoveryEnabled && (
        <section className="space-y-2">
          <button
            className="underline"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setError('');
              try {
                const response = await fetch('/api/portal?view=renewals', { cache: 'no-store' });
                if (!response.ok) throw new Error('Unable to retrieve renewal invoices');
                setRenewals(await response.json());
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Unable to load renewals');
              } finally {
                setBusy(false);
              }
            }}
          >
            Load renewal recovery report
          </button>
          {renewals && (
            <>
              {Object.entries(renewals.recoveryPercent).map(([ccy, rate]) => (
                <p key={ccy}>
                  {ccy.toUpperCase()} · {rate}% of failed renewal revenue recovered
                </p>
              ))}
              <Records
                title="Renewal invoices"
                rows={renewals.invoices}
                columns={[
                  ['id', 'Invoice'],
                  ['subscriptionId', 'Native subscription'],
                  ['status', 'Status'],
                  ['currency', 'Currency'],
                  ['failedMinor', 'Failed (minor units)'],
                  ['recoveredMinor', 'Recovered (minor units)'],
                  ['nextRetry', 'Next scheduled retry'],
                ]}
              />
            </>
          )}
        </section>
      )}
      {report?.balance && (
        <Records title="Commission balance" columns={balances} rows={[report.balance]} />
      )}
      {report?.referrals && (
        <section>
          <h2 className="font-semibold">Referral links</h2>
          {report.referrals.map((link) => (
            <p key={link.campaign}>
              {link.campaign} ·{' '}
              <a className="underline" href={link.url}>
                {link.url}
              </a>
            </p>
          ))}
        </section>
      )}
      {report?.orders && (
        <Records
          title="Referred orders"
          columns={[
            ['orderId', 'Order'],
            ['original', 'Commission'],
            ['reversal', 'Refund reversal'],
            ['earned', 'Earned'],
          ]}
          rows={report.orders}
        />
      )}
      {report?.payouts && (
        <AffiliatePayouts
          payouts={report.payouts}
          merchant={report.role === 'merchant'}
          refresh={refresh}
        />
      )}
      {(report?.role === 'seller' || report?.payoutAccountConfigured) && (
        <section className="space-y-2">
          <button
            className="underline"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setError('');
              try {
                const response = await fetch('/api/portal?view=verification', {
                  cache: 'no-store',
                });
                if (!response.ok) throw new Error('Unable to load provider verification status');
                setVerification(await response.json());
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Unable to load verification');
              } finally {
                setBusy(false);
              }
            }}
          >
            Check payout account verification
          </button>
          {verification && (
            <>
              <p>
                Bank payouts {verification.enabled ? 'enabled' : 'disabled'}
                {verification.reason ? ' · ' + verification.reason : ''}
              </p>
              {verification.required.length > 0 && (
                <p>Required · {verification.required.join(', ')}</p>
              )}
              {verification.pastDue.length > 0 && (
                <p>Past due · {verification.pastDue.join(', ')}</p>
              )}
            </>
          )}
        </section>
      )}
      {report?.campaigns && (
        <>
          <form
            className="flex flex-wrap gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              setCohort('');
              void refresh().catch((e) => setError(e.message));
            }}
          >
            <label>
              First visit from (UTC){' '}
              <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
            </label>
            <label>
              Through{' '}
              <input type="date" min={from} value={to} onChange={(e) => setTo(e.target.value)} />
            </label>
            <button className="underline">Filter campaign results</button>
            <label>
              Compare with variant{' '}
              <input
                className="rounded border px-2"
                value={control}
                placeholder="First variant by ID"
                onChange={(e) => setControl(e.target.value)}
              />
            </label>
          </form>
          <label>
            Drill down by variant and source{' '}
            <select
              className="rounded border p-1"
              value={cohort}
              onChange={(e) => setCohort(e.target.value)}
            >
              <option value="">All campaign results</option>
              {report.campaigns.map((row) => (
                <option key={cohortKey(row)} value={cohortKey(row)}>
                  {row.campaign} · revision {row.revision} · {row.variant} ·{' '}
                  {row.source || 'direct'}
                </option>
              ))}
            </select>
          </label>
          <Records
            title="Campaign results"
            columns={[
              ['campaign', 'Campaign'],
              ['revision', 'Revision'],
              ['variant', 'Variant'],
              ['source', 'Source'],
              ['visitors', 'Visitors'],
              ['trackedCheckoutVisitors', 'Reached checkout'],
              ['paymentVisitors', 'Started payment'],
              ['purchasingVisitors', 'Bought'],
              ['bumps', 'Add-ons'],
              ['upsells', 'Upsells'],
              ['trackedNetPerVisitor', 'Net USD per tracked visitor'],
            ]}
            rows={campaignRows}
          />
          <Records
            title="Visitors who stopped at each stage"
            rows={campaignRows}
            columns={[
              ['campaign', 'Campaign'],
              ['revision', 'Revision'],
              ['variant', 'Variant'],
              ['source', 'Source'],
              ['beforeCheckout', 'Before checkout'],
              ['checkoutWithoutPayment', 'Checkout without payment start'],
              ['paymentWithoutPurchase', 'Payment start without purchase'],
            ]}
          />
          <Records
            title="Net revenue per visitor by comparison variant"
            rows={campaignRows}
            columns={[
              ['campaign', 'Campaign'],
              ['revision', 'Revision'],
              ['variant', 'Variant'],
              ['source', 'Source'],
              ['visitors', 'Visitors'],
              ['comparisonVariant', 'Compared with'],
              ['trackedNetPerVisitor', 'Net USD per tracked visitor'],
              ['netRevenuePerVisitorDifference', 'Difference (USD)'],
              ['netRevenuePerVisitorChangePercent', 'Difference (%)'],
            ]}
          />
        </>
      )}
      {!!campaignOrders?.length && (
        <section>
          <h2 className="text-lg font-semibold">Attributed orders and refunds</h2>
          {campaignOrders.map((order) => (
            <details key={order.orderId} className="border-b py-2">
              <summary>
                {order.number} · {order.campaign} / {order.variant} · Net USD {order.net}
              </summary>
              <p>
                {order.orderId} · Source {order.source || 'direct'} · Charged USD {order.charged}
              </p>
              {order.refunds.map((r) => (
                <p key={r.id}>
                  Refund {r.id} · USD {r.amount}
                </p>
              ))}
            </details>
          ))}
        </section>
      )}
      {report?.affiliates && (
        <Records
          title="Affiliate balances"
          columns={[['affiliate', 'Affiliate'], ...balances]}
          rows={report.affiliates}
        />
      )}
      {(report?.statements || report?.sellers || []).map((s) => (
        <section key={s.seller} className="space-y-4">
          <h2 className="text-xl font-semibold">Seller · {s.seller}</h2>
          <p>
            {s.currency} {s.payable} earned after refunds · {s.transferred} transferred ·{' '}
            {s.outstanding} outstanding
          </p>
          <Records
            title="Seller orders"
            columns={[
              ['orderId', 'Order'],
              ['invoiceId', 'Renewal invoice'],
              ['gross', 'Merchandise'],
              ['fee', 'Fees'],
              ['returnedFees', 'Fees refunded'],
              ['refundDeductions', 'Refunds'],
              ['outstanding', 'Outstanding'],
            ]}
            rows={s.orders}
          />
          <Records
            title="Bank payouts"
            columns={[
              ['id', 'Payout'],
              ['currency', 'Currency'],
              ['amountMinor', 'Minor units'],
              ['status', 'Bank status'],
            ]}
            rows={s.bankPayouts.map(({ reconciliation, ...row }) => row)}
          />
          {s.bankPayouts.map(
            (p) =>
              p.reconciliation && (
                <details key={p.id}>
                  <summary>Reconcile bank payout {p.id}</summary>
                  <p>
                    Difference in {p.currency.toUpperCase()} minor units ·{' '}
                    {p.reconciliation.differenceMinor}
                  </p>
                  <Records
                    title="Bank payout transactions"
                    columns={[
                      ['id', 'Balance transaction'],
                      ['orderId', 'Order'],
                      ['invoiceId', 'Renewal invoice'],
                      ['type', 'Type'],
                      ['amountMinor', 'Gross'],
                      ['feeMinor', 'Fees'],
                      ['netMinor', 'Net'],
                    ]}
                    rows={p.reconciliation.transactions}
                  />
                </details>
              ),
          )}
        </section>
      ))}
      {report?.deliveries && (
        <>
          <Records
            title="Integration deliveries"
            columns={[
              ['provider', 'Provider'],
              ['event', 'Event'],
              ['sourceId', 'Order or refund'],
              ['externalId', 'Provider ID'],
              ['confirmedAt', 'Confirmed at'],
              ['status', 'Delivery'],
              ['message', 'Detail'],
            ]}
            rows={report.deliveries}
          />
          {deliveryNotice && <p role="status">{deliveryNotice}</p>}
          {report.deliveries.map((row) => (
            <div key={row.key} className="flex flex-wrap gap-4">
              <button
                disabled={busy}
                className="underline"
                onClick={() => checkDelivery(row, 'history')}
              >
                {row.provider} history for {row.sourceId}
              </button>
              {row.status !== 'skipped' && (
                <button
                  disabled={busy}
                  className="underline"
                  onClick={() => checkDelivery(row, 'check')}
                >
                  Check {row.provider} for {row.sourceId}
                </button>
              )}
              {['pending', 'failed'].includes(String(row.status)) && (
                <button
                  disabled={busy}
                  className="underline"
                  onClick={() => checkDelivery(row, 'retry')}
                >
                  Send {row.provider} delivery for {row.sourceId}
                </button>
              )}
              {row.status === 'review' && row.claimId && (
                <button
                  disabled={busy}
                  className="underline"
                  onClick={() => checkDelivery(row, 'release')}
                >
                  Retry missing {row.provider} delivery for {row.sourceId}
                </button>
              )}
            </div>
          ))}
          {deliveryHistory && (
            <Records
              title={deliveryHistory.provider + ' attempts for ' + deliveryHistory.sourceId}
              rows={deliveryHistory.history}
              columns={[
                ['at', 'Time'],
                ['action', 'Action'],
                ['status', 'Result'],
                ['externalId', 'Provider ID'],
                ['message', 'Detail'],
              ]}
            />
          )}
        </>
      )}
    </main>
  );
}
