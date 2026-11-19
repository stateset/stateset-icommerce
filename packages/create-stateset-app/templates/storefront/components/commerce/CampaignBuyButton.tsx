'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useCart } from '@/hooks/useCart';

export function CampaignBuyButton({
  sku,
  name,
  label = 'Buy now',
}: {
  sku: string;
  name: string;
  label?: string;
}) {
  const { addItem } = useCart();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  return (
    <>
      <button
        disabled={busy}
        className="rounded bg-black px-5 py-3 text-white"
        onClick={async () => {
          setBusy(true);
          try {
            await addItem(sku, name, 0);
            router.push('/checkout');
          } catch {
            setBusy(false);
            setError('Could not add the offer to your cart.');
          }
        }}
      >
        {label}
      </button>
      {error && <p role="alert">{error}</p>}
    </>
  );
}
