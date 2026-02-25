/**
 * Scaffold Templates & File Generators for StateSet Storefront Creation
 *
 * Contains all template constants (TEMPLATES, PAGE_TEMPLATES, COMPONENT_TEMPLATES)
 * and file-generation functions used by the scaffolding MCP server.
 *
 * @module scaffold-templates
 */

// ============================================================================
// Project Templates
// ============================================================================

export const TEMPLATES = {
  nextjs: {
    name: 'Next.js 14 Storefront',
    description: 'Full-stack React storefront with App Router, SSR, and Tailwind CSS',
    framework: 'next',
    features: ['ssr', 'api-routes', 'tailwind', 'typescript'],
  },
  'nextjs-minimal': {
    name: 'Next.js Minimal',
    description: 'Minimal Next.js storefront with basic pages',
    framework: 'next',
    features: ['typescript'],
  },
  'vite-react': {
    name: 'Vite + React SPA',
    description: 'Client-side React SPA using WASM bindings',
    framework: 'vite',
    features: ['wasm', 'tailwind', 'typescript'],
  },
  astro: {
    name: 'Astro Storefront',
    description: 'Static-first storefront with Islands architecture',
    framework: 'astro',
    features: ['static', 'islands', 'tailwind'],
  },
};

export const PAGE_TEMPLATES = {
  'product-listing': {
    name: 'Product Listing Page',
    description: 'Grid of products with filters and pagination',
    path: 'app/products/page.tsx',
  },
  'product-detail': {
    name: 'Product Detail Page',
    description: 'Single product view with add to cart',
    path: 'app/products/[slug]/page.tsx',
  },
  cart: {
    name: 'Shopping Cart Page',
    description: 'Cart with items, quantities, and totals',
    path: 'app/cart/page.tsx',
  },
  checkout: {
    name: 'Checkout Page',
    description: 'Multi-step checkout flow',
    path: 'app/checkout/page.tsx',
  },
  account: {
    name: 'Account Dashboard',
    description: 'Customer account overview',
    path: 'app/account/page.tsx',
  },
  orders: {
    name: 'Order History',
    description: 'List of customer orders',
    path: 'app/account/orders/page.tsx',
  },
};

export const COMPONENT_TEMPLATES = {
  'product-card': {
    name: 'ProductCard',
    description: 'Product card for grid display',
    path: 'components/commerce/ProductCard.tsx',
  },
  'product-grid': {
    name: 'ProductGrid',
    description: 'Responsive grid of product cards',
    path: 'components/commerce/ProductGrid.tsx',
  },
  'cart-drawer': {
    name: 'CartDrawer',
    description: 'Slide-out cart drawer',
    path: 'components/commerce/CartDrawer.tsx',
  },
  'add-to-cart': {
    name: 'AddToCartButton',
    description: 'Add to cart button with loading state',
    path: 'components/commerce/AddToCartButton.tsx',
  },
  'checkout-form': {
    name: 'CheckoutForm',
    description: 'Multi-step checkout form',
    path: 'components/commerce/CheckoutForm.tsx',
  },
  header: {
    name: 'Header',
    description: 'Site header with navigation and cart',
    path: 'components/layout/Header.tsx',
  },
  footer: {
    name: 'Footer',
    description: 'Site footer with links',
    path: 'components/layout/Footer.tsx',
  },
};

// ============================================================================
// File Generation Functions
// ============================================================================

export function createPackageJson(name, template, features) {
  const base = {
    name: name.toLowerCase().replace(/[^a-z0-9-]/g, '-'),
    version: '0.7.7',
    private: true,
    scripts: {
      dev: 'next dev',
      build: 'next build',
      start: 'next start',
      lint: 'next lint',
      seed: 'node scripts/seed.js',
    },
    dependencies: {
      '@stateset/embedded': '^0.7.7',
      next: '14.0.0',
      react: '^18',
      'react-dom': '^18',
    },
    devDependencies: {
      '@types/node': '^20',
      '@types/react': '^18',
      '@types/react-dom': '^18',
      typescript: '^5',
    },
  };

  if (TEMPLATES[template]?.features.includes('tailwind') || features.includes('tailwind')) {
    base.devDependencies.autoprefixer = '^10';
    base.devDependencies.postcss = '^8';
    base.devDependencies.tailwindcss = '^3';
  }

  return base;
}

