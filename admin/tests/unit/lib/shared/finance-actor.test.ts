import { beforeEach, afterEach, it, expect, vi } from 'vitest';
vi.mock('server-only', () => ({}));
const auth = vi.hoisted(() => ({ session: vi.fn(), disabled: vi.fn() }));
vi.mock('@/lib/shared/auth-session', () => ({
  requireAdminSession: auth.session,
  isAdminAuthDisabled: auth.disabled,
  getBypassAdminUser: () => ({ id: 'local-test' }),
}));
import { requireFinanceActor } from '@/lib/shared/finance-actor';
beforeEach(() => {
  auth.session.mockResolvedValue('session');
  auth.disabled.mockReturnValue(false);
});
afterEach(() => vi.unstubAllGlobals());
it('uses only a verified administrator identity', async () => {
  const fetch = vi
    .fn()
    .mockResolvedValue({
      ok: true,
      json: async () => ({ user: { id: 'finance-admin', role: 'admin' } }),
    });
  vi.stubGlobal('fetch', fetch);
  expect(await requireFinanceActor()).toBe('finance-admin');
  expect(fetch).toHaveBeenCalledWith(expect.stringContaining('/api/auth/me'), {
    headers: { Authorization: 'Bearer session' },
    cache: 'no-store',
  });
});
it('rejects a valid session without administrator privilege', async () => {
  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValue({
        ok: true,
        json: async () => ({ user: { id: 'operator', role: 'operator' } }),
      }),
  );
  await expect(requireFinanceActor()).rejects.toMatchObject({ statusCode: 403 });
});
it('rejects expired sessions and missing identities', async () => {
  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValueOnce({ ok: false })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ user: { role: 'admin' } }) }),
  );
  await expect(requireFinanceActor()).rejects.toMatchObject({ statusCode: 401 });
  await expect(requireFinanceActor()).rejects.toMatchObject({ statusCode: 401 });
});
