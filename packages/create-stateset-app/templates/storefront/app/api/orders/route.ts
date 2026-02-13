import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function GET(request: NextRequest) {
  const customerId = request.nextUrl.searchParams.get('customerId');
  const wallet = request.nextUrl.searchParams.get('wallet');

  try {
    const commerce = getCommerce();

    let resolvedCustomerId = customerId;
    if (!resolvedCustomerId && wallet) {
      const customers = await commerce.customers.list({ limit: 200 });
      const customer = customers.find(
        (c: any) => c.notes?.toLowerCase().includes(wallet.toLowerCase())
      );
      if (customer) resolvedCustomerId = customer.id;
    }

    if (!resolvedCustomerId) {
      return NextResponse.json({ orders: [] });
    }

    const orders = await commerce.orders.list({
      customerId: resolvedCustomerId,
      limit: 50,
    });

    return NextResponse.json({ orders });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch orders' },
      { status: 500 }
    );
  }
}
