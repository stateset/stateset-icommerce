import { getProducts } from '@/lib/commerce';
import Link from 'next/link';
import { ProductCard } from '@/components/commerce/ProductCard';
import { Suspense } from 'react';

async function FeaturedProducts() {
  const { products } = await getProducts({ limit: 4 });

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
      {products?.map((product: any) => (
        <ProductCard key={product.id} product={product} />
      ))}
    </div>
  );
}

const CATEGORIES = [
  { slug: 'apparel', name: 'Apparel', description: 'Comfortable everyday essentials' },
  { slug: 'electronics', name: 'Electronics', description: 'Smart tech for modern life' },
  { slug: 'accessories', name: 'Accessories', description: 'Complete your look' },
  { slug: 'lifestyle', name: 'Lifestyle', description: 'Everyday carry essentials' },
];

export default function CollectionsPage() {
  return (
    <div className="container mx-auto px-4 py-16">
      <h1 className="text-4xl font-bold mb-12 text-center">Collections</h1>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8 mb-16">
        {CATEGORIES.map((category) => (
          <Link
            key={category.slug}
            href={`/products?category=${category.slug}`}
            className="group relative rounded-2xl overflow-hidden bg-gray-100 aspect-[4/3] flex items-center justify-center hover:shadow-xl transition-shadow"
          >
            <div className="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent" />
            <div className="relative text-center text-white p-8">
              <h2 className="text-3xl font-bold mb-2">{category.name}</h2>
              <p className="opacity-90">{category.description}</p>
              <span className="inline-block mt-4 text-sm font-semibold group-hover:underline">
                Shop Collection &rarr;
              </span>
            </div>
          </Link>
        ))}
      </div>

      <section>
        <div className="flex justify-between items-center mb-8">
          <h2 className="text-2xl font-bold">All Products</h2>
          <Link
            href="/products"
            className="text-blue-600 hover:text-blue-800 font-medium"
          >
            View All &rarr;
          </Link>
        </div>
        <Suspense fallback={<div>Loading products...</div>}>
          <FeaturedProducts />
        </Suspense>
      </section>
    </div>
  );
}
