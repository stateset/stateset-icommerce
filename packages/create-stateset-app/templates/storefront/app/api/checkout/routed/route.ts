import { NextRequest, NextResponse } from 'next/server';
import Stripe from 'stripe';
import { getCommerce } from '@/lib/commerce';
import { CheckoutError } from '@/lib/checkout-service.js';
import { campaignCookie, decodeCampaign } from '@/lib/campaigns.js';
import {
  advanceRoutedCheckout,
  routedOffer,
  routedSession,
  startRoutedCheckout,
} from '@/lib/routed-checkout.js';
import { assessStoreConfiguration } from '@/lib/store-readiness.js';
import { inspectStoreDatabase } from '@/lib/store-database-check.js';
import { pricingEnvironment } from '@/lib/store-pricing.js';

export const runtime = 'nodejs';
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const env = process.env;
    const commerce = getCommerce();
    const bearer = request.headers.get('Authorization');
    const headers = { 'Cache-Control': 'private, no-store' };
    if (bearer !== null) {
      const session = await routedSession(commerce, bearer.replace(/^Bearer /, ''));
      if (body.action === 'offer' || body.action === 'skip')
        return NextResponse.json(await routedOffer(commerce, env, session, body.action), {
          headers,
        });
      const response = await advanceRoutedCheckout(commerce, env, session, {
        account: body.account,
        token: body.token,
        showOffer: true,
        stripeSecret: async (key: string, reference: string) =>
          (await new Stripe(key).paymentIntents.retrieve(reference)).client_secret,
      });
      return NextResponse.json(response, { headers });
    }
    const options = { launch: env.NODE_ENV === 'production' };
    if (
      env.STATESET_CHECKOUT_PROVIDER !== 'routed' ||
      inspectStoreDatabase(env, options).status === 'fail' ||
      !assessStoreConfiguration(await pricingEnvironment(commerce, env), options).configurationReady
    )
      throw new CheckoutError('Routed card checkout is unavailable', 503);
    const campaign = decodeCampaign(env, request.cookies.get(campaignCookie)?.value);
    return NextResponse.json(await startRoutedCheckout({ commerce, env, campaign, input: body }), {
      headers,
    });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof CheckoutError
            ? error.message
            : 'Card checkout could not be confirmed. Check the same purchase again.',
      },
      { status: error instanceof CheckoutError ? error.status : 503 },
    );
  }
}
