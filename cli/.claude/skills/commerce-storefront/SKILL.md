---
name: commerce-storefront
description: Use when creating new storefront websites, scaffolding e-commerce projects, or building commerce applications with StateSet.
---

# Commerce Storefront Skill

Domain knowledge for creating complete e-commerce storefronts using StateSet iCommerce engine.

## Overview

The storefront agent creates production-ready e-commerce websites that use `@stateset/embedded` (Node.js) or `@stateset/embedded-wasm` (browser) as the commerce backend.

## Supported Frameworks

| Framework | Package | Use Case |
|-----------|---------|----------|
| **Next.js 14+** | `@stateset/embedded` | Full-stack React with SSR |
| **Remix** | `@stateset/embedded` | Full-stack React with loaders |
| **Astro** | `@stateset/embedded` | Static + Islands architecture |
| **Vite + React** | `@stateset/embedded-wasm` | SPA with client-side commerce |
| **SvelteKit** | `@stateset/embedded` | Full-stack Svelte |

## Project Structure

### Next.js Storefront (Recommended)

```
my-store/
├── app/
│   ├── layout.tsx              # Root layout with providers
│   ├── page.tsx                # Homepage
│   ├── products/
│   │   ├── page.tsx            # Product listing
│   │   └── [slug]/
│   │       └── page.tsx        # Product detail
│   ├── cart/
│   │   └── page.tsx            # Shopping cart
│   ├── checkout/
│   │   └── page.tsx            # Checkout flow
│   ├── account/
│   │   ├── page.tsx            # Account dashboard
│   │   ├── orders/
│   │   │   └── page.tsx        # Order history
│   │   └── addresses/
│   │       └── page.tsx        # Saved addresses
│   └── api/
│       ├── commerce/
│       │   └── [...path]/
│       │       └── route.ts    # Commerce API proxy
│       └── webhooks/
│           └── route.ts        # Webhook handlers
├── components/
│   ├── ui/                     # Base UI components
│   ├── commerce/               # Commerce-specific components
│   │   ├── ProductCard.tsx
│   │   ├── ProductGrid.tsx
│   │   ├── CartDrawer.tsx
│   │   ├── CartItem.tsx
│   │   ├── CheckoutForm.tsx
│   │   ├── AddressForm.tsx
│   │   └── OrderSummary.tsx
│   └── layout/
│       ├── Header.tsx
│       ├── Footer.tsx
│       └── Navigation.tsx
├── lib/
│   ├── commerce.ts             # StateSet client initialization
│   ├── cart.ts                 # Cart utilities
│   └── utils.ts                # Helper functions
├── hooks/
│   ├── useCart.ts              # Cart state hook
│   ├── useProducts.ts          # Product fetching hook
│   └── useCheckout.ts          # Checkout flow hook
├── store.db                    # SQLite database (gitignored)
├── package.json
├── tailwind.config.ts
└── next.config.js
```

## Commerce Client Setup

### Server-Side (Next.js/Remix)

```typescript
// lib/commerce.ts
import { Commerce } from '@stateset/embedded';

// Singleton instance for server
let commerce: Commerce | null = null;

export function getCommerce(): Commerce {
  if (!commerce) {
    commerce = new Commerce(process.env.DATABASE_PATH || './store.db');
  }
  return commerce;
}

// Usage in Server Components
export async function getProducts() {
  const commerce = getCommerce();
  return commerce.products.list();
}

export async function getProduct(slug: string) {
  const commerce = getCommerce();
  return commerce.products.getBySlug(slug);
}
```

### Client-Side (WASM)

```typescript
// lib/commerce-client.ts
import init, { Commerce } from '@stateset/embedded-wasm';

let commerce: Commerce | null = null;
let initPromise: Promise<void> | null = null;

export async function getCommerceClient(): Promise<Commerce> {
  if (!initPromise) {
    initPromise = init();
  }
  await initPromise;

  if (!commerce) {
    commerce = new Commerce(':memory:'); // In-memory for client
  }
  return commerce;
}
```

## Page Templates

### Homepage

