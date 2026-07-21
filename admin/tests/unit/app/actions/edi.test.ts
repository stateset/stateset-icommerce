/**
 * Tests for the EDI operations server actions.
 *
 * Server actions bypass the API middleware, so every exported action in
 * `@/app/actions/edi` must enforce the admin session itself via
 * `requireAdminSession()`. Read-only slice: list, get, summary, page data.
 *
 * @module tests/unit/app/actions/edi
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

// Mock the embedded engine wrappers. Enumerate named exports explicitly
// (a Proxy-based mock makes the module look thenable and hangs the import).
vi.mock('@/lib/embedded', () => ({
  ediDocumentsApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    summary: vi.fn().mockResolvedValue({ total: 0, byStatus: [], byType: [] }),
  },
}));

import { getEdiDocuments, getEdiDocument, getEdiSummary, getEdiPageData } from '@/app/actions/edi';
import { ediDocumentsApi } from '@/lib/embedded';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('edi actions auth guard', () => {
  describe('without a session', () => {
    it('rejects every EDI read and never reaches the embedded engine', async () => {
      await expect(getEdiDocuments()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getEdiDocument('edi_1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getEdiSummary()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getEdiPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(ediDocumentsApi.list).not.toHaveBeenCalled();
      expect(ediDocumentsApi.get).not.toHaveBeenCalled();
      expect(ediDocumentsApi.summary).not.toHaveBeenCalled();
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('passes the filter through to the list API', async () => {
      await expect(getEdiDocuments()).resolves.toEqual([]);
      expect(ediDocumentsApi.list).toHaveBeenCalledWith(undefined);

      await getEdiDocuments({ status: 'error', documentType: '850' });
      expect(ediDocumentsApi.list).toHaveBeenCalledWith({ status: 'error', documentType: '850' });
    });

    it('rejects an empty document id before touching the engine', async () => {
      await expect(getEdiDocument('   ')).rejects.toThrow(/required/);
      expect(ediDocumentsApi.get).not.toHaveBeenCalled();
      await getEdiDocument('edi_1');
      expect(ediDocumentsApi.get).toHaveBeenCalledWith('edi_1');
    });

    it('aggregates documents + summary for the operations page', async () => {
      const result = await getEdiPageData();
      expect(result).toEqual({
        documents: [],
        summary: { total: 0, byStatus: [], byType: [] },
      });
      expect(ediDocumentsApi.list).toHaveBeenCalled();
      expect(ediDocumentsApi.summary).toHaveBeenCalled();
    });
  });

  describe('when admin auth is disabled (dev mode)', () => {
    it('skips the session requirement, mirroring the middleware bypass', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
      await expect(getEdiSummary()).resolves.toEqual({ total: 0, byStatus: [], byType: [] });
      expect(ediDocumentsApi.summary).toHaveBeenCalled();
    });

    it('still requires a session in production even with the flag set', async () => {
      vi.stubEnv('NODE_ENV', 'production');
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
      await expect(getEdiSummary()).rejects.toMatchObject(UNAUTHORIZED);
      expect(ediDocumentsApi.summary).not.toHaveBeenCalled();
    });
  });
});
