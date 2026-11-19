import { NextRequest, NextResponse } from 'next/server';
import { readMerchantRecord } from '@/lib/merchant-records.js';
import { storefrontStripe } from '@/lib/stripe-client';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';
import { walletMatches } from '@/lib/customer-metadata';

export async function PATCH(request: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;

  try {
    const body = await request.json();
    const { action, walletAddress } = body;

    const commerce = getCommerce();
    if (!walletAddress || !(await verifyWalletRequest(request, walletAddress))) {
      return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
    }
    const subscription = await commerce.subscriptions.get(id);
    if (!subscription)
      return NextResponse.json({ error: 'Subscription not found' }, { status: 404 });
    const customer = await commerce.customers.get(subscription.customerId);
    if (!walletMatches(customer?.metadata, walletAddress)) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
    }

    const billing = await readMerchantRecord(commerce, 'billing-native-' + id);
    if (billing) {
      if (action !== 'cancel')
        return NextResponse.json(
          { error: 'This membership is managed by its billing provider.' },
          { status: 409 },
        );
      const { stripe, origin } = storefrontStripe();
      const remote = await stripe.subscriptions.retrieve(billing.providerId);
      if (remote.customer !== billing.customer || remote.metadata?.stateset_subscription_id !== id)
        throw new Error('Provider membership differs from this account');
      const portal = await stripe.billingPortal.sessions.create({
        customer: billing.customer,
        return_url: new URL('/account/subscriptions', origin).href,
        flow_data: {
          type: 'subscription_cancel',
          subscription_cancel: { subscription: billing.providerId },
        },
      });
      return NextResponse.json({ url: portal.url }, { headers: { 'Cache-Control': 'no-store' } });
    }
    if (action === 'cancel') {
      const updated = await commerce.subscriptions.cancel(id, { immediate: false });
      return NextResponse.json({ subscription: updated });
    }

    if (action === 'pause') {
      const updated = await commerce.subscriptions.pause(id);
      return NextResponse.json({ subscription: updated });
    }

    if (action === 'resume') {
      const updated = await commerce.subscriptions.resume(id);
      return NextResponse.json({ subscription: updated });
    }

    return NextResponse.json({ error: 'Unknown action' }, { status: 400 });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to update subscription' },
      { status: 500 },
    );
  }
}
