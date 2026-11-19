'use client';

import { useEffect, useRef, useState } from 'react';
import Script from 'next/script';
import Link from 'next/link';
import { useCart } from '@/hooks/useCart';
import { PurchasedLicenses, type PurchasedLicense } from '@/components/commerce/PurchasedLicenses';

type Account = {
  account: string;
  provider: 'stripe' | 'square';
  publicKey: string;
  location?: string;
  sandbox?: boolean;
};
type Checkout = {
  state: string;
  amount: string;
  currency: string;
  orderNumber: string;
  cartId?: string;
  account?: Account;
  offer?: { name: string; unitPrice: string };
  url?: string;
  clientSecret?: string;
  licenses?: PurchasedLicense[];
  contact?: { email: string; givenName: string; familyName: string; countryCode: string };
};
type StripeCard = { mount(target: HTMLDivElement): void; destroy(): void };
type StripeClient = {
  elements(): { create(type: 'card'): StripeCard };
  createPaymentMethod(input: {
    type: 'card';
    card: StripeCard;
  }): Promise<{ paymentMethod?: { id: string }; error?: { message: string } }>;
  handleNextAction(input: { clientSecret: string }): Promise<{ error?: { message: string } }>;
};
type SquareCard = {
  attach(target: HTMLDivElement): Promise<void>;
  destroy(): Promise<void>;
  tokenize(
    details: object,
  ): Promise<{ status: string; token?: string; errors?: { message: string }[] }>;
};
type ProcessorWindow = Window & {
  Stripe?: (key: string) => StripeClient;
  Square?: { payments(application: string, location: string): { card(): Promise<SquareCard> } };
};

