import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function GET(request: NextRequest) {
  const customerId = request.nextUrl.searchParams.get('customerId');
  if (!customerId) {
    return NextResponse.json({ subscriptions: [] });
  }

  try {
    const commerce = getCommerce();
    const subscriptions = await commerce.subscriptions.list({
      customerId,
      limit: 50,
    });
    return NextResponse.json({ subscriptions });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to fetch subscriptions' },
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { customerId, sku, productName, price } = body;

    if (!customerId || !sku) {
      return NextResponse.json(
        { error: 'customerId and sku are required' },
        { status: 400 }
      );
    }

    const commerce = getCommerce();
    const subscription = await commerce.subscriptions.create({
      customerId,
      planName: productName || sku,
      sku,
      price: price || 0,
      interval: 'monthly',
      status: 'active',
    });

    return NextResponse.json({ subscription });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to create subscription' },
      { status: 500 }
    );
  }
}
