import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';

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
    if (customer?.metadata?.walletAddress?.toLowerCase() !== walletAddress.toLowerCase()) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
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