async function advance(capability: string, body: object = {}): Promise<Checkout> {
  const response = await fetch('/api/checkout/routed', {
    method: 'POST',
    cache: 'no-store',
    headers: { Authorization: 'Bearer ' + capability, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || 'Unable to confirm this purchase');
  return result;
}

function CardFields({
  checkout,
  capability,
  onResult,
}: {
  checkout: Checkout;
  capability: string;
  onResult: (value: Checkout) => void;
}) {
  const account = checkout.account!;
  const container = useRef<HTMLDivElement>(null);
  const tokenize = useRef<(() => Promise<string>) | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const pendingToken = useRef<string | undefined>(undefined);
  const script =
    account.provider === 'stripe'
      ? 'https://js.stripe.com/v3/'
      : account.sandbox
        ? 'https://sandbox.web.squarecdn.com/v1/square.js'
        : 'https://web.squarecdn.com/v1/square.js';
  useEffect(() => {
    if (!loaded || !container.current) return;
    const host = window as ProcessorWindow;
    let active = true;
    let dispose: (() => void) | undefined;
    const node = container.current;
    async function attach() {
      if (account.provider === 'stripe') {
        const stripe = host.Stripe!(account.publicKey);
        const card = stripe.elements().create('card');
        card.mount(node);
        dispose = () => card.destroy();
        tokenize.current = async () => {
          const result = await stripe.createPaymentMethod({ type: 'card', card });
          if (!result.paymentMethod)
            throw new Error(result.error?.message || 'Check your card details');
          return result.paymentMethod.id;
        };
      } else {
        const card = await host.Square!.payments(account.publicKey, account.location!).card();
        if (!active) {
          await card.destroy();
          return;
        }
        await card.attach(node);
        if (!active) {
          await card.destroy();
          return;
        }
        dispose = () => {
          void card.destroy();
        };
        tokenize.current = async () => {
          const result = await card.tokenize({
            amount: checkout.amount,
            currencyCode: checkout.currency,
            intent: 'CHARGE',
            customerInitiated: true,
            sellerKeyedIn: false,
            billingContact: checkout.contact,
          });
          if (result.status !== 'OK' || !result.token)
            throw new Error(result.errors?.[0]?.message || 'Check your card details');
          return result.token;
        };
      }
      if (active) setReady(true);
    }
    void attach().catch(() => {
      if (active) setError('Card fields could not load. Reload this checkout to try again.');
    });
    return () => {
      active = false;
      tokenize.current = null;
      dispose?.();
    };
  }, [
    loaded,
    account.account,
    account.provider,
    account.publicKey,
    account.location,
    checkout.amount,
    checkout.currency,
    checkout.contact,
  ]);
  return (
    <section className="space-y-4">
      <Script
        src={script}
        onReady={() => setLoaded(true)}
        onError={() => setError('The payment provider could not load.')}
      />
      <div ref={container} aria-label="Secure card details" />
      <button
        disabled={!ready || busy}
        className="w-full rounded bg-blue-600 p-3 font-medium text-white disabled:opacity-50"
        onClick={async () => {
          setBusy(true);
          setError('');
          try {
            pendingToken.current ??= await tokenize.current!();
            const result = await advance(capability, {
              account: account.account,
              token: pendingToken.current,
            });
            pendingToken.current = undefined;
            onResult(result);
          } catch (err) {
            setError(err instanceof Error ? err.message : 'Unable to confirm payment');
          } finally {
            setBusy(false);
          }
        }}
      >
        {busy ? 'Confirming payment…' : 'Pay ' + checkout.currency + ' ' + checkout.amount}
      </button>
      {error && <p role="alert">{error}</p>}
    </section>
  );
}

export default function RoutedCardPayment() {
  const [capability, setCapability] = useState('');
  const [checkout, setCheckout] = useState<Checkout | null>(null);
  const [error, setError] = useState('');
  const [authReady, setAuthReady] = useState(false);
  const [checking, setChecking] = useState(false);
  const { clearCart } = useCart();
  useEffect(() => {
    const change = () => {
      setCheckout(null);
      setError('');
      setAuthReady(false);
      setCapability(window.location.hash.slice(1));
    };
    change();
    window.addEventListener('hashchange', change);
    return () => window.removeEventListener('hashchange', change);
  }, []);
  async function check(key: string) {
    setChecking(true);
    try {
      const result = await advance(key);
      if (window.location.hash.slice(1) === key) {
        setCheckout(result);
        setError('');
      }
    } catch (e) {
      if (window.location.hash.slice(1) === key)
        setError(e instanceof Error ? e.message : 'Unable to load checkout');
    } finally {
      setChecking(false);
    }
  }
  useEffect(() => {
    if (capability) void check(capability);
  }, [capability]);
  useEffect(() => {
    if (!capability || checkout?.state !== 'pending') return;
    const timer = setTimeout(() => {
      void check(capability);
    }, 4000);
    return () => clearTimeout(timer);
  }, [checkout, capability]);
  useEffect(() => {
    if (checkout?.state === 'paid' && checkout.cartId === localStorage.getItem('stateset_cart_id'))
      clearCart();
  }, [checkout, clearCart]);
  return (
    <main className="mx-auto max-w-md space-y-5 px-4 py-12">
      <h1 className="text-2xl font-bold">
        {checkout?.state === 'paid' ? 'Payment received' : 'Card payment'}
      </h1>
      {checkout && (
        <p>
          Order {checkout.orderNumber} · {checkout.currency} {checkout.amount}
        </p>
      )}
      {checkout?.state === 'collect_token' && (
        <CardFields
          key={checkout.account!.account}
          checkout={checkout}
          capability={capability}
          onResult={setCheckout}
        />
      )}
      {checkout?.state === 'requires_action' &&
        checkout.account?.provider === 'stripe' &&
        checkout.clientSecret && (
          <>
            <Script src="https://js.stripe.com/v3/" onReady={() => setAuthReady(true)} />
            <button
              disabled={!authReady || checking}
              className="underline"
              onClick={async () => {
                setChecking(true);
                try {
                  const result = await (window as ProcessorWindow).Stripe!(
                    checkout.account!.publicKey,
                  ).handleNextAction({ clientSecret: checkout.clientSecret! });
                  if (result.error) throw new Error(result.error.message);
                  await check(capability);
                } catch (err) {
                  setError(err instanceof Error ? err.message : 'Verification failed');
                } finally {
                  setChecking(false);
                }
              }}
            >
              Verify payment with your bank
            </button>
          </>
        )}
      {checkout?.state === 'paid' && (
        <>
          <PurchasedLicenses licenses={checkout.licenses || []} />
          {checkout.offer && (
            <section className="space-y-2">
              <h2 className="font-semibold">Add {checkout.offer.name}</h2>
              <p>
                USD {checkout.offer.unitPrice} before shipping and tax. Review the total before
                paying.
              </p>
              <button
                disabled={checking}
                className="rounded bg-black px-3 py-2 text-white"
                onClick={async () => {
                  setChecking(true);
                  setError('');
                  try {
                    const next = await advance(capability, { action: 'offer' });
                    if (next.url) window.location.assign(next.url);
                  } catch (e) {
                    setError(e instanceof Error ? e.message : 'Offer is unavailable');
                  } finally {
                    setChecking(false);
                  }
                }}
              >
                Review offer
              </button>
              <button
                disabled={checking}
                className="ml-3 underline"
                onClick={async () => {
                  setChecking(true);
                  try {
                    await advance(capability, { action: 'skip' });
                    await check(capability);
                  } catch (e) {
                    setError(e instanceof Error ? e.message : 'Unable to skip offer');
                  } finally {
                    setChecking(false);
                  }
                }}
              >
                Skip offer
              </button>
            </section>
          )}
          <Link
            className="underline"
            href={
              '/checkout/confirmation?' +
              new URLSearchParams({ method: 'card', orderNumber: checkout.orderNumber })
            }
          >
            View receipt
          </Link>
        </>
      )}
      {checkout?.state === 'declined' && <p>Your payment was declined.</p>}
      {checkout?.state === 'expired' && <p>This checkout has expired.</p>}
      {checkout?.state === 'pending' && (
        <p role="status">Waiting for the processor to confirm payment…</p>
      )}
      {error && <p role="alert">{error}</p>}
      {capability && (error || checkout?.state === 'pending') && (
        <button className="underline" disabled={checking} onClick={() => check(capability)}>
          Check payment status
        </button>
      )}
      <Link className="block" href="/products">
        Continue shopping
      </Link>
    </main>
  );
}
