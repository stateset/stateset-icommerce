import { NextRequest, NextResponse } from 'next/server';
import { getShippingProvider } from '@/lib/shipping.js';

export async function POST(request: NextRequest) {
  try {
    const { shippingAddress, shippingMethodId } = await request.json();
    const provider = getShippingProvider();
    const methods = provider.listMethods(shippingAddress);
    const selected = provider.quote(shippingAddress, shippingMethodId);
    return NextResponse.json({
      methods,
      selected,
      provider: provider.name,
      source: provider.source,
      notice:
        provider.source === 'starter'
          ? 'Starter shipping is free; configure STATESET_SHIPPING_METHODS_JSON before production.'
          : undefined,
    });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Shipping quote failed' },
      { status: 422 },
    );
  }
}
