import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

const VARIANT_CACHE = new Map<string, { variant: any; timestamp: number }>();
const VARIANT_CACHE_TTL = 300_000;

function getCachedVariant(commerce: ReturnType<typeof getCommerce>, sku: string): Promise<any> {
  const cached = VARIANT_CACHE.get(sku);
  const now = Date.now();
  if (cached && now - cached.timestamp < VARIANT_CACHE_TTL) {
    return Promise.resolve(cached.variant);
  }
  return commerce.products.getVariantBySku(sku).then((variant: any) => {
    if (variant) VARIANT_CACHE.set(sku, { variant, timestamp: now });
    return variant;
  });
}

async function getProductInfoBySku(commerce: ReturnType<typeof getCommerce>, sku: string) {
  try {
    const variant = await getCachedVariant(commerce, sku);
    if (variant) {
      let productName = variant.name || sku;
      if (variant.productId) {
        try {
          const product = await commerce.products.get(variant.productId);
          if (product?.name) productName = product.name;
        } catch {}
      }
      return { name: productName, unitPrice: variant.price || 0 };
    }
  } catch {}
  return { name: sku, unitPrice: 0 };
}

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
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const commerce = getCommerce();

    if (body.cartId && body.sku) {
      const productInfo = await getProductInfoBySku(commerce, body.sku);
      await commerce.carts.addItem(body.cartId, {
        sku: body.sku,
        name: body.name || productInfo.name,
        quantity: body.quantity || 1,
        unitPrice: body.unitPrice ?? productInfo.unitPrice,
      });
      const cart = await getCartWithItems(commerce, body.cartId);
      return NextResponse.json({ cart });
    }

    const newCart = await commerce.carts.create({ customerId: body.customerId });

    if (body.sku) {
      const productInfo = await getProductInfoBySku(commerce, body.sku);
      await commerce.carts.addItem(newCart.id, {
        sku: body.sku,
        name: body.name || productInfo.name,
        quantity: body.quantity || 1,
        unitPrice: body.unitPrice ?? productInfo.unitPrice,
      });
      const cart = await getCartWithItems(commerce, newCart.id);
      return NextResponse.json({ cart });
    }

    return NextResponse.json({ cart: { ...newCart, items: [] } });
  } catch (error) {
    console.error('Cart error:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to process cart' },
      { status: 500 }
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
      { status: 500 }
    );
  }
}

export async function PATCH(request: NextRequest) {
  try {
    const body = await request.json();
    const { cartId, itemId, quantity } = body;
    if (!cartId || !itemId || quantity === undefined) {
      return NextResponse.json({ error: 'Cart ID, Item ID, and quantity required' }, { status: 400 });
    }
    const commerce = getCommerce();
    await commerce.carts.updateItem(itemId, { quantity });
    const cart = await getCartWithItems(commerce, cartId);
    return NextResponse.json({ cart });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to update item' },
      { status: 500 }
    );
  }
}
