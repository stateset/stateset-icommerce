// Component test for ReturnsManagement. The component reads `returns`,
// `analytics`, `pipeline` from the data payload — we cover the error
// path and a minimal happy path.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getReturnsManagementData: vi.fn(),
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
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    ProgressBar: Wrapper,
    DonutChart: Chart,
  };
});

import ReturnsManagement from '@/components/returns/returns-management';

afterEach(() => {
  vi.clearAllMocks();
});

describe('ReturnsManagement', () => {
  it('renders the error card on hook failure', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, isLoading: false, error: new Error('x') });
    render(<ReturnsManagement />);
    expect(screen.getByText('Failed to load returns data')).toBeInTheDocument();
  });

  it('renders the summary metrics with empty pipeline / returns', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        returns: [],
        analytics: {
          totalReturns: 0,
          returnRate: 0,
          refundTotal: 0,
          avgProcessingDays: 0,
          returnsByReason: {},
        },
        pipeline: [],
      },
      isLoading: false,
      error: null,
    });
    render(<ReturnsManagement />);
    expect(screen.getByText('Total Returns')).toBeInTheDocument();
    expect(screen.getByText('Return Rate')).toBeInTheDocument();
  });
});
