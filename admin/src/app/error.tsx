'use client';

// Root app-router error boundary. Catches errors thrown from server
// components and server actions (including guarded actions that throw
// AppError.unauthorized — without this boundary those surface as opaque
// masked-digest crashes) and renders a recoverable fallback instead.

import { useEffect } from 'react';
import { ExclamationTriangleIcon } from '@heroicons/react/24/outline';
import { Button } from '@/components/ui/button';

interface RootErrorProps {
  error: Error & { digest?: string };
  reset: () => void;
}

export default function RootError({ error, reset }: RootErrorProps) {
  useEffect(() => {
    // Server errors arrive with their message masked behind a digest;
    // log what we have so the digest can be correlated with server logs.
    console.error('Route error boundary caught:', error);
  }, [error]);

  return (
    <div className="flex min-h-[60vh] items-center justify-center p-6">
      <div
        role="alert"
        className="flex w-full max-w-md flex-col items-center rounded-lg border border-ds-status-fail/25 bg-ds-status-fail/10 p-6 text-center"
      >
        <ExclamationTriangleIcon className="mb-4 h-12 w-12 text-ds-status-fail" />
        <h2 className="mb-2 text-lg font-semibold text-ds-foreground">
          Something went wrong
        </h2>
        <p className="mb-4 text-sm text-ds-status-fail">
          {error.message || 'An unexpected error occurred while loading this page.'}
        </p>
        {error.digest && (
          <p className="mb-4 text-xs text-ds-muted-foreground">
            Reference: <code>{error.digest}</code>
          </p>
        )}
        <Button variant="outline" onClick={() => reset()}>
          Try again
        </Button>
      </div>
    </div>
  );
}
