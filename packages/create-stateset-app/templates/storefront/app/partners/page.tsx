'use client';

import { useEffect, useState } from 'react';
import { useAccount, useSignMessage } from 'wagmi';
import { ConnectWallet } from '@/components/commerce/ConnectWallet';
import { partnerMessage } from '@/lib/partner-signature.js';

export default function Partners() {
  const { address } = useAccount();
  const { signMessageAsync } = useSignMessage();
  const [program, setProgram] = useState<{
    bps: number | null;
    countries: string[];
    affiliateCountries: string[];
  }>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void fetch('/api/partners', { cache: 'no-store' })
      .then((r) => r.json())
      .then((r) => {
        setProgram(r);
      })
      .catch(() => setError('Unable to load partner programs'));
  }, []);
  async function enter(
    role: 'affiliate' | 'seller',
    action: 'portal' | 'onboard',
    country = '',
    email = '',
  ) {
    if (!address || !program) return;
    setBusy(true);
    setError('');
    try {
      const account = {
        role,
        action,
        wallet: address,
        at: Date.now(),
        nonce: crypto.randomUUID(),
        ...(action === 'onboard' && { country, email: email.trim().toLowerCase() }),
        ...(role === 'affiliate' && { bps: program.bps }),
      };
      const signature = await signMessageAsync({
        message: partnerMessage(window.location.origin, account),
      });
      const response = await fetch('/api/partners', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...account, signature }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error || 'Unable to open partner account');
      window.location.assign(result.url);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to open partner account');
      setBusy(false);
    }
  }
  return (
    <main className="mx-auto max-w-xl space-y-6 p-6">
      <h1 className="text-2xl font-bold">Partner accounts</h1>
      {!address && (
        <>
          <p>Connect the wallet you want to use for your partner account.</p>
          <ConnectWallet />
        </>
      )}
      {program && (
        <>
          <section className="space-y-2">
            <h2 className="text-lg font-semibold">Affiliate program</h2>
            {program.bps !== null && (
              <p>New affiliates earn {program.bps / 100}% of attributed net merchandise sales.</p>
            )}
            <button
              disabled={!address || busy}
              className="underline"
              onClick={() => enter('affiliate', 'portal')}
            >
              {program.bps === null ? 'Open affiliate account' : 'Join or open affiliate account'}
            </button>
            <PayoutEnrollment
              role="affiliate"
              countries={program.affiliateCountries}
              disabled={!address || busy}
              submit={enter}
            />
          </section>
          <section className="space-y-2">
            <h2 className="text-lg font-semibold">Seller account</h2>
            <button
              disabled={!address || busy}
              className="underline"
              onClick={() => enter('seller', 'portal')}
            >
              Open seller statement
            </button>
            <PayoutEnrollment
              role="seller"
              countries={program.countries}
              disabled={!address || busy}
              submit={enter}
            />
          </section>
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </main>
  );
}

function PayoutEnrollment({
  role,
  countries,
  disabled,
  submit,
}: {
  role: 'affiliate' | 'seller';
  countries: string[];
  disabled: boolean;
  submit: (
    role: 'affiliate' | 'seller',
    action: 'onboard',
    country: string,
    email: string,
  ) => Promise<void>;
}) {
  if (!countries.length) return null;
  return (
    <form
      className="space-y-2"
      onSubmit={(event) => {
        event.preventDefault();
        const data = new FormData(event.currentTarget);
        void submit(role, 'onboard', String(data.get('country')), String(data.get('email')));
      }}
    >
      <label className="block">
        Payout email
        <input name="email" type="email" required className="block rounded border p-2" />
      </label>
      <label className="block">
        Business country
        <select name="country" className="ml-2 rounded border p-2">
          {countries.map((code) => (
            <option key={code}>{code}</option>
          ))}
        </select>
      </label>
      <button disabled={disabled} className="rounded border px-3 py-2">
        Start or resume Stripe verification
      </button>
    </form>
  );
}
