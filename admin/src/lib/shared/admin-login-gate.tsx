'use client';

import { useEffect, useState } from 'react';

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
    <div className="w-full max-w-md rounded-2xl border border-gray-200 bg-white p-8 shadow-sm dark:border-gray-800 dark:bg-gray-900">
      <div className="mb-6 space-y-2">
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">
          Sign in to StateSet Admin
        </h1>
        <p className="text-sm text-gray-600 dark:text-gray-400">
          A server-side session cookie is required before the admin dashboard will load.
        </p>
      </div>

      <form className="space-y-4" onSubmit={onSubmit}>
        <label className="block space-y-1.5">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Email</span>
          <input
            autoComplete="email"
            className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none ring-0 transition focus:border-indigo-500 dark:border-gray-700 dark:bg-gray-950 dark:text-white"
            onChange={(event) => setEmail(event.target.value)}
            required
            type="email"
            value={email}
          />
        </label>

        <label className="block space-y-1.5">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Password</span>
          <input
            autoComplete="current-password"
            className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none ring-0 transition focus:border-indigo-500 dark:border-gray-700 dark:bg-gray-950 dark:text-white"
            onChange={(event) => setPassword(event.target.value)}
            required
            type="password"
            value={password}
          />
        </label>

        {error ? (
          <p className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900/50 dark:bg-red-950/40 dark:text-red-300">
            {error}
          </p>
        ) : null}

        <button
          className="w-full rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isLoading || !csrfToken}
          type="submit"
        >
          {isLoading ? 'Signing in...' : 'Sign in'}
        </button>
      </form>
    </div>
  );
}
