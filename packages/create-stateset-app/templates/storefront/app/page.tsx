import { getProducts } from '@/lib/commerce';
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
          {products?.map((product: any) => (
            <Link
              key={product.id}
              href={`/products/${product.slug || product.id}`}
              className="group"
            >
              <div className="aspect-square bg-gray-100 rounded-lg mb-4" />
              <h3 className="font-medium group-hover:underline">{product.name}</h3>
              <p className="text-gray-600">
                {product.variants?.[0]?.price ? `$${product.variants[0].price.toFixed(2)}` : 'Price TBD'}
              </p>
            </Link>
          ))}
        </div>
      </section>
    </div>
  );
}
