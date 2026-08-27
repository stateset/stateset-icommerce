import { getProducts } from '@/lib/commerce';
import { ProductSearchClient } from '@/components/commerce/ProductSearchClient';

const categories = ['apparel', 'electronics', 'accessories', 'lifestyle'].map((slug) => ({
  slug,
  name: slug[0].toUpperCase() + slug.slice(1),
}));

export default async function ProductsPage({
  searchParams,
}: {
  searchParams: Promise<{ category?: string }>;
}) {
  const { category = '' } = await searchParams;
  const { products } = await getProducts({ category: category || undefined });

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">All Products</h1>
      <ProductSearchClient
        initialProducts={products || []}
        categories={categories}
        initialCategory={category}
      />
    </div>
  );
}
