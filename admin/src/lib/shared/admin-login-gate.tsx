'use client';

import { useEffect, useState } from 'react';
import { Card, Input, Button } from '@stateset/design';

interface LoginResponse {
  success: boolean;
  data?: {
    token?: string;
    user?: {
      id?: string;
      email?: string;
    };
  };
  error?: {
    message?: string;
    code?: string;
  };
}

export function AdminLoginGate() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [csrfToken, setCsrfToken] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const loadCsrfToken = async () => {
      try {
        const response = await fetch('/api/auth/csrf-token', {
          credentials: 'same-origin',
        });
        const payload = (await response.json()) as {
          success?: boolean;
          data?: { csrfToken?: string };
        };
        if (!cancelled && response.ok && payload?.data?.csrfToken) {
          setCsrfToken(payload.data.csrfToken);
        }
      } catch {
        if (!cancelled) {
          setError('Unable to prepare sign-in. Refresh and try again.');
        }
      }
    };

    void loadCsrfToken();

    return () => {
      cancelled = true;
    };
  }, []);

  const onSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsLoading(true);
    setError(null);

    try {
      const response = await fetch('/api/auth/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'x-csrf-token': csrfToken,
        },
        credentials: 'same-origin',
        body: JSON.stringify({ email, password }),
      });

      const payload = (await response.json()) as LoginResponse;
      if (!response.ok || !payload.success) {
        setError(payload.error?.message || 'Sign-in failed');
        return;
      }

      window.location.reload();
    } catch {
      setError('Unable to sign in. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Card className="w-full max-w-md p-8">
      <div className="mb-6 space-y-2">
        <h1 className="font-ds-display text-2xl font-semibold tracking-ds-tight text-ds-foreground">
          Sign in to StateSet Admin
        </h1>
        <p className="text-sm text-ds-muted-foreground">
          A server-side session cookie is required before the admin dashboard will load.
        </p>
      </div>

      <form className="space-y-4" onSubmit={onSubmit}>
        <Input
          label="Email"
          autoComplete="email"
          onChange={(event) => setEmail(event.target.value)}
          required
          type="email"
          value={email}
        />

        <Input
          label="Password"
          autoComplete="current-password"
          onChange={(event) => setPassword(event.target.value)}
          required
          type="password"
          value={password}
        />

        {error ? (
          <p className="rounded-lg border border-ds-destructive/30 bg-ds-destructive/10 px-3 py-2 text-sm text-ds-destructive">
            {error}
          </p>
        ) : null}

        <Button className="w-full" disabled={isLoading || !csrfToken} type="submit">
          {isLoading ? 'Signing in...' : 'Sign in'}
        </Button>
      </form>
    </Card>
  );
}
