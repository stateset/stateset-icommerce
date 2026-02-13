'use client';

import { useQuery } from '@tanstack/react-query';
import { getProducts } from '@/lib/commerce';

interface UseProductsOptions {
  search?: string;
  category?: string;
  limit?: number;
}

export function useProducts({ search, category, limit = 20 }: UseProductsOptions = {}) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['products', search, category, limit],
    queryFn: () => getProducts({ search, category, limit }),
    staleTime: 60_000,
    enabled: !!(search || category),
  });

  return {
    products: data?.products || [],
    isLoading,
    error,
  };
}
