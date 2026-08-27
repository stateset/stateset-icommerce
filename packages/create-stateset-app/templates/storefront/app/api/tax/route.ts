import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { getTaxProvider } from '@/lib/tax.js';

export async function POST(request: NextRequest) {
  try {
    const { cartId, stateCode } = await request.json();
    if (!cartId || !stateCode)
      return NextResponse.json({ error: 'cartId and stateCode are required' }, { status: 400 });
    const commerce = getCommerce();
    const items = await commerce.carts.getItems(cartId);
    const provider = getTaxProvider();
    const configured = provider.hasJurisdiction(stateCode);
    if (!configured) {
      return NextResponse.json(
        { error: `Tax is not configured for ${String(stateCode).toUpperCase()}` },
        { status: 422 },
      );
    }
    const totals = provider.calculateCart(
      items.map((item: any) => ({
        sku: item.sku,
        quantity: item.quantity,
        unitPrice: item.unitPriceExact || String(item.unitPrice),
      })),
      stateCode,
    );
    return NextResponse.json({
      taxRateExact: totals.rate,
      taxAmountExact: totals.tax,
      subtotalExact: totals.subtotal,
      totalExact: totals.total,
      jurisdiction: stateCode,
      configured,
      provider: provider.name,
      source: provider.source,
      notice:
        provider.source === 'starter'
          ? 'Starter rate only; configure STATESET_TAX_RATES_JSON before production.'
          : undefined,
    });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Tax calculation failed' },
      { status: 500 },
    );
  }
}
