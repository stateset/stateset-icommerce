import { isWhopCatalogVariant } from '@/lib/whop-catalog.js';
import { importedOneTimeProduct } from '@/lib/next-catalog.js';
import { NextRequest, NextResponse } from 'next/server';
import { digitalProducts } from '@/lib/digital-access.js';
import { purchasableVariant } from '@/lib/campaigns.js';
import { getCommerce } from '@/lib/commerce';
import { verifyWalletRequest } from '@/lib/wallet-auth';
import { walletMatches } from '@/lib/customer-metadata';

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
    if (!walletMatches(customer?.metadata, wallet)) {
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
    if (!walletMatches(customer?.metadata, walletAddress)) {
      return NextResponse.json({ error: 'Not authorized' }, { status: 403 });
    }
    if (digitalProducts(process.env)[sku]?.membershipPlanId)
      return NextResponse.json(
        { error: 'Complete checkout to start this paid membership.' },
        { status: 409 },
      );
    const available = await purchasableVariant(commerce, sku);
    if (!available) return NextResponse.json({ error: 'Unknown SKU' }, { status: 404 });
    const { product, variant } = available;
    if (
      (await importedOneTimeProduct(commerce, variant.productId)) ||
      isWhopCatalogVariant(product, variant)
    )
      return NextResponse.json(
        { error: 'This product supports one-time purchase only.' },
        { status: 409 },
      );
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
