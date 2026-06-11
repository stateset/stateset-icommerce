/**
 * Tests for the organizations server action auth guard
 *
 * Server actions bypass the API middleware, so `listOrganizations` in
 * `@/app/actions/organizations` must enforce the admin session itself via
 * `requireAdminSession()`. These tests lock down that contract plus the
 * dev-org env-var parsing behind the guard.
 *
 * @module tests/unit/app/actions/organizations
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { ADMIN_SESSION_COOKIE } from '@/lib/shared/auth-session';

// Mock next/headers cookies() — must be before imports
const cookieStore = vi.hoisted(() => new Map<string, { value: string }>());
vi.mock('next/headers', () => ({
  cookies: vi.fn(() =>
    Promise.resolve({
      get: (name: string) => cookieStore.get(name),
      set: (name: string, value: string, _opts?: unknown) => {
        cookieStore.set(name, { value });
      },
      delete: (name: string) => {
        cookieStore.delete(name);
      },
    })
  ),
}));

import { listOrganizations } from '@/app/actions/organizations';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('organizations action auth guard', () => {
  describe('without a session', () => {
    it('rejects listOrganizations', async () => {
      await expect(listOrganizations()).rejects.toMatchObject(UNAUTHORIZED);
    });

    it('ignores a whitespace-only session cookie', async () => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: '   ' });
      await expect(listOrganizations()).rejects.toMatchObject(UNAUTHORIZED);
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('returns an empty list by default (production default)', async () => {
      await expect(listOrganizations()).resolves.toEqual([]);
    });

    it('parses NEXT_PUBLIC_ADMIN_DEV_ORGS entries with optional display names', async () => {
      vi.stubEnv('NEXT_PUBLIC_ADMIN_DEV_ORGS', 'acme, globex:Globex Corp');

      await expect(listOrganizations()).resolves.toEqual([
        { id: 'acme', name: 'acme' },
        { id: 'globex', name: 'Globex Corp' },
      ]);
    });
  });

  describe('when admin auth is disabled (dev mode)', () => {
    it('skips the session requirement, mirroring the middleware bypass', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      await expect(listOrganizations()).resolves.toEqual([]);
    });

    it('still requires a session in production even with the flag set', async () => {
      vi.stubEnv('NODE_ENV', 'production');
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      await expect(listOrganizations()).rejects.toMatchObject(UNAUTHORIZED);
    });
  });
});
