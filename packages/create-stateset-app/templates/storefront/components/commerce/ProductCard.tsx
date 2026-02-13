import Link from 'next/link';

interface Props {
  product: {
    id: string;
    name: string;
    slug?: string;
    description?: string;
    variants?: { price?: number; sku?: string }[];
  };
}

export function ProductCard({ product }: Props) {
  const price = product.variants?.[0]?.price;
  const slug = product.slug || product.name.toLowerCase().replace(/\s+/g, '-');

  return (
    <Link
      href={`/products/${slug}`}
      className="group block rounded-lg overflow-hidden border hover:shadow-lg transition-shadow"
    >
      <div className="aspect-square bg-gray-100 group-hover:bg-gray-200 transition-colors" />
      <div className="p-4">
        <h3 className="font-semibold group-hover:underline">{product.name}</h3>
        {product.description && (
          <p className="text-sm text-gray-600 mt-1 line-clamp-2">
            {product.description}
          </p>
        )}
        {price !== undefined && (
          <p className="mt-2 font-bold">${price.toFixed(2)}</p>
        )}
      </div>
    </Link>
  );
}