```tsx
// app/page.tsx
import { getCommerce } from '@/lib/commerce';
import { ProductGrid } from '@/components/commerce/ProductGrid';
import { Hero } from '@/components/Hero';

export default async function HomePage() {
  const commerce = getCommerce();
  const { products } = await commerce.products.list({ limit: 8 });
  const { summary } = await commerce.analytics.salesSummary({ period: 'last30days' });

  return (
    <main>
      <Hero
        title="Welcome to Our Store"
        subtitle="Powered by StateSet iCommerce"
      />

      <section className="py-16">
        <div className="container mx-auto px-4">
          <h2 className="text-2xl font-bold mb-8">Featured Products</h2>
          <ProductGrid products={products} />
        </div>
      </section>
    </main>
  );
}
```

### Product Listing

```tsx
// app/products/page.tsx
import { getCommerce } from '@/lib/commerce';
import { ProductGrid } from '@/components/commerce/ProductGrid';
import { ProductFilters } from '@/components/commerce/ProductFilters';

interface Props {
  searchParams: { category?: string; sort?: string; page?: string };
}

export default async function ProductsPage({ searchParams }: Props) {
  const commerce = getCommerce();
  const page = parseInt(searchParams.page || '1');
  const limit = 12;

  const { products, totalCount } = await commerce.products.list({
    limit,
    offset: (page - 1) * limit,
    // Add filters based on searchParams
  });

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">All Products</h1>

      <div className="flex gap-8">
        <aside className="w-64 flex-shrink-0">
          <ProductFilters />
        </aside>

        <main className="flex-1">
          <ProductGrid products={products} />

          {/* Pagination */}
          <div className="mt-8 flex justify-center gap-2">
            {/* Pagination controls */}
          </div>
        </main>
      </div>
    </div>
  );
}
```

### Product Detail

```tsx
// app/products/[slug]/page.tsx
import { getCommerce } from '@/lib/commerce';
import { ProductImages } from '@/components/commerce/ProductImages';
import { AddToCartButton } from '@/components/commerce/AddToCartButton';
import { notFound } from 'next/navigation';

interface Props {
  params: { slug: string };
}

export default async function ProductPage({ params }: Props) {
  const commerce = getCommerce();
  const product = await commerce.products.getBySlug(params.slug);

  if (!product) {
    notFound();
  }

  const variants = product.variants || [];
  const defaultVariant = variants.find(v => v.isDefault) || variants[0];

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="grid md:grid-cols-2 gap-8">
        <ProductImages images={product.images} />

        <div>
          <h1 className="text-3xl font-bold">{product.name}</h1>
          <p className="text-2xl font-semibold mt-4">
            ${defaultVariant?.price?.toFixed(2)}
          </p>

          <p className="mt-4 text-gray-600">{product.description}</p>

          {variants.length > 1 && (
            <div className="mt-6">
              <label className="block text-sm font-medium mb-2">
                Select Option
              </label>
              {/* Variant selector */}
            </div>
          )}

          <AddToCartButton
            productId={product.id}
            variantId={defaultVariant?.id}
            className="mt-6"
          />
        </div>
      </div>
    </div>
  );
}
```

### Cart Page

```tsx
// app/cart/page.tsx
'use client';

import { useCart } from '@/hooks/useCart';
import { CartItem } from '@/components/commerce/CartItem';
import { OrderSummary } from '@/components/commerce/OrderSummary';
import Link from 'next/link';

export default function CartPage() {
  const { cart, isLoading, updateItem, removeItem } = useCart();

  if (isLoading) {
    return <div className="container mx-auto px-4 py-8">Loading...</div>;
  }

  if (!cart || cart.items.length === 0) {
    return (
      <div className="container mx-auto px-4 py-16 text-center">
        <h1 className="text-2xl font-bold mb-4">Your cart is empty</h1>
        <Link href="/products" className="text-blue-600 hover:underline">
          Continue shopping
        </Link>
      </div>
    );
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>

      <div className="grid lg:grid-cols-3 gap-8">
        <div className="lg:col-span-2 space-y-4">
          {cart.items.map((item) => (
            <CartItem
              key={item.id}
              item={item}
              onUpdateQuantity={(qty) => updateItem(item.id, qty)}
              onRemove={() => removeItem(item.id)}
            />
          ))}
        </div>

        <div>
          <OrderSummary cart={cart} />
          <Link
            href="/checkout"
            className="mt-4 w-full block text-center bg-black text-white py-3 rounded-lg hover:bg-gray-800"
          >
            Proceed to Checkout
          </Link>
        </div>
      </div>
    </div>
  );
}
```

