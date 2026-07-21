'use client';

import React from 'react';
import { ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { Button } from './button';

interface ErrorBoundaryProps {
  children: React.ReactNode;
  fallback?: React.ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Error caught by boundary:', error, errorInfo);

    // Report to observability endpoint
    this.reportError(error, errorInfo);
  }

  private getReportUrl(): string | null {
    if (typeof window === 'undefined' || !window.location?.href) {
      return null;
    }

    try {
      return new URL('/api/health', window.location.href).toString();
    } catch {
      return null;
    }
  }

  private async reportError(error: Error, _errorInfo: React.ErrorInfo) {
    const reportUrl = this.getReportUrl();
    if (!reportUrl) {
      return;
    }

    try {
      await fetch(reportUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          type: 'client_error',
          message: error.message,
          url: typeof window !== 'undefined' ? window.location.href : '',
          timestamp: new Date().toISOString(),
        }),
      });
    } catch {
      // Best-effort reporting only; the original error is already logged above.
    }
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div
          role="alert"
          className="flex flex-col items-center justify-center min-h-[200px] p-6 rounded-lg border border-ds-status-fail/25 bg-ds-status-fail/10"
        >
          <ExclamationTriangleIcon className="w-12 h-12 text-ds-status-fail mb-4" />
          <h3 className="text-lg font-semibold text-ds-status-fail mb-2">Something went wrong</h3>
          <p className="text-sm text-ds-status-fail/90 mb-4 text-center max-w-md">
            {this.state.error?.message || 'An unexpected error occurred'}
          </p>
          <Button
            variant="outline"
            onClick={() => this.setState({ hasError: false, error: undefined })}
          >
            Try again
          </Button>
        </div>
      );
    }

    return this.props.children;
  }
}

interface ErrorDisplayProps {
  error: Error | string;
  onRetry?: () => void;
}

export function ErrorDisplay({ error, onRetry }: ErrorDisplayProps) {
  const message = error instanceof Error ? error.message : error;

  return (
    <div
      role="alert"
      className="flex flex-col items-center justify-center min-h-[200px] p-6 rounded-lg border border-ds-status-fail/25 bg-ds-status-fail/10"
    >
      <ExclamationTriangleIcon className="w-12 h-12 text-ds-status-fail mb-4" />
      <h3 className="text-lg font-semibold text-ds-status-fail mb-2">Error</h3>
      <p className="text-sm text-ds-status-fail/90 mb-4 text-center max-w-md">{message}</p>
      {onRetry && (
        <Button variant="outline" onClick={onRetry}>
          Retry
        </Button>
      )}
    </div>
  );
}
