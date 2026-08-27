import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';

export async function GET(request: NextRequest) {
  const customerId = request.nextUrl.searchParams.get('customerId');
  const wallet = request.nextUrl.searchParams.get('wallet');
  if (!customerId) {
    return NextResponse.json({ subscriptions: [] });
  }

  try {
    if (!wallet || !(await verifyWalletRequest(request, wallet))) {
      return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
    }
    const commerce = getCommerce();
    const customer = await commerce.customers.get(customerId);
    if (customer?.metadata?.walletAddress?.toLowerCase() !== wallet.toLowerCase()) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
    }
    const subscriptions = await commerce.subscriptions.list({
      customerId,
      limit: 50,
    });
    return NextResponse.json({ subscriptions });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch subscriptions' },
      { status: 500 },
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { customerId, sku, walletAddress } = body;

    if (!customerId || !sku) {
      return NextResponse.json({ error: 'customerId and sku are required' }, { status: 400 });
    }

    const commerce = getCommerce();
    if (!walletAddress || !(await verifyWalletRequest(request, walletAddress))) {
      return NextResponse.json({ error: 'Wallet signature required' }, { status: 401 });
    }
    const customer = await commerce.customers.get(customerId);
    if (customer?.metadata?.walletAddress?.toLowerCase() !== walletAddress.toLowerCase()) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
    }
    const variant = await commerce.products.getVariantBySku(sku);
    if (!variant) return NextResponse.json({ error: 'Unknown SKU' }, { status: 404 });
    let plan = await commerce.subscriptions.getPlanByCode(`monthly-${sku}`);
    if (!plan) {
      plan = await commerce.subscriptions.createPlan({
        name: `Monthly ${variant.name || sku}`,
        code: `monthly-${sku}`,
        billingInterval: 'monthly',
        price: variant.price,
        currency: 'USD',
      });
    }
    const subscription = await commerce.subscriptions.subscribe({
      customerId,
      planId: plan.id,
    });

    return NextResponse.json({ subscription });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to create subscription' },
      { status: 500 },
    );
  }
}
