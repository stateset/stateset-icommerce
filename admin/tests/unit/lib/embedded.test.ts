import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('server-only', () => ({}));

describe('embedded commerce engine', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    vi.resetModules();
  });

  it('fails closed when the native module is unavailable and mock mode is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { getCommerceEngine } = await import('@/lib/embedded');

    await expect(getCommerceEngine()).rejects.toThrow(
      /STATESET_ADMIN_ALLOW_MOCK_DATA=true only for explicit demo mode/i,
    );
  });

  it('returns deterministic mock data only when mock mode is explicitly enabled', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { getCommerceEngine } = await import('@/lib/embedded');
    const engine = await getCommerceEngine();

    const firstOrders = await engine.orders.list();
    const secondOrders = await engine.orders.list();
    const updated = await engine.orders.updateStatus(firstOrders[0].id, 'processing');

    expect(secondOrders).toEqual(firstOrders);
    expect(updated).toMatchObject({
      id: firstOrders[0].id,
      status: 'processing',
      items: firstOrders[0].items,
      currency: 'USD',
    });
  });

  it('rejects explicit mock mode in production', async () => {
    vi.stubEnv('NODE_ENV', 'production');
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { getCommerceEngine } = await import('@/lib/embedded');

    await expect(getCommerceEngine()).rejects.toThrow(
      /STATESET_ADMIN_ALLOW_MOCK_DATA=true is rejected in production/i,
    );
  });
});
