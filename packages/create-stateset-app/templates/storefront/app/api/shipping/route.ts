import { NextRequest, NextResponse } from 'next/server';
import { getShippingProvider } from '@/lib/shipping.js';
import { quoteMembership } from '@/lib/membership-checkout.js';
import { quoteCheckout } from '@/lib/checkout-service.js';
import { getTaxProvider } from '@/lib/tax.js';
import { unitsToDecimal } from '@/lib/money.js';
import { checkCartMembershipRail } from '@/lib/digital-access.js';
import { getCommerce } from '@/lib/commerce';
import { pricingEnvironment } from '@/lib/store-pricing.js';

export async function POST(request: NextRequest) {
  try {
    const { cartId, shippingAddress, shippingMethodId } = await request.json();
    const commerce = getCommerce();
    const pricing = await pricingEnvironment(commerce);
    const provider = getShippingProvider(pricing);
    const policies = await checkCartMembershipRail(commerce, process.env, cartId);
    const recurring = policies
      ? await quoteMembership(
          commerce,
          policies,
          await quoteCheckout({
            commerce,
            cartId,
            shippingAddress,
            shippingMethodId,
            taxProvider: getTaxProvider(pricing),
            shippingProvider: provider,
          }),
          'Stripe',
        )
      : null;
    const methods = provider.listMethods(shippingAddress);
    const selected = provider.quote(shippingAddress, shippingMethodId);
    return NextResponse.json({
      methods,
      membership: recurring
        ? {
            amount: unitsToDecimal(BigInt(recurring.regularMinor), 2),
            interval: recurring.interval,
            count: recurring.count,
          }
        : null,
      selected,
      provider: provider.name,
      source: provider.source,
      notice:
        provider.source === 'starter'
          ? 'Starter shipping is free; configure STATESET_SHIPPING_METHODS_JSON before production.'
          : undefined,
    });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Shipping quote failed' },
      { status: 422 },
    );
  }
}
