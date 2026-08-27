import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';

export async function GET(request: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const wallet = request.nextUrl.searchParams.get('wallet');

  try {
    if (!wallet || !(await verifyWalletRequest(request, wallet))) {
      return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
    }
    const commerce = getCommerce();
    const order = await commerce.orders.get(id);

    if (!order) {
      return NextResponse.json({ error: 'Order not found' }, { status: 404 });
    }

    // Verify ownership if wallet provided
    if (wallet && order.customerId) {
      try {
        const customer = await commerce.customers.get(order.customerId);
        if (customer && customer.metadata?.walletAddress?.toLowerCase() !== wallet.toLowerCase()) {
          return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
        }
      } catch {}
    }

    const items = order.items || [];

    let payment = null;
    try {
      const payments = (await commerce.payments.list()).filter(
        (candidate) => candidate.orderId === id,
      );
      if (payments?.length) payment = payments[0];
    } catch {}

    return NextResponse.json({ order, items, payment });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch order' },
      { status: 500 },
    );
  }
}
