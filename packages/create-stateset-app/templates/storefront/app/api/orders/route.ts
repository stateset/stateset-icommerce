import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';

export async function GET(request: NextRequest) {
  const customerId = request.nextUrl.searchParams.get('customerId');
  const wallet = request.nextUrl.searchParams.get('wallet');

  try {
    if (!wallet || !(await verifyWalletRequest(request, wallet))) {
      return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
    }
    const commerce = getCommerce();

    const customers = await commerce.customers.list();
    const customer = customers.find(
      (c: any) => c.metadata?.walletAddress?.toLowerCase() === wallet.toLowerCase(),
    );
    const resolvedCustomerId = customer?.id;
    if (customerId && customerId !== resolvedCustomerId) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
    }

    if (!resolvedCustomerId) {
      return NextResponse.json({ orders: [] });
    }

    const orders = (await commerce.orders.list())
      .filter((order) => order.customerId === resolvedCustomerId)
      .slice(0, 50);

    return NextResponse.json({ orders });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch orders' },
      { status: 500 },
    );
  }
}
