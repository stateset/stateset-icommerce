import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { enterCampaign, campaignCookie } from '@/lib/campaigns.js';

export async function GET(request: NextRequest, { params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const entry = await enterCampaign(getCommerce(), process.env, {
    campaignId: slug,
    token: request.cookies.get(campaignCookie)?.value,
    source: request.nextUrl.searchParams.get('utm_source') || '',
    affiliate: request.nextUrl.searchParams.get('ref') || '',
  });
  if (!entry) return new NextResponse('Offer not found', { status: 404 });
  const location = new URL(
    '/offers/' + encodeURIComponent(slug),
    process.env.NEXT_PUBLIC_BASE_URL || request.url,
  );
  const response = NextResponse.redirect(location);
  response.headers.set('Cache-Control', 'private, no-store');
  response.cookies.set(campaignCookie, entry.token, {
    httpOnly: true,
    secure: location.protocol === 'https:',
    sameSite: 'lax',
    path: '/',
    maxAge: entry.maxAge,
  });
  return response;
}
