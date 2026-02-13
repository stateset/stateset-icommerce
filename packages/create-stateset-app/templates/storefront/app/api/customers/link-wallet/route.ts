import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function POST(request: NextRequest) {
  try {
    const { customerId, walletAddress } = await request.json();
    if (!customerId || !walletAddress) {
      return NextResponse.json(
        { error: 'customerId and walletAddress required' },
        { status: 400 }
      );
    }

    const commerce = getCommerce();
    const customer = await commerce.customers.get(customerId);

    if (!customer) {
      return NextResponse.json({ error: 'Customer not found' }, { status: 404 });
    }

    const notes = customer.notes || '';
    if (notes.toLowerCase().includes(walletAddress.toLowerCase())) {
      return NextResponse.json({ customer, message: 'Wallet already linked' });
    }

    const updated = await commerce.customers.update(customerId, {
      notes: `${notes}\nWallet: ${walletAddress}`.trim(),
    });

    return NextResponse.json({ customer: updated });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to link wallet' },
      { status: 500 }
    );
  }
}
