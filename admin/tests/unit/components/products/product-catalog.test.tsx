import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import ProductCatalog from '@/components/products/product-catalog';
import type { Product } from '@/lib/types';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getProducts: vi.fn(),
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
    tr: ({ children, ...props }: HTMLAttributes<HTMLTableRowElement>) => (
      <tr {...props}>{children}</tr>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    BarChart: ({ data }: { data: unknown[] }) => <div data-testid="bar-chart">{data.length}</div>,
  };
});

afterEach(() => {
  vi.clearAllMocks();
});

const sampleProduct: Product = {
  id: 'prod-1',
  sku: 'SKU-001',
  name: 'Trail Backpack',
  description: 'Backpack',
  price: 129.99,
  currency: 'USD',
  category: 'Outdoor',
  tags: ['Backpacks'],
  status: 'active',
  images: [],
  variants: [
    {
      id: 'variant-1',
      sku: 'SKU-001-A',
      name: 'Default',
      price: 129.99,
      options: {},
      inventoryQuantity: 24,
    },
  ],
  createdAt: '2026-03-01T00:00:00.000Z',
  updatedAt: '2026-03-02T00:00:00.000Z',
};

describe('ProductCatalog', () => {
  it('shows an explicit empty state instead of demo catalog data', () => {
    useEmbeddedDataMock.mockReturnValue({ data: [], isLoading: false, error: null });

    render(<ProductCatalog />);

    expect(screen.getByText('No products found')).toBeTruthy();
    expect(
      screen.getByText('No products have been synced into the embedded catalog yet.'),
    ).toBeTruthy();
    expect(
      screen.getByText('Inventory rankings will appear after products are synced.'),
    ).toBeTruthy();
    expect(screen.queryByText('Wireless Bluetooth Headphones')).toBeNull();
  });

  it('renders live product data when products are available', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: [sampleProduct],
      isLoading: false,
      error: null,
    });

    render(<ProductCatalog />);

    expect(screen.getAllByText('Trail Backpack').length).toBeGreaterThan(0);
    expect(screen.getAllByText('SKU-001').length).toBeGreaterThan(0);
    expect(screen.queryByText('No products found')).toBeNull();
  });
});
