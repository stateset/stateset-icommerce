/**
 * Tests for the active-org server actions auth guard
 *
 * Server actions bypass the API middleware, so `setActiveOrg` and
 * `clearActiveOrg` in `@/app/actions/active-org` must enforce the admin
 * session themselves via `requireAdminSession()`. These tests lock down
 * that contract, plus the org-id validation behind the guard.
 *
 * @module tests/unit/app/actions/active-org
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { ADMIN_SESSION_COOKIE } from '@/lib/shared/auth-session';
import { ACTIVE_ORG_COOKIE } from '@/lib/shared/active-org';

// Mock next/headers cookies() — must be before imports. `setActiveOrg`
// uses the object form `set({ name, value, ... })`, so the mock accepts
// both the object and (name, value) signatures.
const cookieStore = vi.hoisted(() => new Map<string, { value: string }>());
vi.mock('next/headers', () => ({
  cookies: vi.fn(() =>
    Promise.resolve({
      get: (name: string) => cookieStore.get(name),
      set: (
        nameOrSpec: string | { name: string; value: string },
        value?: string
      ) => {
        if (typeof nameOrSpec === 'string') {
          cookieStore.set(nameOrSpec, { value: value ?? '' });
        } else {
          cookieStore.set(nameOrSpec.name, { value: nameOrSpec.value });
        }
      },
      delete: (name: string) => {
        cookieStore.delete(name);
      },
    })
  ),
}));

import { setActiveOrg, clearActiveOrg } from '@/app/actions/active-org';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('active-org actions auth guard', () => {
  describe('without a session', () => {
    it('rejects setActiveOrg and never writes the cookie', async () => {
      await expect(setActiveOrg('acme')).rejects.toMatchObject(UNAUTHORIZED);
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });

    it('rejects clearActiveOrg and leaves the cookie intact', async () => {
      cookieStore.set(ACTIVE_ORG_COOKIE, { value: 'acme' });

      await expect(clearActiveOrg()).rejects.toMatchObject(UNAUTHORIZED);
      expect(cookieStore.get(ACTIVE_ORG_COOKIE)).toEqual({ value: 'acme' });
    });

    it('rejects before validating the orgId (auth comes first)', async () => {
      await expect(setActiveOrg('not valid!')).rejects.toMatchObject(UNAUTHORIZED);
    });

    it('ignores a whitespace-only session cookie', async () => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: '   ' });

      await expect(setActiveOrg('acme')).rejects.toMatchObject(UNAUTHORIZED);
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('allows setActiveOrg to write the cookie', async () => {
      await setActiveOrg('acme');
      expect(cookieStore.get(ACTIVE_ORG_COOKIE)).toEqual({ value: 'acme' });
    });

    it('still rejects an invalid orgId', async () => {
      await expect(setActiveOrg('not valid!')).rejects.toThrow(/Invalid orgId/);
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });

    it('allows clearActiveOrg to delete the cookie', async () => {
      cookieStore.set(ACTIVE_ORG_COOKIE, { value: 'acme' });

      await clearActiveOrg();
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });
  });

  describe('when admin auth is disabled (dev mode)', () => {
    it('skips the session requirement, mirroring the middleware bypass', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      await setActiveOrg('globex');
      expect(cookieStore.get(ACTIVE_ORG_COOKIE)).toEqual({ value: 'globex' });

      await clearActiveOrg();
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });

    it('still requires a session in production even with the flag set', async () => {
      vi.stubEnv('NODE_ENV', 'production');
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      await expect(setActiveOrg('acme')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(clearActiveOrg()).rejects.toMatchObject(UNAUTHORIZED);
      expect(cookieStore.has(ACTIVE_ORG_COOKIE)).toBe(false);
    });
  });
});
