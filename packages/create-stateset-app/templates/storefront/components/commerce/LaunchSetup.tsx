'use client';

import { useState } from 'react';

type Method = {
  id: string;
  label: string;
  amount: string;
  countries?: string[];
  carrier?: string;
  estimatedDays?: string;
};
export type Pricing = { revision: number; rates: Record<string, string>; methods: Method[] };
async function operation(body: object) {
  const response = await fetch('/api/portal', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || 'Unable to complete this operation');
  return result;
}
const inputClass = 'block w-full rounded border p-2';

export function LaunchSetup({
  pricing,
  stripeConfigured,
  routingConfigured,
  testMode,
  refresh,
}: {
  pricing: Pricing;
  stripeConfigured: boolean;
  routingConfigured: boolean;
  testMode: boolean;
  refresh: () => Promise<void>;
}) {
  const [rates, setRates] = useState(
    Object.keys(pricing.rates).length ? Object.entries(pricing.rates) : [['', '']],
  );
  const [methods, setMethods] = useState<Method[]>(
    pricing.methods.length
      ? pricing.methods
      : [{ id: 'ground', label: 'Ground', amount: '', countries: ['US'] }],
  );
  const [pricingPreview, setPricingPreview] = useState(false);
  const [publishedProduct, setPublishedProduct] = useState<{ name: string; slug: string }>();
  const [catalogFormat, setCatalogFormat] = useState('stateset');
  const [sourceStore, setSourceStore] = useState('');
  const [review, setReview] = useState<{
    request: object;
    fingerprint?: string;
    warning?: string;
    skipped?: { id: string; reason: string }[];
    items?: {
      slug: string;
      name?: string;
      action: string;
      variants: { sku: string; price: string; openingStock: number }[];
    }[];
  }>();
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [routes, setRoutes] = useState<
    { account: string; provider: string; mode?: string; chargesEnabled?: boolean; error?: string }[]
  >([]);
  const [session, setSession] = useState('');
  const [hostedUrl, setHostedUrl] = useState('');
  async function run(fn: () => Promise<string>) {
    setBusy(true);
    setStatus('');
    try {
      setStatus(await fn());
    } catch (e) {
      setStatus(e instanceof Error ? e.message : 'Operation failed');
    } finally {
      setBusy(false);
    }
  }
  function saveRates(apply: boolean) {
    return run(async () => {
      if (new Set(rates.map(([state]) => state)).size !== rates.length)
        throw new Error('Use each state only once');
      await operation({
        pricing: { revision: pricing.revision, rates: Object.fromEntries(rates), methods },
        apply,
      });
      setPricingPreview(!apply);
      if (apply) await refresh();
      return apply
        ? 'Published checkout tax and shipping settings.'
        : 'Tax rates and shipping methods validated.';
    });
  }
  return (
    <section className="space-y-4">
      <h2 className="text-xl font-semibold">Store setup</h2>
      {pricing.revision > 0 && <p>Checkout pricing uses published revision {pricing.revision}.</p>}
      <details className="space-y-3 rounded border p-3">
        <summary>Add a product</summary>
        <form
          className="grid gap-3 sm:grid-cols-2"
          onSubmit={(event) => {
            event.preventDefault();
            const form = event.currentTarget;
            const fields = new FormData(form);
            const product = {
              name: String(fields.get('name')).trim(),
              slug: String(fields.get('slug')).trim(),
              sku: String(fields.get('sku')).trim(),
              price: String(fields.get('price')).trim(),
              stock: Number(fields.get('stock')),
            };
            void run(async () => {
              await operation({ catalog: [product], apply: true });
              setPublishedProduct({ name: product.name, slug: product.slug });
              form.reset();
              await refresh();
              return 'Product published.';
            });
          }}
        >
          <label>
            Product name
            <input className={inputClass} name="name" required />
          </label>
          <label>
            Product URL slug
            <input
              className={inputClass}
              name="slug"
              placeholder="coffee"
              pattern="[a-z0-9]+(-[a-z0-9]+)*"
              required
            />
          </label>
          <label>
            SKU
            <input
              className={inputClass}
              name="sku"
              placeholder="COFFEE"
              maxLength={100}
              required
            />
          </label>
          <label>
            Price (USD)
            <input
              className={inputClass}
              name="price"
              inputMode="decimal"
              placeholder="15.00"
              required
            />
          </label>
          <label>
            Opening stock
            <input className={inputClass} name="stock" type="number" min="0" step="1" required />
          </label>
          <button className="self-end rounded border p-2" disabled={busy}>
            Publish product
          </button>
        </form>
        {publishedProduct && (
          <a className="underline" href={'/products/' + publishedProduct.slug}>
            View {publishedProduct.name} in the storefront
          </a>
        )}
      </details>
      <details className="rounded border p-3">
        <summary>Import a product catalog</summary>
        <label className="block py-2">
          Catalog format
          <select
            className={inputClass}
            value={catalogFormat}
            disabled={busy}
            onChange={(e) => {
              setCatalogFormat(e.target.value);
              setReview(undefined);
              setStatus('');
            }}
          >
            <option value="stateset">StateSet catalog JSON</option>
            <option value="next">Next Commerce Products List JSON</option>
            <option value="whop">Whop products and variants JSON</option>
          </select>
        </label>
        {catalogFormat === 'next' && (
          <label className="block py-2">
            Source store code
            <input
              className={inputClass}
              placeholder="my-next-store"
              value={sourceStore}
              disabled={busy}
              onChange={(e) => {
                setSourceStore(e.target.value);
                setReview(undefined);
                setStatus('');
              }}
            />
          </label>
        )}
        <p>
          {catalogFormat === 'next'
            ? 'Upload one saved Admin API results page. Supported products use USD, one-time purchase, tracked physical stock at one location and zero allocated units.'
            : catalogFormat === 'whop'
              ? 'Upload the account ID with its saved Products and Variants API results. Eligible one-time USD offers are staged as drafts with zero stock.'
              : 'Each row requires name, slug, SKU in the sku field, USD price as a string, and initial stock.'}
        </p>
        <label className="block py-2">
          Catalog JSON file
          <input
            key={catalogFormat + '/' + sourceStore}
            type="file"
            accept=".json,application/json"
            disabled={busy || (catalogFormat === 'next' && !sourceStore)}
            onChange={(event) => {
              const file = event.target.files?.[0];
              setReview(undefined);
              if (file)
                void run(async () => {
                  if (file.size > 5_000_000) throw new Error('Choose a catalog under 5 MB');
                  const catalog = JSON.parse(await file.text());
                  const request =
                    catalogFormat === 'stateset'
                      ? { catalog }
                      : { action: 'import-catalog', format: catalogFormat, sourceStore, catalog };
                  const result = await operation({ ...request, apply: false });
                  setReview({
                    request,
                    fingerprint: result.fingerprint,
                    warning: result.warning,
                    items: result.items,
                    skipped: result.skipped,
                  });
                  const total = result.total ?? result.rows;
                  return total + (total === 1 ? ' product reviewed.' : ' products reviewed.');
                });
            }}
          />
        </label>
        {review?.warning && <p>{review.warning}</p>}
        {!!review?.skipped?.length && (
          <details>
            <summary>{review.skipped.length} excluded source records</summary>
            <ul>
              {review.skipped.map((entry) => (
                <li key={entry.id}>
                  {entry.id} · {entry.reason}
                </li>
              ))}
            </ul>
          </details>
        )}
        {review?.items && (
          <ul className="list-disc pl-5">
            {review.items.map((item) => (
              <li key={item.variants[0]?.sku || item.slug}>
                {item.name || item.slug} · {item.action}
                <ul className="pl-3">
                  {item.variants.map((v) => (
                    <li key={v.sku}>
                      {v.sku} · USD {v.price} · {v.openingStock} opening units
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        )}
        <button
          disabled={busy || !review}
          className="underline"
          onClick={() =>
            run(async () => {
              if (!review) throw new Error('Preview the catalog first.');
              const result = await operation({
                ...review.request,
                fingerprint: review.fingerprint,
                apply: true,
              });
              await refresh();
              setReview(undefined);
              return result.inserted + ' products added from ' + result.total + ' catalog rows.';
            })
          }
        >
          Import reviewed catalog
        </button>
      </details>
      <details className="space-y-3 rounded border p-3">
        <summary>Set tax and shipping</summary>
        <form
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            void saveRates(false);
          }}
        >
          <h3 className="font-semibold">US tax rates</h3>
          {rates.map(([state, rate], i) => (
            <div className="flex gap-3" key={i}>
              <label>
                State
                <input
                  required
                  maxLength={2}
                  value={state}
                  className={inputClass}
                  onChange={(e) => {
                    setPricingPreview(false);
                    setRates(
                      rates.map((r, j) => (j === i ? [e.target.value.toUpperCase(), rate] : r)),
                    );
                  }}
                />
              </label>
              <label>
                Decimal rate
                <input
                  required
                  placeholder="0.0725"
                  inputMode="decimal"
                  className={inputClass}
                  value={rate}
                  onChange={(e) => {
                    setPricingPreview(false);
                    setRates(rates.map((r, j) => (j === i ? [state, e.target.value] : r)));
                  }}
                />
              </label>
              {rates.length > 1 && (
                <button
                  type="button"
                  className="underline"
                  onClick={() => {
                    setPricingPreview(false);
                    setRates(rates.filter((_, j) => i !== j));
                  }}
                >
                  Remove state
                </button>
              )}
            </div>
          ))}
          <button
            type="button"
            className="underline"
            onClick={() => {
              setPricingPreview(false);
              setRates([...rates, ['', '']]);
            }}
          >
            Add state
          </button>
          <h3 className="font-semibold">Shipping methods</h3>
          {methods.map((method, i) => (
            <div key={i} className="flex flex-wrap gap-3">
              {(
                [
                  ['id', 'Method ID'],
                  ['label', 'Shipping label'],
                  ['amount', 'USD amount'],
                ] as const
              ).map(([key, label]) => (
                <label key={key}>
                  {label}
                  <input
                    required
                    value={method[key]}
                    className={inputClass}
                    onChange={(e) => {
                      setPricingPreview(false);
                      setMethods(
                        methods.map((m, j) => (j === i ? { ...m, [key]: e.target.value } : m)),
                      );
                    }}
                  />
                </label>
              ))}
              {methods.length > 1 && (
                <button
                  type="button"
                  className="underline"
                  onClick={() => {
                    setPricingPreview(false);
                    setMethods(methods.filter((_, j) => i !== j));
                  }}
                >
                  Remove method
                </button>
              )}
            </div>
          ))}
          <button
            type="button"
            className="underline"
            onClick={() => {
              setPricingPreview(false);
              setMethods([...methods, { id: '', label: '', amount: '', countries: ['US'] }]);
            }}
          >
            Add shipping method
          </button>
          <div className="flex gap-3">
            <button disabled={busy} className="rounded border p-2">
              Validate pricing
            </button>
            {pricingPreview && (
              <button
                type="button"
                disabled={busy}
                className="rounded border p-2"
                onClick={() => saveRates(true)}
              >
                Publish checkout pricing
              </button>
            )}
          </div>
        </form>
      </details>
      <details className="space-y-3 rounded border p-3">
        <summary>Verify processor and test a sale</summary>
        {routingConfigured && (
          <>
            <button
              disabled={busy}
              type="button"
              className="underline"
              onClick={() =>
                run(async () => {
                  setRoutes([]);
                  const result = await operation({ action: 'verify-routes' });
                  setRoutes(result.accounts);
                  return 'Processor account lookup finished.';
                })
              }
            >
              Verify routed processor accounts
            </button>
            {!!routes.length && (
              <ul>
                {routes.map((route) => (
                  <li key={route.account}>
                    {route.account} · {route.provider} ·{' '}
                    {route.error ||
                      route.mode +
                        ' · card payments ' +
                        (route.chargesEnabled ? 'enabled' : 'disabled')}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
        {stripeConfigured ? (
          <button
            disabled={busy}
            className="underline"
            onClick={() =>
              run(async () => {
                const r = await operation({ action: 'verify-processor' });
                return (
                  'Stripe account ' +
                  r.account +
                  ' · Charges ' +
                  (r.chargesEnabled ? 'enabled' : 'disabled') +
                  ' · Webhook ' +
                  (r.checkoutWebhookRegistered ? 'verified' : 'missing')
                );
              })
            }
          >
            Verify Stripe account and webhook
          </button>
        ) : (
          !routingConfigured && (
            <p>
              Configure the Stripe account and webhook keys on the server to verify the processor.
            </p>
          )
        )}
        {stripeConfigured && testMode && (
          <form
            className="grid gap-3 sm:grid-cols-2"
            onSubmit={(e) => {
              e.preventDefault();
              const form = new FormData(e.currentTarget);
              const verifyOnly =
                ((e.nativeEvent as SubmitEvent).submitter as HTMLButtonElement | null)?.value ===
                'verify';
              const get = (k: string) => String(form.get(k) || '');
              void run(async () => {
                const key = get('key');
                const finish = get('session');
                const address = {
                  firstName: get('firstName'),
                  lastName: get('lastName'),
                  line1: get('line1'),
                  city: get('city'),
                  state: get('state'),
                  postalCode: get('postalCode'),
                  country: 'US',
                };
                const drill = finish
                  ? { finish, key, refund: get('refund') }
                  : {
                      purchase: get('sku'),
                      key,
                      email: get('email'),
                      shipping: get('shipping'),
                      address,
                    };
                const r = await operation({
                  drill: { ...drill, wallet: get('wallet') },
                  apply: !verifyOnly,
                });
                if (r.testPurchase) {
                  setSession(r.testPurchase.sessionId);
                  setHostedUrl(r.testPurchase.url);
                  return r.testPurchase.expectedWallet
                    ? 'Open the checkout on an eligible device and pay with ' +
                        r.testPurchase.expectedWallet +
                        '.'
                    : 'Test checkout created. Complete payment, then finish the same drill.';
                }
                const walletEvidence = r.wallet
                  ? ' · ' + r.wallet.method + ' verified on charge ' + r.wallet.chargeId
                  : '';
                if (verifyOnly) return 'Provider payment ' + r.payment.status + walletEvidence;
                await refresh();
                return (
                  'Provider payment ' +
                  r.payment.status +
                  walletEvidence +
                  ' · Native shipment ' +
                  r.shipment.status +
                  ' · Refund ' +
                  r.refund.status +
                  ' (USD ' +
                  r.refund.amount +
                  ').'
                );
              });
            }}
          >
            {(
              [
                ['key', 'Drill ID'],
                ['sku', 'Test product SKU'],
                ['email', 'Test buyer email'],
                ['shipping', 'Shipping method ID'],
                ['firstName', 'First name'],
                ['lastName', 'Last name'],
                ['line1', 'Address'],
                ['city', 'City'],
                ['state', 'State'],
                ['postalCode', 'ZIP code'],
                ['refund', 'Partial refund in USD'],
              ] as const
            ).map(([key, label]) => (
              <label key={key}>
                {label}
                <input
                  className={inputClass}
                  name={key}
                  required={key === 'key' || (!session && key !== 'refund')}
                  defaultValue={
                    key === 'refund' ? '0.01' : key === 'shipping' ? methods[0]?.id : ''
                  }
                />
              </label>
            ))}
            <label>
              Wallet to verify
              <select name="wallet" className={inputClass} defaultValue="">
                <option value="">Use drill default</option>
                <option value="apple_pay">Apple Pay</option>
                <option value="google_pay">Google Pay</option>
              </select>
            </label>
            <label>
              Paid test session
              <input
                name="session"
                className={inputClass}
                value={session}
                onChange={(e) => setSession(e.target.value)}
              />
            </label>
            {hostedUrl && (
              <a href={hostedUrl} target="_blank" rel="noreferrer" className="underline">
                Open hosted test checkout
              </a>
            )}
            <button disabled={busy} className="rounded border p-2">
              {session ? 'Verify, ship and partially refund' : 'Create test checkout'}
            </button>
            {session && (
              <button value="verify" disabled={busy} className="rounded border p-2">
                Verify payment only
              </button>
            )}
          </form>
        )}
        {stripeConfigured && !testMode && (
          <p>
            Run the payment, shipment and partial refund drill in a test store using a Stripe test
            key.
          </p>
        )}
      </details>
      {status && <p role="status">{status}</p>}
    </section>
  );
}
