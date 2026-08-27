'use client';

import { useState } from 'react';
import { useProducts } from '@/hooks/useProducts';
import { ProductCard } from './ProductCard';

interface Props {
  initialProducts: any[];
  categories?: { slug: string; name: string }[];
  initialCategory?: string;
}

export function ProductSearchClient({
  initialProducts,
  categories = [],
  initialCategory = '',
}: Props) {
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState(initialCategory);
  const { products, isLoading } = useProducts({ search, category });

  const displayProducts =
    search || (category && category !== initialCategory) ? products : initialProducts;

  return (
    <div>
      <div className="flex flex-col md:flex-row gap-4 mb-8">
        <input
          type="text"
          placeholder="Search products..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="flex-1 px-4 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
        <select
          value={category}
          onChange={(e) => setCategory(e.target.value)}
          className="px-4 py-2 border rounded-lg bg-white focus:ring-2 focus:ring-blue-500"
        >
          <option value="">All Categories</option>
          {categories.map((cat) => (
            <option key={cat.slug} value={cat.slug}>
              {cat.name}
            </option>
          ))}
        </select>
      </div>

      {isLoading ? (
        <p className="text-gray-500 text-center py-8">Loading...</p>
      ) : displayProducts.length === 0 ? (
        <p className="text-gray-500 text-center py-8">No products found.</p>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {displayProducts.map((product: any) => (
            <ProductCard key={product.id} product={product} />
          ))}
        </div>
      )}
    </div>
  );
}
