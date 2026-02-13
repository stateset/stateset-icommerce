import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { cartId, email, txHash, walletAddress } = body;

    if (!cartId || !email || !txHash) {
      return NextResponse.json(
        { error: 'cartId, email, and txHash are required' },
        { status: 400 }
      );
    }

    const commerce = getCommerce();

    // Get cart and items
    const [cart, items] = await Promise.all([
      commerce.carts.get(cartId),
      commerce.carts.getItems(cartId),
    ]);

    if (!items || items.length === 0) {
      return NextResponse.json({ error: 'Cart is empty' }, { status: 400 });
    }

    // Find or create customer
    let customer;
    try {
      const customers = await commerce.customers.list({ limit: 100 });
      customer = customers.find(
        (c: any) =>
          c.email === email ||
          c.notes?.toLowerCase().includes(walletAddress?.toLowerCase())
      );
    } catch {}

    if (!customer) {
      customer = await commerce.customers.create({
        email,
        firstName: '',
        lastName: '',
        notes: walletAddress ? `Wallet: ${walletAddress}` : '',
      });
    } else if (walletAddress && !customer.notes?.includes(walletAddress)) {
      try {
        await commerce.customers.update(customer.id, {
          notes: `${customer.notes || ''}\nWallet: ${walletAddress}`.trim(),
        });
      } catch {}
    }

    // Create order
    const subtotal = items.reduce(
      (sum: number, item: any) => sum + (item.unitPrice || 0) * (item.quantity || 1),
      0
    );

    const order = await commerce.orders.create({
      customerId: customer.id,
      status: 'pending',
      subtotal,
      grandTotal: subtotal,
      currency: 'USDC',
      notes: `Tx: ${txHash}`,
    });

    // Add order items
    for (const item of items) {
      await commerce.orders.addItem(order.id, {
        sku: item.sku,
        name: item.name || item.sku,
        quantity: item.quantity || 1,
        unitPrice: item.unitPrice || 0,
      });
    }

    // Record payment
    try {
      await commerce.payments.create({
        orderId: order.id,
        amount: subtotal,
        currency: 'USDC',
        status: 'completed',
        paymentMethod: 'crypto',
      });
      await commerce.orders.update(order.id, { status: 'confirmed' });
    } catch (err) {
      console.warn('Payment record failed:', err);
    }

    return NextResponse.json({
      orderId: order.id,
      orderNumber: order.orderNumber,
      status: 'confirmed',
    });
  } catch (error) {
    console.error('Checkout error:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Checkout failed' },
      { status: 500 }
    );
  }
}
