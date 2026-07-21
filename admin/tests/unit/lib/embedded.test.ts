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

  it('serves EDI documents and a consistent summary in mock mode', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { ediDocumentsApi, summarizeEdiDocuments } = await import('@/lib/embedded');

    const documents = await ediDocumentsApi.list();
    expect(documents.length).toBeGreaterThan(0);
    for (const doc of documents) {
      expect(['inbound', 'outbound']).toContain(doc.direction);
      expect(['pending', 'sent', 'acknowledged', 'processed', 'error']).toContain(doc.status);
      expect(['850', '855', '856', '810']).toContain(doc.documentType);
    }

    const filtered = await ediDocumentsApi.list({ status: 'error' });
    expect(filtered.every((doc) => doc.status === 'error')).toBe(true);
    expect(filtered.every((doc) => Boolean(doc.errorMessage))).toBe(true);

    const fetched = await ediDocumentsApi.get(documents[0].id);
    expect(fetched).toEqual(documents[0]);
    await expect(ediDocumentsApi.get('missing')).resolves.toBeNull();

    // The engine summary must agree with a summary computed from the list.
    const summary = await ediDocumentsApi.summary();
    expect(summary).toEqual(summarizeEdiDocuments(documents));
    expect(summary.total).toBe(documents.length);
  });

  it('degrades EDI reads gracefully when the engine build lacks the surface', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { getCommerceEngine, ediDocumentsApi } = await import('@/lib/embedded');

    // Simulate an older binding build by stripping the EDI accessor from the
    // cached engine instance the API layer resolves.
    const engine = await getCommerceEngine();
    delete engine.ediDocuments;

    await expect(ediDocumentsApi.list()).resolves.toEqual([]);
    await expect(ediDocumentsApi.get('edi_1')).resolves.toBeNull();
    await expect(ediDocumentsApi.summary()).resolves.toEqual({
      total: 0,
      byStatus: [],
      byType: [],
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
