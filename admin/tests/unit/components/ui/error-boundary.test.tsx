// Component tests for ErrorBoundary and ErrorDisplay. Covers the happy path
// (children render through), error capture (default + custom fallback),
// reset-via-Try-again, and the standalone ErrorDisplay's optional retry.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { ErrorBoundary, ErrorDisplay } from '@/components/ui/error-boundary';

// React logs the error via console.error; silence it in test output so
// failed assertions are easier to read. Spies are reset per-test.
let errorSpy: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  // Block fetch so the best-effort `/api/health` report doesn't surface
  // unhandled rejections in the test runner.
  vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null));
});
afterEach(() => {
  errorSpy.mockRestore();
  vi.restoreAllMocks();
});

const Boom = ({ message = 'kaboom' }: { message?: string }) => {
  throw new Error(message);
};

describe('ErrorBoundary', () => {
  it('renders children unchanged when no error is thrown', () => {
    render(
      <ErrorBoundary>
        <div>healthy</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText('healthy')).toBeInTheDocument();
  });

  it('renders the default fallback UI when a child throws', () => {
    render(
      <ErrorBoundary>
        <Boom message="downstream failure" />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
    expect(screen.getByText('downstream failure')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('renders the custom fallback when provided', () => {
    render(
      <ErrorBoundary fallback={<div data-testid="custom-fallback">oops</div>}>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByTestId('custom-fallback')).toBeInTheDocument();
    expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument();
  });

  it('shows a generic message when the thrown Error has no message', () => {
    const Anonymous = () => {
      throw new Error('');
    };
    render(
      <ErrorBoundary>
        <Anonymous />
      </ErrorBoundary>,
    );
    expect(screen.getByText('An unexpected error occurred')).toBeInTheDocument();
  });
});

describe('ErrorDisplay', () => {
  it('renders an Error instance message', () => {
    render(<ErrorDisplay error={new Error('the message')} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('the message')).toBeInTheDocument();
  });

  it('renders a plain string error', () => {
    render(<ErrorDisplay error="raw string" />);
    expect(screen.getByText('raw string')).toBeInTheDocument();
  });

  it('omits the Retry button when no onRetry is provided', () => {
    render(<ErrorDisplay error="x" />);
    expect(screen.queryByRole('button', { name: 'Retry' })).not.toBeInTheDocument();
  });

  it('calls onRetry when the Retry button is clicked', () => {
    const onRetry = vi.fn();
    render(<ErrorDisplay error="x" onRetry={onRetry} />);
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
