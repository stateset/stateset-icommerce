import { NextResponse } from 'next/server';

// Wallets are linked only by the verified checkout settlement flow. Accepting
// a customer ID and address here allowed account takeover without proof of
// either wallet or email ownership.
export async function POST() {
  return NextResponse.json(
    {
      error:
        'Direct wallet linking is disabled; use verified checkout or an operator-owned recovery flow',
    },
    { status: 410 },
  );
}
