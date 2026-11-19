import { NextRequest, NextResponse } from 'next/server';
import { verifyMessage } from 'viem';
import { getCommerce } from '@/lib/commerce';
import { partnerPrograms, openPartnerAccount } from '@/lib/partner-accounts.js';
import { payoutStripe } from '@/lib/stripe-client';

const headers = { 'Cache-Control': 'private, no-store' };
export function GET() {
  return NextResponse.json(partnerPrograms(process.env), { headers });
}
export async function POST(request: NextRequest) {
  if (
    request.headers.get('origin') !==
    new URL(process.env.NEXT_PUBLIC_BASE_URL || request.url).origin
  )
    return new NextResponse(null, { status: 403, headers });
  try {
    const result = await openPartnerAccount(
      getCommerce(),
      process.env,
      await request.json(),
      verifyMessage,
      payoutStripe,
    );
    return NextResponse.json(result, { headers });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Unable to open partner account' },
      { status: 403, headers },
    );
  }
}