export function createTsConfig(_template) {
  return JSON.stringify(
    {
      compilerOptions: {
        target: 'es5',
        lib: ['dom', 'dom.iterable', 'esnext'],
        allowJs: true,
        skipLibCheck: true,
        strict: true,
        noEmit: true,
        esModuleInterop: true,
        module: 'esnext',
        moduleResolution: 'bundler',
        resolveJsonModule: true,
        isolatedModules: true,
        jsx: 'preserve',
        incremental: true,
        plugins: [{ name: 'next' }],
        paths: { '@/*': ['./*'] },
      },
      include: ['next-env.d.ts', '**/*.ts', '**/*.tsx', '.next/types/**/*.ts'],
      exclude: ['node_modules'],
    },
    null,
    2,
  );
}

export function createNextConfig() {
  return `/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    remotePatterns: [
      { protocol: 'https', hostname: 'images.unsplash.com' },
    ],
  },
  experimental: {
    serverComponentsExternalPackages: ['@stateset/embedded'],
  },
};

module.exports = nextConfig;
`;
}

export function createTailwindConfig() {
  return `import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './pages/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
`;
}

export function createPostCssConfig() {
  return `module.exports = {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
`;
}

export function createCommerceLib() {
  return `import { Commerce } from '@stateset/embedded';

let commerce: Commerce | null = null;

export function getCommerce(): Commerce {
  if (!commerce) {
    commerce = new Commerce(process.env.DATABASE_PATH || './store.db');
  }
  return commerce;
}

export async function getProducts(options?: { limit?: number; offset?: number }) {
  const commerce = getCommerce();
  return commerce.products.list(options);
}

export async function getProduct(id: string) {
  const commerce = getCommerce();
  return commerce.products.get(id);
}

export async function getProductBySlug(slug: string) {
  const commerce = getCommerce();
  const products = await commerce.products.list();
  return products.products?.find(p => p.slug === slug);
}
`;
}

export function createRootLayout(name) {
  return `import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: '${name}',
  description: 'Powered by StateSet iCommerce',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className={inter.className}>
        <header className="border-b">
          <nav className="container mx-auto px-4 py-4 flex justify-between items-center">
            <a href="/" className="text-xl font-bold">${name}</a>
            <div className="flex gap-4">
              <a href="/products" className="hover:underline">Products</a>
              <a href="/cart" className="hover:underline">Cart</a>
            </div>
          </nav>
        </header>
        <main>{children}</main>
        <footer className="border-t mt-16">
          <div className="container mx-auto px-4 py-8 text-center text-gray-500">
            Powered by StateSet iCommerce
          </div>
        </footer>
      </body>
    </html>
  );
}
`;
}

export function createHomePage() {
  return `import { getProducts } from '@/lib/commerce';
import Link from 'next/link';

export default async function HomePage() {
  const { products } = await getProducts({ limit: 8 });

  return (
    <div className="container mx-auto px-4 py-16">
      <section className="text-center mb-16">
        <h1 className="text-4xl font-bold mb-4">Welcome to Our Store</h1>
        <p className="text-gray-600 mb-8">Discover our amazing products</p>
        <Link
          href="/products"
          className="bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800"
        >
          Shop Now
        </Link>
      </section>

      <section>
        <h2 className="text-2xl font-bold mb-8">Featured Products</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {products?.map((product) => (
            <Link
              key={product.id}
              href={\`/products/\${product.slug || product.id}\`}
              className="group"
            >
              <div className="aspect-square bg-gray-100 rounded-lg mb-4" />
              <h3 className="font-medium group-hover:underline">{product.name}</h3>
              <p className="text-gray-600">
                {product.variants?.[0]?.price ? \`$\${product.variants[0].price.toFixed(2)}\` : 'Price TBD'}
              </p>
            </Link>
          ))}
        </div>
      </section>
    </div>
  );
}
`;
}

