'use client';

import { useQuery } from '@tanstack/react-query';

interface UseProductsOptions {
  search?: string;
  category?: string;
  limit?: number;
}

export function useProducts({ search, category, limit = 20 }: UseProductsOptions = {}) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['products', search, category, limit],
    queryFn: async () => {
      const params = new URLSearchParams({ limit: String(limit) });
      if (search) params.set('search', search);
      if (category) params.set('category', category);
      const response = await fetch(`/api/products?${params}`);
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || 'Failed to load products');
      return data;
    },
    staleTime: 60_000,
    enabled: !!(search || category),
  });

  return {
    products: data?.products || [],
    isLoading,
    error,
  };
}
