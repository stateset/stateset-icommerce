import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const wallet = request.nextUrl.searchParams.get('wallet');

  try {
    const commerce = getCommerce();
    const order = await commerce.orders.get(id);

    if (!order) {
      return NextResponse.json({ error: 'Order not found' }, { status: 404 });
    }

    // Verify ownership if wallet provided
    if (wallet && order.customerId) {
      try {
        const customer = await commerce.customers.get(order.customerId);
        if (customer && !customer.notes?.toLowerCase().includes(wallet.toLowerCase())) {
          return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
        }
      } catch {}
    }

    let items: any[] = [];
    try {
      items = await commerce.orders.getItems(id);
    } catch {}

    let payment = null;
    try {
      const payments = await commerce.payments.list({ orderId: id });
      if (payments?.length) payment = payments[0];
    } catch {}

    return NextResponse.json({ order, items, payment });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch order' },
      { status: 500 }
    );
  }
}
