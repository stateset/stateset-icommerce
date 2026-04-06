/**
 * Tests for ErrorBoundary and ErrorDisplay components
 *
 * @module tests/unit/components/error-boundary
 */

import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ErrorBoundary, ErrorDisplay } from '@/components/ui/error-boundary';

// Mock heroicons
vi.mock('@heroicons/react/24/outline', () => ({
  ExclamationTriangleIcon: (props: React.SVGProps<SVGSVGElement>) =>
    React.createElement('svg', { ...props, 'data-testid': 'warning-icon' }),
}));

// Mock the Button component
vi.mock('@/components/ui/button', () => ({
  Button: ({
    children,
    onClick,
    variant,
    ...rest
  }: {
    children: React.ReactNode;
    onClick?: () => void;
    variant?: string;
  }) =>
    React.createElement(
      'button',
      { onClick, 'data-variant': variant, ...rest },
      children
    ),
}));

// Suppress console.error output from ErrorBoundary's componentDidCatch
beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// A component that throws an error for testing ErrorBoundary
function ThrowingComponent({ message }: { message: string }) {
  throw new Error(message);
  return null;
}

// A component that renders normally
function GoodComponent() {
  return React.createElement('div', { 'data-testid': 'good-child' }, 'Hello World');
}

describe('ErrorBoundary', () => {
  // Suppress unhandled error warnings from React during tests
  let originalOnError: typeof window.onerror;

  beforeEach(() => {
    originalOnError = window.onerror;
    window.onerror = () => true;
  });

  afterEach(() => {
    window.onerror = originalOnError;
  });

  describe('when no error occurs', () => {
    it('renders children normally', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(GoodComponent)
        )
      );

      expect(screen.getByTestId('good-child')).toBeDefined();
      expect(screen.getByText('Hello World')).toBeDefined();
    });

    it('does not show error UI', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(GoodComponent)
        )
      );

      expect(screen.queryByText('Something went wrong')).toBeNull();
    });
  });

  describe('when an error occurs', () => {
    it('renders default error UI', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, { message: 'Test crash' })
        )
      );

      expect(screen.getByRole('alert')).toBeDefined();
      expect(screen.getByText('Something went wrong')).toBeDefined();
    });

    it('displays the error message', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, {
            message: 'Specific failure reason',
          })
        )
      );

      expect(screen.getByText('Specific failure reason')).toBeDefined();
    });

    it('renders custom fallback when provided', () => {
      const fallback = React.createElement(
        'div',
        { 'data-testid': 'custom-fallback' },
        'Custom error UI'
      );

      render(
        React.createElement(
          ErrorBoundary,
          {
            fallback,
            children: React.createElement(ThrowingComponent, { message: 'Crash' }),
          }
        )
      );

      expect(screen.getByTestId('custom-fallback')).toBeDefined();
      expect(screen.getByText('Custom error UI')).toBeDefined();
      // Default error UI should not be shown
      expect(screen.queryByText('Something went wrong')).toBeNull();
    });

    it('shows a Try again button in default error UI', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, { message: 'Oops' })
        )
      );

      expect(screen.getByText('Try again')).toBeDefined();
    });

    it('reports error to console', () => {
      const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, { message: 'Error to log' })
        )
      );

      expect(consoleErrorSpy).toHaveBeenCalled();
      // componentDidCatch calls console.error with the error
      const calls = consoleErrorSpy.mock.calls;
      const hasErrorLog = calls.some(
        (call) =>
          typeof call[0] === 'string' &&
          call[0].includes('Error caught by boundary')
      );
      expect(hasErrorLog).toBe(true);
    });

    it('renders alert role for accessibility', () => {
      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, { message: 'Crash' })
        )
      );

      expect(screen.getByRole('alert')).toBeDefined();
    });

    it('reports errors to the health endpoint using an absolute URL', async () => {
      const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 202 }));
      vi.stubGlobal('fetch', fetchMock);

      render(
        React.createElement(
          ErrorBoundary,
          null,
          React.createElement(ThrowingComponent, { message: 'Report me' })
        )
      );

      await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
      expect(String(fetchMock.mock.calls[0]?.[0] ?? '')).toMatch(/\/api\/health$/);
    });
  });
});

describe('ErrorDisplay', () => {
  it('renders error message from Error object', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: new Error('Something failed'),
      })
    );

    expect(screen.getByText('Something failed')).toBeDefined();
  });

  it('renders error message from string', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: 'String error message',
      })
    );

    expect(screen.getByText('String error message')).toBeDefined();
  });

  it('displays the "Error" heading', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: 'Test error',
      })
    );

    expect(screen.getByText('Error')).toBeDefined();
  });

  it('renders alert role for accessibility', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: 'Test',
      })
    );

    expect(screen.getByRole('alert')).toBeDefined();
  });

  it('shows Retry button when onRetry is provided', () => {
    const onRetry = vi.fn();

    render(
      React.createElement(ErrorDisplay, {
        error: 'Failed',
        onRetry,
      })
    );

    const retryButton = screen.getByText('Retry');
    expect(retryButton).toBeDefined();
  });

  it('calls onRetry when Retry button is clicked', () => {
    const onRetry = vi.fn();

    render(
      React.createElement(ErrorDisplay, {
        error: 'Failed',
        onRetry,
      })
    );

    fireEvent.click(screen.getByText('Retry'));

    expect(onRetry).toHaveBeenCalledOnce();
  });

  it('does not show Retry button when onRetry is not provided', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: 'No retry',
      })
    );

    expect(screen.queryByText('Retry')).toBeNull();
  });

  it('renders the warning icon', () => {
    render(
      React.createElement(ErrorDisplay, {
        error: 'Icon test',
      })
    );

    expect(screen.getByTestId('warning-icon')).toBeDefined();
  });
});
