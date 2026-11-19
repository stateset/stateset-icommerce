'use client';

import { Suspense, useEffect, useState } from 'react';
import { useSearchParams, useRouter } from 'next/navigation';
import Link from 'next/link';
import { PurchasedLicenses, type PurchasedLicense } from '@/components/commerce/PurchasedLicenses';
import { useCart } from '@/hooks/useCart';

function CardReturn() {
  const search = useSearchParams();
  const router = useRouter();
  const { clearCart } = useCart();
  const [licenses, setLicenses] = useState<PurchasedLicense[]>([]);
  const [offer, setOffer] = useState<{
    name: string;
    unitPrice: string;
    chargeTotal?: string;
    receipt: string;
  } | null>(null);
  const [membership, setMembership] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('Checking your payment…');
  const [attempt, retry] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function verify() {
      try {
        const query = new URLSearchParams({ session_id: search.get('session_id') || '' });
        const response = await fetch('/api/checkout/card?' + query, {
          cache: 'no-store',
          signal: controller.signal,
        });
        const result = await response.json();
        if (!response.ok) throw new Error(result.error || 'Payment confirmation is pending');
        setMembership(Boolean(result.renewingMembership));
        if (result.state === 'pending') {
          setMessage('Waiting for payment confirmation…');
          timer = setTimeout(verify, 3000);
          return;
        }
        if (result.cartId === localStorage.getItem('stateset_cart_id')) clearCart();
        if (result.state === 'expired') {
          setMessage('This checkout expired. Your reserved stock has been released.');
          return;
        }
        const receipt =
          '/checkout/confirmation?' +
          new URLSearchParams({ orderNumber: result.orderNumber, method: 'card' });
        setLicenses(result.licenses || []);
        if (result.upsell) {
          setOffer({ ...result.upsell, receipt });
          setMessage('Your order is confirmed.');
        } else if (result.licenses?.length) setMessage('Your purchase is ready.');
        else if (!result.renewingMembership) router.replace(receipt);
        else setMessage('Membership payment received.');
      } catch (cause) {
        if (!controller.signal.aborted)
          setMessage(cause instanceof Error ? cause.message : 'Payment confirmation is pending');
      }
    }
    void verify();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [search, router, clearCart, attempt]);
  return (
    <main className="mx-auto max-w-md space-y-4 px-4 py-12">
      <h1 className="text-2xl font-bold">Payment status</h1>
      <p role="status">{message}</p>
      <PurchasedLicenses licenses={licenses} />
      {membership && (
        <button
          type="button"
          className="underline"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              const response = await fetch('/api/checkout/card', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                  action: 'manage-membership',
                  sessionId: search.get('session_id'),
                }),
              });
              const body = await response.json();
              if (!response.ok)
                throw new Error(body.error || 'Unable to open membership management');
              window.location.assign(body.url);
            } catch (e) {
              setMessage(e instanceof Error ? e.message : 'Unable to open membership management');
              setBusy(false);
            }
          }}
        >
          Manage membership cancellation
        </button>
      )}
      {offer && (
        <section className="space-y-3">
          <h2 className="text-xl font-semibold">Add {offer.name}</h2>
          <p>
            {offer.chargeTotal
              ? 'USD ' +
                offer.chargeTotal +
                ' including shipping and tax, charged to your saved card.'
              : 'USD ' +
                offer.unitPrice +
                ' before shipping and tax. Review the total before paying.'}
          </p>
          <button
            disabled={busy}
            className="rounded bg-black px-4 py-2 text-white"
            onClick={async () => {
              setBusy(true);
              try {
                const response = await fetch('/api/offers', {
                  method: 'POST',
                  headers: { 'Content-Type': 'application/json' },
                  body: JSON.stringify({
                    sessionId: search.get('session_id'),
                    expectedTotal: offer.chargeTotal,
                  }),
                });
                const result = await response.json();
                if (!response.ok) throw new Error(result.error);
                window.location.assign(result.url);
              } catch (cause) {
                setMessage(cause instanceof Error ? cause.message : 'Offer is unavailable');
                setBusy(false);
              }
            }}
          >
            {offer.chargeTotal ? 'Add and pay USD ' + offer.chargeTotal : 'Review offer'}
          </button>
          <Link
            className="block underline"
            href={offer.receipt}
            onClick={() => {
              void fetch('/api/offers', {
                method: 'POST',
                keepalive: true,
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ sessionId: search.get('session_id'), action: 'skip' }),
              }).catch(() => {});
            }}
          >
            Skip and view receipt
          </Link>
        </section>
      )}
      {!offer && (
        <button className="underline" onClick={() => retry(attempt + 1)}>
          Check again
        </button>
      )}
      <Link className="block" href="/products">
        Continue shopping
      </Link>
    </main>
  );
}

export default function Page() {
  return (
    <Suspense>
      <CardReturn />
    </Suspense>
  );
}
