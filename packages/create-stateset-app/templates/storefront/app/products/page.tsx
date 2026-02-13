import { getProducts } from '@/lib/commerce';
import { ProductSearchClient } from '@/components/commerce/ProductSearchClient';

export default async function ProductsPage() {
  const { products } = await getProducts();

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">All Products</h1>
      <ProductSearchClient initialProducts={products || []} />
    </div>
  );
}
