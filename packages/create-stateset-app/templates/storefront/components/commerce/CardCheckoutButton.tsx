'use client';

import { useEffect, useState } from 'react';

export function CardCheckoutButton({
  checkout,
  disabled,
  routed = false,
}: {
  routed?: boolean;
  checkout: { cartId: string; shippingAddress: object; shippingMethodId: string; email: string };
  disabled: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [offer, setOffer] = useState<{ name: string; unitPrice: string } | null>(null);
  const [bump, setBump] = useState(false);
  const [canSave, setCanSave] = useState(false);
  const [saveCardForOffer, setSaveCard] = useState(false);
  const [coupon, setCoupon] = useState('');
  useEffect(() => {
    fetch('/api/offers')
      .then((res) => res.json())
      .then((data) => {
        setOffer(data.bump);
        setCanSave(data.saveCardForOffer);
        setCoupon(data.coupon || '');
      })
      .catch(() => setOffer(null));
  }, []);
  async function pay() {
    setBusy(true);
    setError('');
    try {
      const response = await fetch(routed ? '/api/checkout/routed' : '/api/checkout/card', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...checkout, bump, saveCardForOffer }),
      });
      const result = await response.json();
      if (!response.ok || !result.url)
        throw new Error(result.error || 'Card checkout is unavailable');
      window.location.assign(result.url);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Card checkout is unavailable');
      setBusy(false);
    }
  }
  return (
    <div>
      {coupon && (
        <p className="mb-3">Campaign coupon {coupon} applies to eligible items at checkout.</p>
      )}
      {offer && (
        <label className="mb-4 flex items-center gap-2">
          <input type="checkbox" checked={bump} onChange={(e) => setBump(e.target.checked)} />
          Add {offer.name} for USD {offer.unitPrice} before tax
        </label>
      )}
      {canSave && (
        <label className="mb-4 flex items-start gap-2">
          <input
            type="checkbox"
            checked={saveCardForOffer}
            onChange={(e) => setSaveCard(e.target.checked)}
          />
          Save my card for an optional offer after checkout. Charge it only if I accept the
          displayed total.
        </label>
      )}
      <button
        type="button"
        disabled={disabled || busy}
        onClick={pay}
        className="w-full rounded-lg bg-blue-600 px-4 py-3 font-medium text-white disabled:opacity-50"
      >
        {busy
          ? 'Opening checkout…'
          : routed
            ? 'Continue to card payment'
            : 'Pay by card or digital wallet'}
      </button>
      {error && (
        <p role="alert" className="mt-2 text-sm text-red-700">
          {error}
        </p>
      )}
    </div>
  );
}
