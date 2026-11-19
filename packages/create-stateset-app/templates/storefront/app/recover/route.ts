import { NextRequest, NextResponse } from 'next/server';
import { recoveryPortal } from '@/lib/billing-recovery.js';
import { storefrontStripe } from '@/lib/stripe-client';

export async function GET(request: NextRequest) {
  try {
    const url = await recoveryPortal(
      storefrontStripe().stripe,
      process.env,
      request.nextUrl.searchParams.get('token') || '',
    );
    return new NextResponse(null, {
      status: 303,
      headers: { Location: url, 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' },
    });
  } catch {
    return new NextResponse(
      'This payment update link is unavailable. Contact the store for a new link.',
      { status: 400, headers: { 'Cache-Control': 'no-store' } },
    );
  }
}
