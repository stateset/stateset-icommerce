import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';

export async function GET(request: NextRequest) {
  const address = request.nextUrl.searchParams.get('address');
  if (!address) {
    return NextResponse.json({ error: 'Wallet address required' }, { status: 400 });
  }
  if (!(await verifyWalletRequest(request, address))) {
    return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
  }

  try {
    const commerce = getCommerce();
    const customers = await commerce.customers.list();
    const customer = customers.find(
      (c: any) => c.metadata?.walletAddress?.toLowerCase() === address.toLowerCase(),
    );

    if (!customer) {
      return NextResponse.json({ customer: null, orders: [] });
    }

    const orders = (await commerce.orders.list())
      .filter((order) => order.customerId === customer.id)
      .slice(0, 50);

    return NextResponse.json({ customer, orders });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to look up customer' },
      { status: 500 },
    );
  }
}
