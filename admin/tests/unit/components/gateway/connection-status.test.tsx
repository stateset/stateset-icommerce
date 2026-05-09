// Component test for the Gateway connection-status pill. The component
// derives its three states (connecting / online / offline) from the
// useEmbeddedData hook result, so we mock the hook directly.

import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const useEmbeddedDataMock = vi.fn();
vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/lib/gateway-client', () => ({
  getGatewayHealth: vi.fn(),
}));

import { GatewayConnectionStatus } from '@/components/gateway/connection-status';

afterEach(() => {
  vi.clearAllMocks();
});

describe('GatewayConnectionStatus', () => {
  it('shows Connecting… while the fetch is in flight', () => {
    useEmbeddedDataMock.mockReturnValue({ data: undefined, error: null, isLoading: true });
    render(<GatewayConnectionStatus />);
    expect(screen.getByText('Connecting...')).toBeInTheDocument();
  });

  it('shows the gateway version when status is ok', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { status: 'ok', version: '1.2.3', subsystems: {} },
      error: null,
      isLoading: false,
    });
    render(<GatewayConnectionStatus />);
    expect(screen.getByText('Gateway v1.2.3')).toBeInTheDocument();
  });

  it('shows Gateway Offline when an error is reported', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: undefined,
      error: new Error('connect ECONNREFUSED'),
      isLoading: false,
    });
    render(<GatewayConnectionStatus />);
    expect(screen.getByText('Gateway Offline')).toBeInTheDocument();
  });

  it('shows Gateway Offline when status is not ok', () => {
    useEmbeddedDataMock.mockReturnValue({
      data: { status: 'degraded', version: '1.2.3', subsystems: {} },
      error: null,
      isLoading: false,
    });
    render(<GatewayConnectionStatus />);
    expect(screen.getByText('Gateway Offline')).toBeInTheDocument();
  });
});