export function createGlobalStyles() {
  return `@tailwind base;
@tailwind components;
@tailwind utilities;

:root {
  --foreground-rgb: 0, 0, 0;
  --background-rgb: 255, 255, 255;
}

body {
  color: rgb(var(--foreground-rgb));
  background: rgb(var(--background-rgb));
}
`;
}

export function createGitignore() {
  return `# Dependencies
node_modules/
.pnp
.pnp.js

# Next.js
.next/
out/

# Production
build/
dist/

# Database
*.db
*.sqlite
*.sqlite3

# Environment
.env
.env.local
.env.development.local
.env.test.local
.env.production.local

# Debug
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
`;
}

export function createEnvLocal() {
  return `# Database
DATABASE_PATH=./store.db

# Add your environment variables here
`;
}

export function createReadme(name, template) {
  return `# ${name}

A commerce storefront built with StateSet iCommerce engine.

## Getting Started

1. Install dependencies:
   \`\`\`bash
   npm install
   \`\`\`

2. Seed the database (optional):
   \`\`\`bash
   npm run seed
   \`\`\`

3. Start the development server:
   \`\`\`bash
   npm run dev
   \`\`\`

4. Open [http://localhost:3000](http://localhost:3000)

## Stack

- **Framework:** ${TEMPLATES[template]?.name || 'Next.js'}
- **Commerce:** StateSet iCommerce (@stateset/embedded)
- **Database:** SQLite (embedded)
- **Styling:** Tailwind CSS

## Project Structure

\`\`\`
${name}/
├── app/                  # Next.js App Router
│   ├── products/        # Product pages
│   ├── cart/            # Shopping cart
│   ├── checkout/        # Checkout flow
│   └── api/             # API routes
├── components/          # React components
│   ├── commerce/        # Commerce components
│   └── ui/              # UI components
├── lib/                 # Utilities
│   └── commerce.ts      # StateSet client
├── hooks/               # React hooks
└── store.db             # SQLite database
\`\`\`

## Learn More

- [StateSet Documentation](https://docs.stateset.io)
- [Next.js Documentation](https://nextjs.org/docs)
`;
}

export function generatePageContent(pageType, customName) {
  const templates = {
    'product-listing': `import { getProducts } from '@/lib/commerce';

export default async function ProductsPage() {
  const { products } = await getProducts();

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">All Products</h1>
      <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-6">
        {products?.map((product) => (
          <a
            key={product.id}
            href={\`/products/\${product.slug || product.id}\`}
            className="group"
          >
            <div className="aspect-square bg-gray-100 rounded-lg mb-4" />
            <h3 className="font-medium group-hover:underline">{product.name}</h3>
          </a>
        ))}
      </div>
    </div>
  );
}
`,
    'product-detail': `import { getProductBySlug } from '@/lib/commerce';
import { notFound } from 'next/navigation';

interface Props {
  params: { slug: string };
}

export default async function ProductPage({ params }: Props) {
  const product = await getProductBySlug(params.slug);

  if (!product) {
    notFound();
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="grid md:grid-cols-2 gap-8">
        <div className="aspect-square bg-gray-100 rounded-lg" />
        <div>
          <h1 className="text-3xl font-bold">{product.name}</h1>
          <p className="text-2xl font-semibold mt-4">
            {product.variants?.[0]?.price?.toFixed(2) || 'Price TBD'}
          </p>
          <p className="mt-4 text-gray-600">{product.description}</p>
          <button className="mt-6 bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800">
            Add to Cart
          </button>
        </div>
      </div>
    </div>
  );
}
`,
    cart: `'use client';

export default function CartPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>
      <p className="text-gray-600">Your cart is empty.</p>
    </div>
  );
}
`,
    checkout: `'use client';

export default function CheckoutPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Checkout</h1>
      {/* Checkout form */}
    </div>
  );
}
`,
    account: `export default function AccountPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">My Account</h1>
    </div>
  );
}
`,
    orders: `export default function OrdersPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Order History</h1>
    </div>
  );
}
`,
  };

  return (
    templates[pageType] ||
    `export default function ${customName || 'Page'}() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">${customName || 'Page'}</h1>
    </div>
  );
}
`
  );
}

