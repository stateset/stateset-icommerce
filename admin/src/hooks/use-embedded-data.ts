'use client';

import { useState, useEffect, useCallback, useRef } from 'react';

export interface UseEmbeddedDataResult<T> {
  data: T | null;
  isLoading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  mutate: (newData: T) => void;
}

export function useEmbeddedData<T>(
  fetcher: () => Promise<T>,
  options?: {
    refreshInterval?: number;
    initialData?: T;
    enabled?: boolean;
  },
): UseEmbeddedDataResult<T> {
  const { refreshInterval, initialData, enabled = true } = options || {};

  const [data, setData] = useState<T | null>(initialData || null);
  const [isLoading, setIsLoading] = useState(!initialData && enabled);
  const [error, setError] = useState<Error | null>(null);

  // Consumers typically pass inline arrow fetchers whose identity changes on
  // every render. Track the latest one in a ref so the effects below only
  // re-fire on genuine signal changes (enabled / refreshInterval), not on
  // fetcher identity churn — which would cause continuous refetch loops.
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const fetch = useCallback(async () => {
    if (!enabled) return;

    setIsLoading(true);
    setError(null);

    try {
      const result = await fetcherRef.current();
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setIsLoading(false);
    }
  }, [enabled]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  useEffect(() => {
    if (refreshInterval && refreshInterval > 0 && enabled) {
      const interval = setInterval(fetch, refreshInterval);
      return () => clearInterval(interval);
    }
  }, [refreshInterval, fetch, enabled]);

  const mutate = useCallback((newData: T) => {
    setData(newData);
  }, []);

  return {
    data,
    isLoading,
    error,
    refetch: fetch,
    mutate,
  };
}

// Hook for paginated data
export function useEmbeddedPaginatedData<T>(
  fetcher: (params: { limit: number; offset: number }) => Promise<{ items: T[]; total: number }>,
  options?: {
    pageSize?: number;
    enabled?: boolean;
  },
) {
  const { pageSize = 20, enabled = true } = options || {};

  const [items, setItems] = useState<T[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [isLoading, setIsLoading] = useState(enabled);
  const [error, setError] = useState<Error | null>(null);

  // Same identity-churn guard as useEmbeddedData: only page/pageSize/enabled
  // changes should trigger a refetch, never a new inline fetcher reference.
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const fetch = useCallback(async () => {
    if (!enabled) return;

    setIsLoading(true);
    setError(null);

    try {
      const offset = (page - 1) * pageSize;
      const result = await fetcherRef.current({ limit: pageSize, offset });
      setItems(result.items);
      setTotal(result.total);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setIsLoading(false);
    }
  }, [page, pageSize, enabled]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  const totalPages = Math.ceil(total / pageSize);

  return {
    items,
    total,
    page,
    pageSize,
    totalPages,
    isLoading,
    error,
    setPage,
    refetch: fetch,
    hasNextPage: page < totalPages,
    hasPrevPage: page > 1,
    nextPage: () => setPage((p) => Math.min(p + 1, totalPages)),
    prevPage: () => setPage((p) => Math.max(p - 1, 1)),
  };
}
