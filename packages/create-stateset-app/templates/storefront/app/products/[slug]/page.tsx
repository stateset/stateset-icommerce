import { getProductBySlug } from '@/lib/commerce';
import { notFound } from 'next/navigation';
import { ProductPurchaseOptions } from '@/components/commerce/ProductPurchaseOptions';

interface Props {
  params: Promise<{ slug: string }>;
}

export default async function ProductPage({ params }: Props) {
  const { slug } = await params;
  const product = await getProductBySlug(slug);

  if (!product) {
    notFound();
  }

  const sku = product.variants?.[0]?.sku || slug.toUpperCase();
  const price = product.variants?.[0]?.price;

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="grid md:grid-cols-2 gap-8">
        <div className="aspect-square bg-gray-100 rounded-lg" />
        <div>
          <h1 className="text-3xl font-bold">{product.name}</h1>
          <p className="mt-4 text-gray-600">{product.description}</p>
          <p className="mt-2 text-sm text-gray-400">SKU: {sku}</p>
          {price ? (
            <div className="mt-6">
              <ProductPurchaseOptions
                sku={sku}
                productName={product.name}
                price={price}
              />
            </div>
          ) : (
            <p className="mt-4 text-gray-500">Price TBD</p>
          )}
        </div>
      </div>
    </div>
  );
}