export function generateComponentContent(componentType, customName) {
  const templates = {
    'product-card': `import Link from 'next/link';

interface ProductCardProps {
  product: {
    id: string;
    name: string;
    slug?: string;
    imageUrl?: string;
    variants?: Array<{ price?: number }>;
  };
}

export function ProductCard({ product }: ProductCardProps) {
  const price = product.variants?.[0]?.price;

  return (
    <Link href={\`/products/\${product.slug || product.id}\`} className="group">
      <div className="aspect-square bg-gray-100 rounded-lg overflow-hidden mb-4">
        {product.imageUrl && (
          <img
            src={product.imageUrl}
            alt={product.name}
            className="w-full h-full object-cover group-hover:scale-105 transition-transform"
          />
        )}
      </div>
      <h3 className="font-medium group-hover:underline">{product.name}</h3>
      {price && <p className="text-gray-600">\${price.toFixed(2)}</p>}
    </Link>
  );
}
`,
    'add-to-cart': `'use client';

import { useState } from 'react';

interface AddToCartButtonProps {
  productId: string;
  variantId?: string;
  className?: string;
}

export function AddToCartButton({ productId, variantId, className }: AddToCartButtonProps) {
  const [isAdding, setIsAdding] = useState(false);

  const handleClick = async () => {
    setIsAdding(true);
    try {
      // Add to cart logic here
      await new Promise(resolve => setTimeout(resolve, 500));
    } finally {
      setIsAdding(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={isAdding}
      className={\`bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800 disabled:opacity-50 \${className}\`}
    >
      {isAdding ? 'Adding...' : 'Add to Cart'}
    </button>
  );
}
`,
    header: `import Link from 'next/link';

export function Header() {
  return (
    <header className="border-b">
      <nav className="container mx-auto px-4 py-4 flex justify-between items-center">
        <Link href="/" className="text-xl font-bold">Store</Link>
        <div className="flex gap-4">
          <Link href="/products" className="hover:underline">Products</Link>
          <Link href="/cart" className="hover:underline">Cart</Link>
        </div>
      </nav>
    </header>
  );
}
`,
    footer: `export function Footer() {
  return (
    <footer className="border-t mt-16">
      <div className="container mx-auto px-4 py-8 text-center text-gray-500">
        <p>Powered by StateSet iCommerce</p>
      </div>
    </footer>
  );
}
`,
  };

  return (
    templates[componentType] ||
    `interface ${customName || 'Component'}Props {}

export function ${customName || 'Component'}({}: ${customName || 'Component'}Props) {
  return (
    <div>
      {/* ${customName || 'Component'} */}
    </div>
  );
}
`
  );
}

