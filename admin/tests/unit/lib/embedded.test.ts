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
    expect(secondOrders).toEqual(firstOrders);

    // Mock orders follow the engine's order lifecycle: confirmed → processing
    // is legal, a terminal order moves nowhere.
    const confirmed = firstOrders.find((order) => order.status === 'confirmed');
    expect(confirmed).toBeDefined();
    const updated = await engine.orders.updateStatus(confirmed!.id, 'processing');
    expect(updated).toMatchObject({
      id: confirmed!.id,
      status: 'processing',
      items: confirmed!.items,
      currency: 'USD',
    });
    const cancelled = firstOrders.find((order) => order.status === 'cancelled');
    await expect(engine.orders.updateStatus(cancelled!.id, 'processing')).rejects.toThrow(
      /cannot move from cancelled to processing/,
    );
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

  it('normalises the native binding Debug-cased enums once, at the boundary', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');

    // A stand-in for the napi `Commerce` class: sub-APIs are prototype
    // getters, and the private fields fail loudly if a getter or method runs
    // with the proxy instead of the native instance as `this`.
    class NativeLots {
      readonly #rows = [
        { id: 'lot_1', status: 'OnHold' },
        { id: 'lot_2', status: 'Quarantine' },
      ];
      async list() {
        return this.#rows;
      }
    }
    class NativeCommerce {
      readonly #lots = new NativeLots();
      readonly #features = ['governed-writes'];
      get lots() {
        return this.#lots;
      }
      get accountsPayable() {
        return { listBills: async () => [{ id: 'bill_1', status: 'PartiallyPaid' }] };
      }
      get accountsReceivable() {
        return {
          listCreditMemos: async () => [
            { id: 'cm_1', status: 'FullyApplied', reason: 'ReturnedGoods' },
          ],
        };
      }
      get quality() {
        return {
          listNcrs: async () => [
            {
              id: 'ncr_1',
              status: 'UnderReview',
              severity: 'Critical',
              source: 'CustomerComplaint',
            },
          ],
        };
      }
      get generalLedger() {
        return {
          listAccounts: async () => [{ id: 'gl_1', status: 'Active', accountType: 'Asset' }],
          listJournalEntries: async () => [
            { id: 'je_1', status: 'Posted', description: 'Keep THIS casing', lines: [] },
          ],
          closeMonth: async () => ({ periodStatus: 'closed', closingEntry: { status: 'Posted' } }),
        };
      }
      get orders() {
        return { list: async () => [{ id: 'ord_1', status: 'partially_shipped', notes: 'Rush' }] };
      }
      kernelFeatures() {
        return this.#features;
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const {
      accountsPayableApi,
      generalLedgerApi,
      getCommerceEngine,
      lotsApi,
      ordersApi,
      qualityApi,
    } = await import('@/lib/embedded');

    expect((await lotsApi.list()).map((lot) => lot.status)).toEqual(['on_hold', 'quarantine']);
    expect((await accountsPayableApi.listBills())[0].status).toBe('partially_paid');
    expect((await qualityApi.listNcrs())[0]).toMatchObject({
      status: 'under_review',
      severity: 'critical',
      source: 'customer_complaint',
    });
    expect((await generalLedgerApi.listAccounts())[0]).toMatchObject({
      status: 'active',
      accountType: 'asset',
    });
    // Only enum fields change; free text keeps its casing.
    expect((await generalLedgerApi.listJournalEntries())[0]).toMatchObject({
      status: 'posted',
      description: 'Keep THIS casing',
    });
    // Nested records (the close report's closing entry) are normalised too.
    expect(await generalLedgerApi.closeMonth('per_1')).toMatchObject({
      closingEntry: { status: 'posted' },
    });

    const engine = (await getCommerceEngine()) as unknown as {
      accountsReceivable: { listCreditMemos(): Promise<{ status: string; reason: string }[]> };
      kernelFeatures(): string[];
    };
    // The ERP actions reach the engine directly (lib/erp.ts); they get the
    // same normalisation.
    expect((await engine.accountsReceivable.listCreditMemos())[0]).toMatchObject({
      status: 'fully_applied',
      reason: 'returned_goods',
    });
    // Domains that already render snake_case pass through untouched, and
    // plain methods stay bound to the native instance.
    expect(await ordersApi.list()).toEqual([
      { id: 'ord_1', status: 'partially_shipped', notes: 'Rush' },
    ]);
    expect(engine.kernelFeatures()).toEqual(['governed-writes']);
  });

  it('walks a mock return through the engine lifecycle and refuses illegal moves', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { returnsApi } = await import('@/lib/embedded');
    const { RETURN_STATUSES } = await import('@/lib/engine-lifecycles');

    const all = await returnsApi.list();
    // Demo data covers every engine state (and nothing invented).
    expect(new Set(all.map((entry) => entry.status))).toEqual(new Set(RETURN_STATUSES));

    const requested = all.find((entry) => entry.status === 'requested');
    expect(requested).toBeDefined();
    const id = requested!.id;

    // Receive is only legal once the return is in transit.
    await expect(returnsApi.markReceived(id)).rejects.toThrow(/cannot move from requested/);

    expect((await returnsApi.approve(id)).status).toBe('approved');
    await expect(returnsApi.reject(id, 'too late')).rejects.toThrow(/cannot move from approved/);
    expect(await returnsApi.addTracking(id, '1Z999')).toMatchObject({
      status: 'in_transit',
      trackingNumber: '1Z999',
    });
    expect((await returnsApi.markReceived(id)).status).toBe('received');
    expect((await returnsApi.complete(id)).status).toBe('completed');
    // Terminal: nothing moves a completed return.
    await expect(returnsApi.cancel(id)).rejects.toThrow(/cannot move from completed/);
    expect((await returnsApi.get(id))?.status).toBe('completed');

    await expect(returnsApi.approve('ret_missing')).rejects.toThrow(/not found/);
  });

  it('fills native return headers and derives return analytics the binding lacks', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const header = {
      id: 'ret_1',
      orderId: 'ord_1',
      status: 'in_transit',
      reason: 'damaged',
      version: 2,
      createdAt: '2026-05-07T12:00:00.000Z',
    };
    class NativeCommerce {
      get returns() {
        return {
          list: async () => [header],
          markReceived: async (id: string) => ({ ...header, id, status: 'received' }),
        };
      }
      get analytics() {
        return {
          returnMetrics: async () => ({
            totalReturns: 1,
            returnRatePercent: 12.5,
            totalRefunded: 10.5,
            totalRefundedExact: '10.50',
          }),
        };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { returnsApi } = await import('@/lib/embedded');

    expect(await returnsApi.list()).toEqual([
      {
        ...header,
        customerId: '',
        items: [],
        reasonCategory: 'damaged',
        updatedAt: header.createdAt,
      },
    ]);
    expect(await returnsApi.markReceived('ret_1')).toMatchObject({
      status: 'received',
      items: [],
    });
    expect(await returnsApi.getAnalytics()).toEqual({
      totalReturns: 1,
      returnRate: 0.125,
      refundTotal: 10.5,
      returnsByReason: { damaged: 1 },
      returnsByStatus: { in_transit: 1 },
      averageProcessingTime: 0,
      topReturnedProducts: [],
    });
  });

  it('routes subscriptions to the binding methods and derives their analytics', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const native = {
      id: 'sub_1',
      customerId: 'cus_1',
      planId: 'plan_pro',
      planName: 'Pro',
      status: 'active',
      billingInterval: 'quarterly',
      price: 90,
      createdAt: '2026-05-01T00:00:00.000Z',
      updatedAt: '2026-05-02T00:00:00.000Z',
    };
    const subscribe = vi.fn(async () => native);
    const cancel = vi.fn(async (id: string) => ({ ...native, id, status: 'cancelled' }));
    class NativeCommerce {
      get subscriptions() {
        return {
          list: async () => [native, { ...native, id: 'sub_2', status: 'trial' }],
          subscribe,
          cancel,
        };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { subscriptionsApi } = await import('@/lib/embedded');

    await expect(
      subscriptionsApi.create({ customerId: 'cus_1', planId: 'plan_pro' }),
    ).resolves.toMatchObject({ plan: 'Pro', totalAmount: 90, frequency: 'quarterly' });
    expect(subscribe).toHaveBeenCalledWith({ customerId: 'cus_1', planId: 'plan_pro' });

    // The binding takes `CancelSubscriptionInput`, not a bare string.
    await subscriptionsApi.cancel('sub_1', 'too expensive');
    expect(cancel).toHaveBeenCalledWith('sub_1', { reason: 'too expensive' });
    await subscriptionsApi.cancel('sub_1');
    expect(cancel).toHaveBeenLastCalledWith('sub_1', undefined);

    expect(await subscriptionsApi.getAnalytics()).toMatchObject({
      totalSubscriptions: 2,
      activeSubscriptions: 1,
      mrr: 30,
      arr: 360,
      subscriptionsByPlan: { Pro: 2 },
    });
  });

  it('refuses mock subscription moves the engine lifecycle forbids', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });

    const { subscriptionsApi } = await import('@/lib/embedded');
    const { SUBSCRIPTION_STATUSES } = await import('@/lib/engine-lifecycles');
    const all = await subscriptionsApi.list();
    expect(new Set(all.map((subscription) => subscription.status))).toEqual(
      new Set(SUBSCRIPTION_STATUSES),
    );

    const active = all.find((subscription) => subscription.status === 'active')!;
    expect((await subscriptionsApi.pause(active.id)).status).toBe('paused');
    expect((await subscriptionsApi.resume(active.id)).status).toBe('active');

    const expired = all.find((subscription) => subscription.status === 'expired')!;
    await expect(subscriptionsApi.resume(expired.id)).rejects.toThrow(
      /cannot move from expired to active/,
    );
  });

  it('routes admin calls the binding names differently and explains the ones it lacks', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const orders = [
      {
        id: 'ord_1',
        customerId: 'cus_1',
        status: 'delivered',
        totalAmount: 40,
        items: [],
        currency: 'USD',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
      {
        id: 'ord_2',
        customerId: 'cus_2',
        status: 'partially_shipped',
        totalAmount: 60,
        items: [],
        currency: 'USD',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    ];
    const listOrders = vi.fn(async (filter?: { customerId?: string }) =>
      filter?.customerId
        ? orders.filter((order) => order.customerId === filter.customerId)
        : orders,
    );
    class NativeCommerce {
      get orders() {
        return { list: listOrders };
      }
      get inventory() {
        return {
          getStock: async (sku: string) => ({
            sku,
            name: 'Widget',
            totalOnHand: '12',
            totalAllocated: '2',
            totalAvailable: '10',
          }),
        };
      }
      get customers() {
        return { list: async () => [] };
      }
      get analytics() {
        return {
          lowStockItems: async () => [
            {
              sku: 'SKU-1',
              name: 'Widget',
              onHand: 3,
              allocated: 1,
              available: 2,
              reorderPoint: 5,
            },
          ],
          inventoryHealth: async () => ({
            totalSkus: 4,
            lowStockSkus: 1,
            outOfStockSkus: 0,
            totalValue: 99.5,
            totalValueExact: '99.50',
          }),
          demandForecast: async () => [
            {
              sku: 'SKU-1',
              averageDailyDemand: 2,
              forecastedDemand: 6,
              confidence: 0.8,
              currentStock: 10,
              recommendedReorderQty: 20,
            },
          ],
        };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { analyticsApi, customersApi, inventoryApi, ordersApi, EngineCapabilityError } =
      await import('@/lib/embedded');

    // Derived from the order records.
    expect(await ordersApi.getAnalytics()).toMatchObject({
      totalOrders: 2,
      totalRevenue: 100,
      averageOrderValue: 50,
      ordersByStatus: { delivered: 1, partially_shipped: 1 },
    });
    // customers.getOrders -> orders.list({ customerId })
    expect(await customersApi.getOrders('cus_2')).toEqual([orders[1]]);
    expect(listOrders).toHaveBeenLastCalledWith({ customerId: 'cus_2' });

    // inventory.get -> getStock, getLowStock/getAnalytics/forecast -> analytics.*
    expect(await inventoryApi.get('SKU-1')).toMatchObject({
      sku: 'SKU-1',
      productName: 'Widget',
      quantity: 12,
      reservedQuantity: 2,
      availableQuantity: 10,
    });
    expect(await inventoryApi.getLowStock()).toMatchObject([
      { sku: 'SKU-1', availableQuantity: 2, reorderPoint: 5 },
    ]);
    expect(await inventoryApi.getAnalytics()).toMatchObject({
      totalSKUs: 4,
      lowStockItems: 1,
      outOfStockItems: 0,
      totalValue: 99.5,
    });
    expect(await inventoryApi.forecast('SKU-1', 3)).toMatchObject({
      sku: 'SKU-1',
      forecastedDemand: [2, 2, 2],
      recommendedReorder: 20,
      confidence: 0.8,
    });

    // No native equivalent: a typed, explanatory error instead of a TypeError.
    await expect(inventoryApi.list()).rejects.toBeInstanceOf(EngineCapabilityError);
    await expect(inventoryApi.list()).rejects.toThrow(/no inventory listing/);
    await expect(inventoryApi.supportsListing()).resolves.toBe(false);
    await expect(inventoryApi.release('SKU-1', 1, 'ord_1')).rejects.toBeInstanceOf(
      EngineCapabilityError,
    );
    await expect(customersApi.getHealthScore('cus_1')).rejects.toBeInstanceOf(
      EngineCapabilityError,
    );
    await expect(customersApi.getAnalytics()).rejects.toBeInstanceOf(EngineCapabilityError);
    await expect(analyticsApi.getDashboardMetrics()).rejects.toMatchObject({
      code: 'ENGINE_CAPABILITY_UNAVAILABLE',
      feature: 'analytics.getDashboardMetrics',
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
