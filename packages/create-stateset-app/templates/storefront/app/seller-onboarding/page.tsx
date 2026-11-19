export default async function SellerOnboarding({
  searchParams,
}: {
  searchParams: Promise<{ state?: string }>;
}) {
  const expired = (await searchParams).state === 'expired';
  return (
    <main className="mx-auto max-w-lg space-y-3 px-4 py-12">
      <h1 className="text-2xl font-bold">
        {expired ? 'Onboarding link expired' : 'Payout account onboarding'}
      </h1>
      <p>
        {expired
          ? 'Sign in to your partner account to renew your verification link. If your operator opened the account for you, request a new link from them.'
          : 'Your partner account shows your payout details and verification status.'}
      </p>
      <a className="underline" href="/partners">
        Open partner account
      </a>
    </main>
  );
}