export function generateHookContent(hookName, customName) {
  const templates = {
    useCart: `'use client';

import { useState, useEffect, useCallback } from 'react';

const CART_ID_KEY = 'stateset_cart_id';

export function useCart() {
  const [cart, setCart] = useState(null);
  const [isLoading, setIsLoading] = useState(true);

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
      const res = await fetch(\`/api/carts/\${cartId}\`);
      if (res.ok) {
        const data = await res.json();
        setCart(data.cart);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const addItem = useCallback(async (item: any) => {
    // Implementation
  }, [cart]);

  const removeItem = useCallback(async (itemId: string) => {
    // Implementation
  }, [cart]);

  return {
    cart,
    isLoading,
    itemCount: cart?.items?.length || 0,
    addItem,
    removeItem,
  };
}
`,
    useProducts: `'use client';

import { useState, useEffect } from 'react';

export function useProducts(options?: { limit?: number }) {
  const [products, setProducts] = useState([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    const fetchProducts = async () => {
      try {
        const res = await fetch('/api/products');
        const data = await res.json();
        setProducts(data.products || []);
      } catch (err) {
        setError(err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchProducts();
  }, [options?.limit]);

  return { products, isLoading, error };
}
`,
  };

  return (
    templates[hookName] ||
    `'use client';

import { useState } from 'react';

export function ${customName || hookName}() {
  const [state, setState] = useState(null);

  return { state };
}
`
  );
}

export function generateApiRouteContent(routePath, methods) {
  const normalizedRoutePath = String(routePath || '').replace(/^\/+|\/+$/g, '');
  const handlers = methods
    .map(
      (method) =>
        `export async function ${method}(request: NextRequest) {\n  return routeRequest(request, '${method.toUpperCase()}');\n}`,
    )
    .join('\n\n');

  return `import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

const ROUTE_PATH = '${normalizedRoutePath}';

type RouteErrorPayload = {
  error: string;
  details?: unknown;
};

const toPascalCase = (value: string) =>
  value
    .split(/[-_/]/g)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join('');

const toCamelCase = (value: string) => {
  const [first, ...rest] = toPascalCase(value).split(/(?=[A-Z])/g);
  return first.toLowerCase() + rest.join('');
};

const resolveResourceManager = (commerce: ReturnType<typeof getCommerce>, route: string) => {
  const segments = route
    .split('/')
    .map((segment) => segment.trim())
    .filter(Boolean)
    .map((segment) => segment.replace(/\\{\\{.*?\\}\\}/g, '').trim())
    .filter(Boolean);
  const resource = segments[segments.length - 1] || route;
  const direct = commerce?.[resource];
  if (direct) return direct;

  const singular = resource.endsWith('s') ? resource.slice(0, -1) : resource;
  if (commerce?.[singular]) return commerce[singular];
  if (commerce?.[toCamelCase(resource)]) return commerce[toCamelCase(resource)];
  if (commerce?.[toCamelCase(singular)]) return commerce[toCamelCase(singular)];
  return null;
};

const parseEntityId = (request: NextRequest) => {
  const url = new URL(request.url);
  const searchParams = url.searchParams;
  return (
    searchParams.get('id') ||
    searchParams.get('slug') ||
    searchParams.get('sku') ||
    searchParams.get('key') ||
    null
  );
};

const parseBody = async (request: NextRequest) => {
  try {
    const contentType = request.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      return await request.json();
    }
    return {};
  } catch (err) {
    console.debug('[scaffold-templates] Request body parse failed:', err.message || err);
    return {};
  }
};

const methodUnavailable = (method: string) => \`Method \${method} is not supported for this generated route.\`;

const formatError = (error: unknown, fallback: string, status = 500): NextResponse<RouteErrorPayload> =>
  NextResponse.json(
    {
      error: error instanceof Error ? error.message : fallback,
      details: error instanceof Error ? undefined : error,
    },
    { status },
  );

const routeRequest = async (request: NextRequest, method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE') => {
  const commerce = getCommerce();
  const resource = resolveResourceManager(commerce, ROUTE_PATH);

  if (!resource || typeof resource !== 'object') {
    return NextResponse.json(
      {
        error: \`No Commerce resource found for route: ${normalizedRoutePath}\`,
      },
      { status: 400 },
    );
  }

  try {
    const id = parseEntityId(request);
    const payload = ['POST', 'PUT', 'PATCH'].includes(method) ? await parseBody(request) : {};

    if (method === 'GET') {
      if (id) {
        const getMethod = resource.get || resource.getById || resource.findById;
        if (!getMethod) {
          return NextResponse.json(
            {
              error: methodUnavailable('GET'),
            },
            { status: 405 },
          );
        }
        const data = await getMethod(id);
        return NextResponse.json({ data });
      }

      const listMethod = resource.list || resource.find || resource.findMany;
      if (!listMethod) {
        return NextResponse.json(
          {
            error: methodUnavailable('GET'),
          },
          { status: 405 },
        );
      }
      const data = await listMethod();
      return NextResponse.json({ data });
    }

    if (method === 'POST') {
      if (!resource.create) {
        return NextResponse.json(
          {
            error: methodUnavailable('POST'),
          },
          { status: 405 },
        );
      }
      const data = await resource.create(payload);
      return NextResponse.json({ data, entity: toCamelCase(ROUTE_PATH) }, { status: 201 });
    }

    if (method === 'PUT' || method === 'PATCH') {
      const updateId = id || payload.id || payload.entityId || payload.orderId || payload.productId;
      if (!updateId) {
        return NextResponse.json(
          { error: 'Missing id for update request' },
          { status: 400 },
        );
      }

      const updateMethod = resource.update || resource.save || resource.patch;
      if (!updateMethod) {
        return NextResponse.json(
          {
            error: methodUnavailable(method),
          },
          { status: 405 },
        );
      }

      const data = await updateMethod(updateId, payload);
      return NextResponse.json({ data });
    }

    if (method === 'DELETE') {
      const deleteMethod = resource.delete || resource.remove || resource.destroy;
      if (!deleteMethod) {
        return NextResponse.json(
          {
            error: methodUnavailable('DELETE'),
          },
          { status: 405 },
        );
      }
      if (!id && !payload.id) {
        return NextResponse.json({ error: 'Missing id for delete request' }, { status: 400 });
      }

      const targetId = id || payload.id;
      const data = await deleteMethod(targetId);
      return NextResponse.json({ data });
    }

    return NextResponse.json({ error: 'Unsupported method ' + method }, { status: 405 });
  } catch (error) {
    return formatError(error, \`Failed to handle ${'${method}'} ${normalizedRoutePath}\`, 500);
  }
};

${handlers}
`;
}

