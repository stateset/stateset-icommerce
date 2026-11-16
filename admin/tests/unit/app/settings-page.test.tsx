// The settings page's engine status reads the system health snapshot. The
// native engine has no such aggregate (getSystemHealth throws a capability
// error), so the page must say the figures are unavailable rather than show
// "0ms", "0%" and "1 connection(s)" beside a "Healthy" pill.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getSystemHealth: vi.fn(),
}));

vi.mock('@/components/ui/error-boundary', () => ({
  ErrorBoundary: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
}));

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
  },
}));

import SettingsPage from '@/app/settings/page';

afterEach(() => {
  vi.clearAllMocks();
});

describe('SettingsPage engine status', () => {
  it('shows the health figures as unavailable without a snapshot', () => {
    useEmbeddedDataMock.mockReturnValue({ data: null, isLoading: false, error: new Error('x') });
    render(<SettingsPage />);

    // Latency, error rate and connections.
    expect(screen.getAllByText('—')).toHaveLength(3);
    expect(screen.queryByText('0ms')).toBeNull();
    expect(screen.queryByText('0%')).toBeNull();
    expect(screen.queryByText('1 connection(s)')).toBeNull();
    expect(screen.queryByText('Healthy')).toBeNull();
    expect(screen.getByText('Unavailable')).toBeInTheDocument();
  });

  it('renders a snapshot, zeros included', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: {
        databaseLatency: 0.48,
        errorRate: 0,
        activeConnections: 0,
        queueDepth: 6,
        processingSpeed: 1240,
      },
      isLoading: false,
      error: null,
    });
    render(<SettingsPage />);

    expect(screen.getByText('0.48ms')).toBeInTheDocument();
    expect(screen.getByText('0%')).toBeInTheDocument();
    expect(screen.getByText('0 connection(s)')).toBeInTheDocument();
    expect(screen.getByText('Healthy')).toBeInTheDocument();
  });
});
