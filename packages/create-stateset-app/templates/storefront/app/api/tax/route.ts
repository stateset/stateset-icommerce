import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { items, stateCode } = body;

    if (!items || !stateCode) {
      return NextResponse.json(
        { error: 'items and stateCode are required' },
        { status: 400 }
      );
    }

    const commerce = getCommerce();

    // Try to get tax jurisdiction for the state
    let taxRate = 0;
    try {
      const jurisdictions = await commerce.tax.listJurisdictions();
      const jurisdiction = jurisdictions.find(
        (j: any) => j.code === stateCode || j.state === stateCode
      );
      if (jurisdiction) {
        taxRate = jurisdiction.rate || 0;
      }
    } catch {
      // Fall back to a basic lookup
      const basicRates: Record<string, number> = {
        CA: 0.0725, NY: 0.08, TX: 0.0625, FL: 0.06, WA: 0.065,
        IL: 0.0625, PA: 0.06, OH: 0.0575, GA: 0.04, NC: 0.0475,
        NJ: 0.06625, VA: 0.053, MI: 0.06, AZ: 0.056, MA: 0.0625,
      };
      taxRate = basicRates[stateCode] || 0;
    }

    const subtotal = items.reduce(
      (sum: number, item: any) => sum + (item.unitPrice || 0) * (item.quantity || 1),
      0
    );

    const taxAmount = Math.round(subtotal * taxRate * 100) / 100;

    return NextResponse.json({
      taxRate,
      taxAmount,
      subtotal,
      total: subtotal + taxAmount,
      jurisdiction: stateCode,
    });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Tax calculation failed' },
      { status: 500 }
    );
  }
}