export function generateSeedScript(dbPath, productCount) {
  return `const { Commerce } = require('@stateset/embedded');

async function seed() {
  const commerce = new Commerce('${dbPath}');

  console.log('Seeding database...');

  const products = [
    { name: 'Classic T-Shirt', slug: 'classic-t-shirt', price: 29.99 },
    { name: 'Premium Hoodie', slug: 'premium-hoodie', price: 79.99 },
    { name: 'Canvas Sneakers', slug: 'canvas-sneakers', price: 59.99 },
    { name: 'Leather Wallet', slug: 'leather-wallet', price: 49.99 },
    { name: 'Wireless Earbuds', slug: 'wireless-earbuds', price: 99.99 },
    { name: 'Smart Watch', slug: 'smart-watch', price: 199.99 },
    { name: 'Backpack', slug: 'backpack', price: 89.99 },
    { name: 'Sunglasses', slug: 'sunglasses', price: 149.99 },
    { name: 'Water Bottle', slug: 'water-bottle', price: 24.99 },
    { name: 'Notebook Set', slug: 'notebook-set', price: 19.99 },
  ].slice(0, ${productCount});

  for (const product of products) {
    try {
      await commerce.products.create({
        name: product.name,
        slug: product.slug,
        description: \`High-quality \${product.name.toLowerCase()}\`,
        variants: [
          {
            sku: product.slug.toUpperCase(),
            name: 'Default',
            price: product.price,
            isDefault: true,
          },
        ],
      });

      await commerce.inventory.createItem({
        sku: product.slug.toUpperCase(),
        name: product.name,
        initialQuantity: 100,
      });

      console.log(\`Created: \${product.name}\`);
    } catch (err) {
      console.error(\`Error creating \${product.name}:\`, err.message);
    }
  }

  console.log('Seeding complete!');
}

seed().catch(console.error);
`;
}
