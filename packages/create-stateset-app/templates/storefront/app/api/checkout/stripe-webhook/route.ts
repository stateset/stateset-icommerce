import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { reconcileStripeCheckout, reconcileStripeRefund } from '@/lib/stripe-checkout.js';
import {
  recordMembershipStatus,
  recordPaidRenewal,
  recordRenewalFailure,
} from '@/lib/billing-recovery.js';
import { storefrontStripe } from '@/lib/stripe-client';

export const runtime = 'nodejs';

export async function POST(request: NextRequest) {
  const { stripe, webhookSecret } = storefrontStripe();
  let event;
  try {
    event = stripe.webhooks.constructEvent(
      await request.text(),
      request.headers.get('stripe-signature') || '',
      webhookSecret,
    );
  } catch {
    return NextResponse.json({ error: 'Invalid Stripe signature' }, { status: 400 });
  }
  try {
    if (
      event.type === 'checkout.session.completed' ||
      event.type === 'checkout.session.expired' ||
      event.type === 'checkout.session.async_payment_succeeded'
    ) {
      if (event.data.object.metadata?.storefront_quote) {
        await reconcileStripeCheckout(getCommerce(), stripe, event.data.object.id);
      }
    } else if (
      event.type === 'invoice.payment_failed' &&
      process.env.STATESET_RENEWAL_RECOVERY === 'true'
    ) {
      await recordRenewalFailure(getCommerce(), event, stripe);
    } else if (event.type === 'invoice.paid') {
      await recordPaidRenewal(getCommerce(), stripe, event.data.object.id);
    } else if (
      [
        'customer.subscription.updated',
        'customer.subscription.deleted',
        'customer.subscription.paused',
        'customer.subscription.resumed',
      ].includes(event.type)
    ) {
      await recordMembershipStatus(getCommerce(), stripe, event);
    } else if (
      event.type === 'refund.created' ||
      event.type === 'refund.updated' ||
      event.type === 'refund.failed'
    ) {
      await reconcileStripeRefund(getCommerce(), stripe, event.data.object.id);
    }
    return NextResponse.json({ received: true });
  } catch {
    return NextResponse.json({ error: 'Settlement reconciliation pending' }, { status: 503 });
  }
}
