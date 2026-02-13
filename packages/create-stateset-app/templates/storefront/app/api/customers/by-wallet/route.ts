import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function GET(request: NextRequest) {
  const address = request.nextUrl.searchParams.get('address');
  if (!address) {
    return NextResponse.json({ error: 'Wallet address required' }, { status: 400 });
  }

  try {
    const commerce = getCommerce();
    const customers = await commerce.customers.list({ limit: 200 });
    const customer = customers.find(
      (c: any) => c.notes?.toLowerCase().includes(address.toLowerCase())
    );

    if (!customer) {
      return NextResponse.json({ customer: null, orders: [] });
    }

    const orders = await commerce.orders.list({ customerId: customer.id, limit: 50 });

    return NextResponse.json({ customer, orders });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to look up customer' },
      { status: 500 }
    );
  }
}
