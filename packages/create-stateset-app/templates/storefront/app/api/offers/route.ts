import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import {
  activeOffer,
  campaignCookie,
  decodeCampaign,
  catalogOffer,
  recordCampaignStep,
} from '@/lib/campaigns.js';
import { storefrontStripe } from '@/lib/stripe-client';
import { CheckoutError } from '@/lib/checkout-service.js';
import { startCheckoutUpsell } from '@/lib/stripe-checkout.js';
import { startRoutedCheckout } from '@/lib/routed-checkout.js';

export async function GET(request: NextRequest) {
  const attribution = decodeCampaign(process.env, request.cookies.get(campaignCookie)?.value);
  const variant = await activeOffer(getCommerce(), process.env, attribution);
  if (variant) await recordCampaignStep(getCommerce(), attribution, 'checkout');
  const offer = variant?.bumpSku ? await catalogOffer(getCommerce(), variant.bumpSku) : null;
  return NextResponse.json(
    {
      bump: offer ? { name: offer.name, unitPrice: offer.unitPrice } : null,
      saveCardForOffer: Boolean(
        variant?.upsellSku &&
        process.env.STATESET_CHECKOUT_PROVIDER === 'stripe' &&
        process.env.NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY,
      ),
      coupon: variant?.couponCode,
    },
    { headers: { 'Cache-Control': 'private, no-store' } },
  );
}

export async function POST(request: NextRequest) {
  const attribution = decodeCampaign(process.env, request.cookies.get(campaignCookie)?.value);
  if (!attribution) return NextResponse.json({ error: 'Offer expired' }, { status: 403 });
  try {
    const { stripe, origin } = storefrontStripe();
    const { sessionId, action, expectedTotal } = await request.json();
    return NextResponse.json(
      await startCheckoutUpsell(
        getCommerce(),
        stripe,
        origin,
        sessionId,
        attribution.visitor,
        action,
        expectedTotal,
        startRoutedCheckout,
      ),
    );
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof CheckoutError ? error.message : 'Offer checkout unavailable' },
      { status: error instanceof CheckoutError ? error.status : 503 },
    );
  }
}