### Checkout Page

```tsx
// app/checkout/page.tsx
'use client';

import { useState } from 'react';
import { useCart } from '@/hooks/useCart';
import { useCheckout } from '@/hooks/useCheckout';
import { AddressForm } from '@/components/commerce/AddressForm';
import { PaymentForm } from '@/components/commerce/PaymentForm';
import { OrderSummary } from '@/components/commerce/OrderSummary';

type Step = 'shipping' | 'payment' | 'review';

export default function CheckoutPage() {
  const [step, setStep] = useState<Step>('shipping');
  const { cart } = useCart();
  const {
    setShippingAddress,
    setPaymentMethod,
    completeCheckout,
    isProcessing
  } = useCheckout();

  if (!cart) {
    return <div>Loading...</div>;
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Checkout</h1>

      <div className="grid lg:grid-cols-3 gap-8">
        <div className="lg:col-span-2">
          {/* Progress Steps */}
          <div className="flex mb-8">
            {['shipping', 'payment', 'review'].map((s, i) => (
              <div key={s} className="flex-1 text-center">
                <div className={`w-8 h-8 mx-auto rounded-full flex items-center justify-center ${
                  step === s ? 'bg-black text-white' : 'bg-gray-200'
                }`}>
                  {i + 1}
                </div>
                <span className="text-sm mt-1 capitalize">{s}</span>
              </div>
            ))}
          </div>

          {step === 'shipping' && (
            <AddressForm
              onSubmit={(address) => {
                setShippingAddress(address);
                setStep('payment');
              }}
            />
          )}

          {step === 'payment' && (
            <PaymentForm
              onSubmit={(payment) => {
                setPaymentMethod(payment);
                setStep('review');
              }}
              onBack={() => setStep('shipping')}
            />
          )}

          {step === 'review' && (
            <div>
              <h2 className="text-xl font-bold mb-4">Review Your Order</h2>
              {/* Order review content */}
              <button
                onClick={completeCheckout}
                disabled={isProcessing}
                className="w-full bg-black text-white py-3 rounded-lg hover:bg-gray-800 disabled:opacity-50"
              >
                {isProcessing ? 'Processing...' : 'Place Order'}
              </button>
            </div>
          )}
        </div>

        <div>
          <OrderSummary cart={cart} />
        </div>
      </div>
    </div>
  );
}
```

## Component Templates

### ProductCard

```tsx
// components/commerce/ProductCard.tsx
import Image from 'next/image';
import Link from 'next/link';
import { Product } from '@stateset/embedded';

interface Props {
  product: Product;
}

export function ProductCard({ product }: Props) {
  const defaultVariant = product.variants?.find(v => v.isDefault) || product.variants?.[0];
  const price = defaultVariant?.price || 0;
  const compareAtPrice = defaultVariant?.compareAtPrice;

  return (
    <Link href={`/products/${product.slug}`} className="group">
      <div className="aspect-square relative overflow-hidden rounded-lg bg-gray-100">
        {product.imageUrl && (
          <Image
            src={product.imageUrl}
            alt={product.name}
            fill
            className="object-cover group-hover:scale-105 transition-transform"
          />
        )}
        {compareAtPrice && compareAtPrice > price && (
          <span className="absolute top-2 left-2 bg-red-500 text-white text-xs px-2 py-1 rounded">
            Sale
          </span>
        )}
      </div>

      <div className="mt-4">
        <h3 className="font-medium group-hover:underline">{product.name}</h3>
        <div className="flex items-center gap-2 mt-1">
          <span className="font-semibold">${price.toFixed(2)}</span>
          {compareAtPrice && compareAtPrice > price && (
            <span className="text-gray-500 line-through text-sm">
              ${compareAtPrice.toFixed(2)}
            </span>
          )}
        </div>
      </div>
    </Link>
  );
}
```

### AddToCartButton

