import { NextRequest, NextResponse } from 'next/server';
import { readMerchantRecord } from '@/lib/merchant-records.js';
import { membershipManagement } from '@/lib/membership-checkout.js';
import { licensesForOrder } from '@/lib/digital-access.js';
import { decodeCampaign, campaignCookie, recordCampaignStep } from '@/lib/campaigns.js';
import { getCommerce } from '@/lib/commerce';
import { CheckoutError } from '@/lib/checkout-service.js';
import { startStripeCheckout, reconcileStripeCheckout } from '@/lib/stripe-checkout.js';
import { storefrontStripe } from '@/lib/stripe-client';
import { assessStoreConfiguration } from '@/lib/store-readiness.js';
import { pricingEnvironment } from '@/lib/store-pricing.js';
import { inspectStoreDatabase } from '@/lib/store-database-check.js';

export const runtime = 'nodejs';

function failure(error: unknown) {
  const status = error instanceof CheckoutError ? error.status : 503;
  return NextResponse.json(
    {
      error:
        error instanceof CheckoutError
          ? error.message
          : 'Unable to reach card checkout. Retry this checkout.',
    },
    { status },
  );
}

export async function POST(request: NextRequest) {
  try {
    const input = await request.json();
    if (input.action === 'manage-membership') {
      const { stripe, origin } = storefrontStripe();
      const result = await membershipManagement(getCommerce(), stripe, input.sessionId, origin);
      return NextResponse.json({ url: result.url }, { headers: { 'Cache-Control': 'no-store' } });
    }
    if (process.env.STATESET_CHECKOUT_PROVIDER !== 'stripe')
      throw new CheckoutError('Hosted checkout is disabled', 409);
    const options = { launch: process.env.NODE_ENV === 'production' };
    if (
      inspectStoreDatabase(process.env, options).status === 'fail' ||
      !assessStoreConfiguration(await pricingEnvironment(getCommerce()), options).configurationReady
    ) {
      throw new CheckoutError('Store configuration is incomplete', 503);
    }
    const { stripe, origin } = storefrontStripe();
    const campaign = decodeCampaign(process.env, request.cookies.get(campaignCookie)?.value);
    const result = await startStripeCheckout({
      commerce: getCommerce(),
      stripe,
      origin,
      input,
      campaign,
    });
    return NextResponse.json(result);
  } catch (error) {
    return failure(error);
  }
}

export async function GET(request: NextRequest) {
  try {
    const { stripe } = storefrontStripe();
    const result = await reconcileStripeCheckout(
      getCommerce(),
      stripe,
      request.nextUrl.searchParams.get('session_id') || '',
    );
    if ('upsell' in result && result.upsell && 'orderId' in result) {
      const attribution = decodeCampaign(process.env, request.cookies.get(campaignCookie)?.value);
      const orderTerms = await readMerchantRecord(
        getCommerce(),
        'campaign-order-' + result.orderId,
      );
      if (attribution?.visitor === orderTerms?.visitor)
        await recordCampaignStep(getCommerce(), orderTerms, 'upsell-view', result.orderId);
    }
    const licenses =
      'orderId' in result && result.state === 'paid'
        ? await licensesForOrder(getCommerce(), process.env, result.orderId)
        : [];
    return NextResponse.json({ ...result, licenses }, { headers: { 'Cache-Control': 'no-store' } });
  } catch (error) {
    return failure(error);
  }
}
