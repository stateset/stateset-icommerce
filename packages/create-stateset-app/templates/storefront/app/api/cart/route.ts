import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { purchasableVariant } from '@/lib/campaigns.js';

async function getCartWithItems(commerce: ReturnType<typeof getCommerce>, cartId: string) {
  const [cart, items] = await Promise.all([
    commerce.carts.get(cartId),
    commerce.carts.getItems(cartId),
  ]);
  return { ...cart, items };
}

export async function GET(request: NextRequest) {
  const cartId = request.nextUrl.searchParams.get('cartId');
  if (!cartId) {
    return NextResponse.json({ error: 'Cart ID required' }, { status: 400 });
  }
  try {
    const commerce = getCommerce();
    const cart = await getCartWithItems(commerce, cartId);
    return NextResponse.json({ cart });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to get cart' },
      { status: 500 },
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const commerce = getCommerce();

    if (!body.sku) {
      const newCart = await commerce.carts.create({ customerId: body.customerId, currency: 'USD' });
      return NextResponse.json({ cart: { ...newCart, items: [] } });
    }

    const available = await purchasableVariant(commerce, body.sku);
    if (!available) return NextResponse.json({ error: 'Unknown SKU' }, { status: 404 });
    const quantity = Number(body.quantity ?? 1);
    if (!Number.isSafeInteger(quantity) || quantity < 1)
      return NextResponse.json({ error: 'Quantity must be a positive integer' }, { status: 400 });

    const cartId = body.cartId || (await commerce.carts.create({ customerId: body.customerId, currency: 'USD' })).id;
    const { product, variant } = available;
    await commerce.carts.addItemExact(cartId, {
      productId: product.id,
      variantId: variant.id,
      sku: variant.sku,
      name: product.name,
      quantity,
      unitPrice: variant.priceExact,
    });
    return NextResponse.json({ cart: await getCartWithItems(commerce, cartId) });
  } catch (error) {
    console.error('Cart error:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to process cart' },
      { status: 500 },
    );
  }
}

export async function DELETE(request: NextRequest) {
  try {
    const body = await request.json();
    const { cartId, itemId } = body;
    if (!cartId || !itemId) {
      return NextResponse.json({ error: 'Cart ID and Item ID required' }, { status: 400 });
    }
    const commerce = getCommerce();
    await commerce.carts.removeItem(itemId);
    const cart = await getCartWithItems(commerce, cartId);
    return NextResponse.json({ cart });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to remove item' },
      { status: 500 },
    );
  }
}

export async function PATCH(request: NextRequest) {
  try {
    const body = await request.json();
    const { cartId, itemId, quantity } = body;
    if (!cartId || !itemId || quantity === undefined) {
      return NextResponse.json(
        { error: 'Cart ID, Item ID, and quantity required' },
        { status: 400 },
      );
    }
    const commerce = getCommerce();
    await commerce.carts.updateItem(itemId, { quantity });
    const cart = await getCartWithItems(commerce, cartId);
    return NextResponse.json({ cart });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to update item' },
      { status: 500 },
    );
  }
}