```tsx
// components/commerce/AddToCartButton.tsx
'use client';

import { useState } from 'react';
import { useCart } from '@/hooks/useCart';

interface Props {
  productId: string;
  variantId?: string;
  quantity?: number;
  className?: string;
}

export function AddToCartButton({ productId, variantId, quantity = 1, className }: Props) {
  const { addItem } = useCart();
  const [isAdding, setIsAdding] = useState(false);

  const handleClick = async () => {
    setIsAdding(true);
    try {
      await addItem({ productId, variantId, quantity });
    } finally {
      setIsAdding(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={isAdding}
      className={`bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800 disabled:opacity-50 ${className}`}
    >
      {isAdding ? 'Adding...' : 'Add to Cart'}
    </button>
  );
}
```

## Hooks

### useCart Hook

```tsx
// hooks/useCart.ts
'use client';

import { useState, useEffect, useCallback } from 'react';
import { Cart, CartItem } from '@stateset/embedded';

const CART_ID_KEY = 'stateset_cart_id';

export function useCart() {
  const [cart, setCart] = useState<Cart | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Load cart on mount
  useEffect(() => {
    const cartId = localStorage.getItem(CART_ID_KEY);
    if (cartId) {
      fetchCart(cartId);
    } else {
      setIsLoading(false);
    }
  }, []);

  const fetchCart = async (cartId: string) => {
    try {
      const res = await fetch(`/api/commerce/carts/${cartId}`);
      if (res.ok) {
        const data = await res.json();
        setCart(data.cart);
      } else {
        localStorage.removeItem(CART_ID_KEY);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const createCart = async () => {
    const res = await fetch('/api/commerce/carts', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ currency: 'USD' }),
    });
    const data = await res.json();
    localStorage.setItem(CART_ID_KEY, data.cart.id);
    setCart(data.cart);
    return data.cart;
  };

  const addItem = useCallback(async (item: {
    productId: string;
    variantId?: string;
    quantity: number;
  }) => {
    let currentCart = cart;
    if (!currentCart) {
      currentCart = await createCart();
    }

    const res = await fetch(`/api/commerce/carts/${currentCart.id}/items`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(item),
    });
    const data = await res.json();
    setCart(data.cart);
  }, [cart]);

  const updateItem = useCallback(async (itemId: string, quantity: number) => {
    if (!cart) return;

    const res = await fetch(`/api/commerce/carts/${cart.id}/items/${itemId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ quantity }),
    });
    const data = await res.json();
    setCart(data.cart);
  }, [cart]);

  const removeItem = useCallback(async (itemId: string) => {
    if (!cart) return;

    const res = await fetch(`/api/commerce/carts/${cart.id}/items/${itemId}`, {
      method: 'DELETE',
    });
    const data = await res.json();
    setCart(data.cart);
  }, [cart]);

  const clearCart = useCallback(() => {
    localStorage.removeItem(CART_ID_KEY);
    setCart(null);
  }, []);

  return {
    cart,
    isLoading,
    itemCount: cart?.items?.length || 0,
    addItem,
    updateItem,
    removeItem,
    clearCart,
  };
}
```

## API Routes

### Commerce API Proxy

```typescript
// app/api/commerce/[...path]/route.ts
import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function GET(
  request: NextRequest,
  { params }: { params: { path: string[] } }
) {
  const commerce = getCommerce();
  const [resource, id, subResource] = params.path;

  try {
    switch (resource) {
      case 'products':
        if (id) {
          const product = await commerce.products.get(id);
          return NextResponse.json({ product });
        }
        const products = await commerce.products.list();
        return NextResponse.json(products);

      case 'carts':
        if (id) {
          const cart = await commerce.carts.get(id);
          return NextResponse.json({ cart });
        }
        break;

      case 'orders':
        if (id) {
          const order = await commerce.orders.get(id);
          return NextResponse.json({ order });
        }
        break;

      default:
        return NextResponse.json({ error: 'Not found' }, { status: 404 });
    }
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    );
  }
}

