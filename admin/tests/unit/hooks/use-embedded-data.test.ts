/**
 * Tests for useEmbeddedData hook
 * @module tests/unit/hooks/use-embedded-data
 */

import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useEmbeddedData } from '@/hooks/use-embedded-data';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('useEmbeddedData', () => {
  it('returns loading state initially', () => {
    const fetcher = vi.fn(() => new Promise<string>(() => {}));
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('resolves with data after fetch', async () => {
    const fetcher = vi.fn().mockResolvedValue({ items: [1, 2, 3] });
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toEqual({ items: [1, 2, 3] });
    expect(result.current.error).toBeNull();
  });

  it('handles errors gracefully', async () => {
    const fetcher = vi.fn().mockRejectedValue(new Error('Fetch failed'));
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    await waitFor(() => {
      expect(result.current.error).toBeInstanceOf(Error);
    });

    expect(result.current.isLoading).toBe(false);
    expect(result.current.data).toBeNull();
    expect(result.current.error?.message).toBe('Fetch failed');
  });

  it('wraps non-Error thrown values in Error', async () => {
    const fetcher = vi.fn().mockRejectedValue('string error');
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    await waitFor(() => {
      expect(result.current.error).toBeInstanceOf(Error);
    });

    expect(result.current.error?.message).toBe('string error');
  });

  it('refreshInterval triggers periodic refetch', async () => {
    vi.useFakeTimers();
    const fetcher = vi.fn().mockResolvedValue('data');

    renderHook(() => useEmbeddedData(fetcher, { refreshInterval: 5000 }));

    expect(fetcher).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it('refetch() re-runs fetcher', async () => {
    let callCount = 0;
    const fetcher = vi.fn().mockImplementation(async () => {
      callCount++;
      return `data-${callCount}`;
    });
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    await waitFor(() => {
      expect(result.current.data).toBe('data-1');
    });

    await act(async () => {
      await result.current.refetch();
    });

    expect(result.current.data).toBe('data-2');
  });

  it('mutate() updates data locally', async () => {
    const fetcher = vi.fn().mockResolvedValue('original');
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    await waitFor(() => {
      expect(result.current.data).toBe('original');
    });

    act(() => {
      result.current.mutate('mutated');
    });

    expect(result.current.data).toBe('mutated');
  });

  it('does not fetch when enabled is false', () => {
    const fetcher = vi.fn().mockResolvedValue('data');
    const { result } = renderHook(() =>
      useEmbeddedData(fetcher, { enabled: false })
    );

    expect(fetcher).not.toHaveBeenCalled();
    expect(result.current.isLoading).toBe(false);
    expect(result.current.data).toBeNull();
  });

  it('uses initialData when provided', async () => {
    const fetcher = vi.fn().mockResolvedValue('fresh');
    const { result } = renderHook(() =>
      useEmbeddedData(fetcher, { initialData: 'initial' })
    );

    expect(result.current.data).toBe('initial');
    expect(result.current.error).toBeNull();

    await waitFor(() => {
      expect(result.current.data).toBe('fresh');
    });

    expect(result.current.isLoading).toBe(false);
  });

  it('returns a refetch function', () => {
    const fetcher = vi.fn(() => new Promise<string>(() => {}));
    const { result } = renderHook(() => useEmbeddedData(fetcher));

    expect(typeof result.current.refetch).toBe('function');
    expect(typeof result.current.mutate).toBe('function');
  });

  it('fetches once even when the fetcher identity changes on every render', async () => {
    const fetchSpy = vi.fn().mockResolvedValue('data');
    // Dashboard consumers pass inline arrow fetchers, so every render hands
    // the hook a brand-new function reference.
    const { result, rerender } = renderHook(() => useEmbeddedData(() => fetchSpy()));

    await waitFor(() => {
      expect(result.current.data).toBe('data');
    });
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    for (let i = 0; i < 5; i++) {
      rerender();
    }
    await act(async () => {});

    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('keeps refreshInterval cadence despite unstable fetcher identity', async () => {
    vi.useFakeTimers();
    const fetchSpy = vi.fn().mockResolvedValue('data');
    const { rerender } = renderHook(() =>
      useEmbeddedData(() => fetchSpy(), { refreshInterval: 5000 })
    );

    expect(fetchSpy).toHaveBeenCalledTimes(1);

    // Re-renders alone must not trigger extra fetches or reset the interval.
    rerender();
    rerender();
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(fetchSpy).toHaveBeenCalledTimes(2);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(fetchSpy).toHaveBeenCalledTimes(3);
  });

  it('refetch() uses the latest fetcher passed on the most recent render', async () => {
    const { result, rerender } = renderHook(
      ({ value }: { value: string }) => useEmbeddedData(() => Promise.resolve(value)),
      { initialProps: { value: 'first' } }
    );

    await waitFor(() => {
      expect(result.current.data).toBe('first');
    });

    rerender({ value: 'second' });
    await act(async () => {});
    // Identity churn alone must not refetch...
    expect(result.current.data).toBe('first');

    // ...but a manual refetch sees the freshest fetcher.
    await act(async () => {
      await result.current.refetch();
    });
    expect(result.current.data).toBe('second');
  });
});
