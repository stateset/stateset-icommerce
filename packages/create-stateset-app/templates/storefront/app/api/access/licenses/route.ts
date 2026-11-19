import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';
import { walletMatches } from '@/lib/customer-metadata';
import { licensesForOrder } from '@/lib/digital-access.js';

export async function GET(request: NextRequest) {
  const wallet = request.nextUrl.searchParams.get('wallet');
  if (!wallet || !(await verifyWalletRequest(request, wallet)))
    return new NextResponse(null, { status: 401 });
  const commerce = getCommerce();
  const order = await commerce.orders.get(request.nextUrl.searchParams.get('orderId') || '');
  const customer = order && (await commerce.customers.get(order.customerId));
  if (!customer || !walletMatches(customer.metadata, wallet))
    return new NextResponse(null, { status: 403 });
  return NextResponse.json(await licensesForOrder(commerce, process.env, order!.id), {
    headers: { 'Cache-Control': 'no-store' },
  });
}