export async function POST(
  request: NextRequest,
  { params }: { params: { path: string[] } }
) {
  const commerce = getCommerce();
  const [resource, id, subResource] = params.path;
  const body = await request.json();

  try {
    switch (resource) {
      case 'carts':
        if (id && subResource === 'items') {
          // Add item to cart
          await commerce.carts.addItem(id, body);
          const cart = await commerce.carts.get(id);
          return NextResponse.json({ cart });
        }
        if (id && subResource === 'checkout') {
          // Complete checkout
          const result = await commerce.carts.complete(id);
          return NextResponse.json(result);
        }
        if (!id) {
          // Create cart
          const cart = await commerce.carts.create(body);
          return NextResponse.json({ cart });
        }
        break;

      default:
        return NextResponse.json({ error: 'Not found' }, { status: 404 });
    }
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    );
  }
}
```

## Configuration Files

### package.json

```json
{
  "name": "my-stateset-store",
  "version": "0.2.0",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "lint": "next lint",
    "seed": "node scripts/seed.js"
  },
  "dependencies": {
    "@stateset/embedded": "^0.2.0",
    "next": "14.0.0",
    "react": "^18",
    "react-dom": "^18"
  },
  "devDependencies": {
    "@types/node": "^20",
    "@types/react": "^18",
    "@types/react-dom": "^18",
    "autoprefixer": "^10",
    "postcss": "^8",
    "tailwindcss": "^3",
    "typescript": "^5"
  }
}
```

### tailwind.config.ts

```typescript
import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './pages/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#f0f9ff',
          500: '#0ea5e9',
          600: '#0284c7',
          700: '#0369a1',
        },
      },
    },
  },
  plugins: [],
};

export default config;
```

### next.config.js

```javascript
/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    domains: ['images.unsplash.com'], // Add your image domains
  },
  experimental: {
    serverComponentsExternalPackages: ['@stateset/embedded'],
  },
};

module.exports = nextConfig;
```

## Seed Script

```javascript
// scripts/seed.js
const { Commerce } = require('@stateset/embedded');

async function seed() {
  const commerce = new Commerce('./store.db');

  console.log('Seeding database...');

  // Create sample products
  const products = [
    {
      name: 'Classic T-Shirt',
      slug: 'classic-t-shirt',
      description: 'A comfortable cotton t-shirt for everyday wear.',
      variants: [
        { sku: 'TSHIRT-S', name: 'Small', price: 29.99 },
        { sku: 'TSHIRT-M', name: 'Medium', price: 29.99, isDefault: true },
        { sku: 'TSHIRT-L', name: 'Large', price: 29.99 },
      ],
    },
    {
      name: 'Premium Hoodie',
      slug: 'premium-hoodie',
      description: 'Stay warm with our premium fleece hoodie.',
      variants: [
        { sku: 'HOODIE-M', name: 'Medium', price: 79.99, isDefault: true },
        { sku: 'HOODIE-L', name: 'Large', price: 79.99 },
      ],
    },
  ];

  for (const product of products) {
    await commerce.products.create(product);
    console.log(`Created product: ${product.name}`);
  }

  // Create inventory
  for (const product of products) {
    for (const variant of product.variants) {
      await commerce.inventory.createItem({
        sku: variant.sku,
        name: `${product.name} - ${variant.name}`,
        initialQuantity: 100,
      });
      console.log(`Created inventory: ${variant.sku}`);
    }
  }

  console.log('Seeding complete!');
}

seed().catch(console.error);
```

## Best Practices

### Performance

1. **Use Server Components** - Fetch data on server when possible
2. **Implement caching** - Cache product data with revalidation
3. **Lazy load images** - Use Next.js Image component
4. **Code splitting** - Dynamic imports for heavy components

### Security

1. **Validate inputs** - Server-side validation for all API routes
2. **Sanitize data** - Prevent XSS in user content
3. **Rate limiting** - Protect API routes from abuse
4. **HTTPS only** - Enforce secure connections

### SEO

1. **Metadata** - Product titles, descriptions, OpenGraph
2. **Structured data** - JSON-LD for products
3. **Sitemap** - Generate for all products
4. **Canonical URLs** - Prevent duplicate content

### Accessibility

1. **ARIA labels** - For interactive elements
2. **Keyboard navigation** - Full keyboard support
3. **Focus management** - Proper focus handling
4. **Color contrast** - WCAG compliant colors

## Common Integrations

### Payment Providers

- Stripe Elements for card input
- PayPal SDK for PayPal checkout
- Coinbase Commerce for crypto

### Shipping

- ShipEngine for rate calculation
- EasyPost for label generation
- Shippo for multi-carrier

### Analytics

- Google Analytics 4
- Plausible Analytics
- PostHog for product analytics

### Email

- SendGrid for transactional
- Resend for modern email
- Postmark for deliverability
