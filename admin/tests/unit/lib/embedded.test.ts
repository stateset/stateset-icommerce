import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Subscription } from '@/lib/types';

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
      refundTotalExact: '10.50',
      returnsByReason: { damaged: 1 },
      returnsByStatus: { in_transit: 1 },
      averageProcessingTime: 0,
      topReturnedProducts: [],
      coverage: { complete: true, records: 1 },
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

  /**
   * A stand-in for a native list that pages the way the engine does
   * (`append_limit_offset`): 500 rows when no limit is given, never more than
   * 1000, `offset` rows skipped.
   */
  function nativePager<T>(rows: (offset: number, limit: number) => T[]) {
    return vi.fn(async (filter?: { limit?: number; offset?: number }) => {
      const limit = Math.min(filter?.limit ?? 500, 1000);
      return rows(filter?.offset ?? 0, limit);
    });
  }

  function pagedSlice<T>(all: T[]) {
    return nativePager((offset, limit) => all.slice(offset, offset + limit));
  }

  it('derives native analytics from every page, not the first 500 records', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const createdAt = new Date().toISOString();
    const orders = Array.from({ length: 1_200 }, (_, index) => ({
      id: `ord_${index}`,
      customerId: 'cus_1',
      status: index < 700 ? 'delivered' : 'pending',
      totalAmount: 10,
      totalAmountExact: '10.00',
      items: [],
      currency: 'USD',
      createdAt,
      updatedAt: createdAt,
    }));
    const returns = Array.from({ length: 600 }, (_, index) => ({
      id: `ret_${index}`,
      orderId: `ord_${index}`,
      status: 'requested',
      reason: 'damaged',
      version: 1,
      createdAt,
    }));
    // The finding's example: 600 active $10/month subscriptions are $6,000 MRR.
    const subscriptions = Array.from({ length: 600 }, (_, index) => ({
      id: `sub_${index}`,
      customerId: 'cus_1',
      planId: 'plan_basic',
      planName: 'Basic',
      status: 'active',
      billingInterval: 'monthly',
      price: 10,
      priceExact: '10.00',
      createdAt,
      updatedAt: createdAt,
    }));
    const listOrders = pagedSlice(orders);
    class NativeCommerce {
      get orders() {
        return { list: listOrders };
      }
      get returns() {
        return { list: pagedSlice(returns) };
      }
      get subscriptions() {
        return { list: pagedSlice(subscriptions) };
      }
      get analytics() {
        return {
          returnMetrics: async () => ({
            totalReturns: 600,
            returnRatePercent: 50,
            totalRefunded: 0,
            totalRefundedExact: '0',
          }),
        };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { ordersApi, returnsApi, subscriptionsApi } = await import('@/lib/embedded');

    const orderAnalytics = await ordersApi.getAnalytics();
    expect(orderAnalytics).toMatchObject({
      totalOrders: 1_200,
      totalRevenue: 12_000,
      ordersByStatus: { delivered: 700, pending: 500 },
      coverage: { complete: true, records: 1_200 },
    });
    expect(listOrders.mock.calls.length).toBeGreaterThan(1);

    const returnAnalytics = await returnsApi.getAnalytics();
    expect(returnAnalytics).toMatchObject({
      totalReturns: 600,
      returnsByStatus: { requested: 600 },
      coverage: { complete: true, records: 600 },
    });

    expect(await subscriptionsApi.getAnalytics()).toMatchObject({
      totalSubscriptions: 600,
      activeSubscriptions: 600,
      mrr: 6_000,
      coverage: { complete: true, records: 600 },
    });
  });

  it('marks a derived aggregate partial when the scan stops at its cap', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const createdAt = new Date().toISOString();
    // A collection that never ends: every page is full.
    const listOrders = nativePager((offset, limit) =>
      Array.from({ length: limit }, (_, index) => ({
        id: `ord_${offset + index}`,
        customerId: 'cus_1',
        status: 'pending',
        totalAmount: 1,
        totalAmountExact: '1',
        items: [],
        currency: 'USD',
        createdAt,
        updatedAt: createdAt,
      })),
    );
    class NativeCommerce {
      get orders() {
        return { list: listOrders };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { ordersApi, ANALYTICS_SCAN_LIMIT } = await import('@/lib/embedded');

    const analytics = await ordersApi.getAnalytics();
    expect(analytics.totalOrders).toBe(ANALYTICS_SCAN_LIMIT);
    expect(analytics.coverage).toMatchObject({ complete: false, records: ANALYTICS_SCAN_LIMIT });
  });

  function nativeOrder(id: string, createdAt: string, totalAmountExact = '10.00') {
    return {
      id,
      customerId: 'cus_1',
      status: 'delivered',
      totalAmount: Number(totalAmountExact),
      totalAmountExact,
      items: [],
      currency: 'USD',
      createdAt,
      updatedAt: createdAt,
    };
  }

  function nativeReturn(id: string, createdAt: string) {
    return { id, orderId: 'ord_1', status: 'completed', reason: 'damaged', version: 1, createdAt };
  }

  it('honours a requested window in the native order analytics', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const orders = [
      nativeOrder('ord_sep', '2026-09-30T23:59:59.999Z'),
      nativeOrder('ord_first', '2026-10-01T00:00:00.000Z'),
      nativeOrder('ord_last', '2026-10-31T23:59:59.000Z'),
      nativeOrder('ord_nov', '2026-11-01T00:00:00.000Z'),
    ];
    // Worst case: an engine build that ignores the date filter.
    const listOrders = vi.fn(async (_filter?: Record<string, unknown>) => orders);
    class NativeCommerce {
      get orders() {
        return { list: listOrders };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { ordersApi } = await import('@/lib/embedded');

    const october = await ordersApi.getAnalytics({
      startDate: '2026-10-01',
      endDate: '2026-10-31',
    });
    // The window goes to the engine as RFC 3339 bounds covering whole days...
    expect(listOrders).toHaveBeenCalledWith(
      expect.objectContaining({
        fromDate: '2026-10-01T00:00:00.000Z',
        toDate: '2026-10-31T23:59:59.999Z',
      }),
    );
    // ...and every figure covers that window only, boundaries included.
    expect(october).toMatchObject({
      totalOrders: 2,
      totalRevenue: 20,
      ordersByStatus: { delivered: 2 },
      coverage: {
        complete: true,
        records: 2,
        window: { start: '2026-10-01T00:00:00.000Z', end: '2026-10-31T23:59:59.999Z' },
      },
    });
    expect(october.ordersByDay).toHaveLength(31);
    expect(october.ordersByDay[0]).toEqual({
      date: '2026-10-01',
      count: 1,
      revenue: 10,
      revenueExact: '10.00',
    });
    expect(october.ordersByDay[30]).toMatchObject({ date: '2026-10-31', count: 1 });

    await expect(ordersApi.getAnalytics({ startDate: 'not a date' })).rejects.toThrow(
      /Invalid analytics startDate/,
    );
  });

  it('reports undated return analytics over one all-time population', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    // One $50 return from 60 days ago and nothing recent: the engine's
    // default (last 30 days) metrics would report no refunds for it.
    const old = new Date(Date.now() - 60 * 24 * 60 * 60 * 1000).toISOString();
    const returnMetrics = vi.fn(async (query?: { period?: string }) =>
      query?.period === 'all_time'
        ? {
            totalReturns: 1,
            returnRatePercent: 100,
            totalRefunded: 50,
            totalRefundedExact: '50.00',
          }
        : { totalReturns: 0, returnRatePercent: 0, totalRefunded: 0, totalRefundedExact: '0' },
    );
    class NativeCommerce {
      get returns() {
        return { list: async () => [nativeReturn('ret_old', old)] };
      }
      get analytics() {
        return { returnMetrics };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { returnsApi } = await import('@/lib/embedded');

    expect(await returnsApi.getAnalytics()).toMatchObject({
      totalReturns: 1,
      returnRate: 1,
      refundTotal: 50,
    });
    expect(returnMetrics).toHaveBeenCalledWith({ period: 'all_time' });
  });

  it('honours a requested window in the native return analytics', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const returns = [
      nativeReturn('ret_sep', '2026-09-15T12:00:00.000Z'),
      nativeReturn('ret_oct', '2026-10-15T12:00:00.000Z'),
      nativeReturn('ret_nov', '2026-11-02T12:00:00.000Z'),
    ];
    const orders = [
      nativeOrder('ord_sep', '2026-09-10T00:00:00.000Z'),
      ...['01', '08', '15', '22'].map((day) =>
        nativeOrder(`ord_${day}`, `2026-10-${day}T09:00:00Z`),
      ),
      nativeOrder('ord_nov', '2026-11-01T00:00:00.000Z'),
    ];
    const listReturns = vi.fn(async (_filter?: Record<string, unknown>) => returns);
    // The engine's metrics only take named periods; they cannot express October.
    const returnMetrics = vi.fn(async () => ({
      totalReturns: 3,
      returnRatePercent: 50,
      totalRefunded: 75,
      totalRefundedExact: '75.00',
    }));
    class NativeCommerce {
      get returns() {
        return { list: listReturns };
      }
      get orders() {
        return { list: async () => orders };
      }
      get analytics() {
        return { returnMetrics };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { returnsApi } = await import('@/lib/embedded');

    const october = await returnsApi.getAnalytics({
      startDate: '2026-10-01',
      endDate: '2026-10-31',
    });
    expect(listReturns).toHaveBeenCalledWith(
      expect.objectContaining({
        fromDate: '2026-10-01T00:00:00.000Z',
        toDate: '2026-10-31T23:59:59.999Z',
      }),
    );
    expect(returnMetrics).not.toHaveBeenCalled();
    expect(october).toMatchObject({
      totalReturns: 1,
      returnsByStatus: { completed: 1 },
      // One October return against four October orders.
      returnRate: 0.25,
      // Native return records carry no refund amount, so the October refund
      // total is not measured; it is flagged, not invented.
      refundTotal: 0,
      coverage: { complete: true, records: 1, unavailable: ['refundTotal'] },
    });
  });

  it('keeps money exact through the derived native summaries', async () => {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'false');
    const now = new Date().toISOString();
    const subscription = (id: string, billingInterval: string, priceExact: string) => ({
      id,
      customerId: 'cus_1',
      planId: `plan_${id}`,
      planName: 'Plan',
      status: 'active',
      billingInterval,
      price: Number(priceExact),
      priceExact,
      createdAt: now,
      updatedAt: now,
    });
    class NativeCommerce {
      get orders() {
        return {
          list: async () => [
            nativeOrder('ord_1', now, '0.10'),
            nativeOrder('ord_2', now, '0.20'),
            nativeOrder('ord_3', now, '1.005'),
          ],
        };
      }
      get returns() {
        return { list: async () => [nativeReturn('ret_1', now)] };
      }
      get subscriptions() {
        return {
          list: async () => [
            subscription('sub_w', 'weekly', '10.00'),
            subscription('sub_m', 'monthly', '19.99'),
            subscription('sub_q', 'quarterly', '0.10'),
          ],
        };
      }
      get inventory() {
        return {};
      }
      get analytics() {
        return {
          returnMetrics: async () => ({
            totalReturns: 1,
            returnRatePercent: 10,
            totalRefunded: 1.005,
            totalRefundedExact: '1.005',
          }),
          inventoryHealth: async () => ({
            totalSkus: 1,
            lowStockSkus: 0,
            outOfStockSkus: 0,
            totalValue: 90071992547409.94,
            totalValueExact: '90071992547409.94',
          }),
        };
      }
    }
    vi.doMock('@stateset/embedded', () => ({ Commerce: NativeCommerce }));

    const { inventoryApi, ordersApi, returnsApi, subscriptionsApi } =
      await import('@/lib/embedded');

    const orders = await ordersApi.getAnalytics();
    expect(orders).toMatchObject({
      totalRevenueExact: '1.305',
      averageOrderValueExact: '0.435',
    });
    expect(orders.ordersByDay.at(-1)).toMatchObject({ count: 3, revenueExact: '1.305' });

    expect(await returnsApi.getAnalytics()).toMatchObject({ refundTotalExact: '1.005' });

    // Yearly value 10.00 x 52 + 19.99 x 12 + 0.10 x 4 = 760.28, exactly; MRR
    // and ARPU are rounded once from it. The float path summed rounded
    // monthly shares and reported ARR 760.32.
    expect(await subscriptionsApi.getAnalytics()).toMatchObject({
      arrExact: '760.28',
      arr: 760.28,
      mrrExact: '63.36',
      mrr: 63.36,
      arpuExact: '21.12',
    });
    const [listed] = await subscriptionsApi.list();
    expect(listed).toMatchObject({ totalAmountExact: '10.00' });

    expect(await inventoryApi.getAnalytics()).toMatchObject({
      totalValueExact: '90071992547409.94',
    });
  });

  function useMockEngine() {
    vi.stubEnv('STATESET_ADMIN_ALLOW_MOCK_DATA', 'true');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.doMock('@stateset/embedded', () => {
      throw new Error('native addon missing');
    });
  }

  it('lets demo records created in the session take lifecycle actions', async () => {
    useMockEngine();
    const { ordersApi, returnsApi, subscriptionsApi } = await import('@/lib/embedded');

    const order = await ordersApi.create({
      customerId: 'cus_100',
      items: [{ productId: 'prod_1', quantity: 2 }],
    });
    expect(order.status).toBe('pending');
    expect((await ordersApi.updateStatus(order.id, 'confirmed')).status).toBe('confirmed');
    expect((await ordersApi.get(order.id))?.status).toBe('confirmed');
    expect((await ordersApi.list()).map((entry) => entry.id)).toContain(order.id);

    const created = await returnsApi.create({
      orderId: order.id,
      items: [{ productId: 'prod_1', quantity: 1 }],
      reason: 'Arrived broken',
      reasonCategory: 'damaged',
    });
    expect(created.status).toBe('requested');
    expect((await returnsApi.approve(created.id)).status).toBe('approved');

    const subscription = await subscriptionsApi.create({ customerId: 'cus_100', planId: 'pro' });
    expect((await subscriptionsApi.pause(subscription.id)).status).toBe('paused');
    expect((await subscriptionsApi.get(subscription.id))?.status).toBe('paused');

    // Unknown ids are still refused.
    await expect(ordersApi.updateStatus('ord_missing', 'confirmed')).rejects.toThrow(/not found/);
    await expect(subscriptionsApi.pause('sub_missing')).rejects.toThrow(/not found/);
  });

  it('keeps demo analytics in step with demo lifecycle changes', async () => {
    useMockEngine();
    const { analyticsApi, ordersApi, returnsApi, subscriptionsApi } =
      await import('@/lib/embedded');

    // Orders: cancelling a pending order moves it between status counts.
    const before = await ordersApi.getAnalytics();
    const pending = (await ordersApi.list()).find((order) => order.status === 'pending')!;
    await ordersApi.cancel(pending.id);
    const after = await ordersApi.getAnalytics();
    expect(after.ordersByStatus.pending ?? 0).toBe(before.ordersByStatus.pending - 1);
    expect(after.ordersByStatus.cancelled).toBe(before.ordersByStatus.cancelled + 1);
    expect(after.totalOrders).toBe(before.totalOrders);

    // Returns: approving a requested return.
    const returnsBefore = await returnsApi.getAnalytics();
    const requested = (await returnsApi.list()).find((entry) => entry.status === 'requested')!;
    await returnsApi.approve(requested.id);
    const returnsAfter = await returnsApi.getAnalytics();
    expect(returnsAfter.returnsByStatus.requested ?? 0).toBe(
      returnsBefore.returnsByStatus.requested - 1,
    );
    expect(returnsAfter.returnsByStatus.approved).toBe(returnsBefore.returnsByStatus.approved + 1);
    // Simulated-only figures survive the recomputation.
    expect(returnsAfter.averageProcessingTime).toBe(returnsBefore.averageProcessingTime);
    expect(returnsAfter.topReturnedProducts).toEqual(returnsBefore.topReturnedProducts);

    // Subscriptions: pausing an active monthly subscription takes its price out of MRR.
    const subsBefore = await subscriptionsApi.getAnalytics();
    const active = (await subscriptionsApi.list()).find(
      (subscription) => subscription.status === 'active' && subscription.frequency === 'monthly',
    )!;
    await subscriptionsApi.pause(active.id);
    const subsAfter = await subscriptionsApi.getAnalytics();
    expect(subsAfter.activeSubscriptions).toBe(subsBefore.activeSubscriptions - 1);
    expect(subsAfter.mrr).toBeCloseTo(subsBefore.mrr - active.totalAmount, 2);
    expect(subsAfter.mrrGrowth).toBe(subsBefore.mrrGrowth);

    // Creations reach the aggregates too, the dashboard included.
    const dashboardBefore = await analyticsApi.getDashboardMetrics();
    const order = await ordersApi.create({
      customerId: 'cus_100',
      items: [{ productId: 'prod_1', quantity: 1 }],
    });
    await returnsApi.create({
      orderId: order.id,
      items: [{ productId: 'prod_1', quantity: 1 }],
      reason: 'Not the one ordered',
      reasonCategory: 'wrong_item',
    });
    expect((await ordersApi.getAnalytics()).totalOrders).toBe(before.totalOrders + 1);
    expect((await returnsApi.getAnalytics()).totalReturns).toBe(returnsBefore.totalReturns + 1);
    const dashboardAfter = await analyticsApi.getDashboardMetrics();
    expect(dashboardAfter.returnRate).toBeCloseTo(
      (returnsBefore.totalReturns + 1) / (before.totalOrders + 1),
      3,
    );
    expect(dashboardAfter.returnRate).not.toBe(dashboardBefore.returnRate);
  });

  it('honours a requested window in demo analytics', async () => {
    useMockEngine();
    const { ordersApi } = await import('@/lib/embedded');

    const all = await ordersApi.list();
    const day = all[0].createdAt.slice(0, 10);
    const sameDay = all.filter((order) => order.createdAt.startsWith(day));
    const analytics = await ordersApi.getAnalytics({ startDate: day, endDate: day });
    expect(analytics.totalOrders).toBe(sameDay.length);
    expect(analytics.totalOrders).toBeLessThan(all.length);
    expect(analytics.ordersByDay).toEqual([
      expect.objectContaining({ date: day, count: sameDay.length }),
    ]);
  });

  it('pauses a demo trial subscription, as the engine pause action does', async () => {
    useMockEngine();
    const { subscriptionsApi } = await import('@/lib/embedded');
    const all = await subscriptionsApi.list();

    const trial = all.find((subscription) => subscription.status === 'trial')!;
    expect((await subscriptionsApi.pause(trial.id)).status).toBe('paused');
    expect((await subscriptionsApi.resume(trial.id)).status).toBe('active');

    // The engine refuses to pause anything but an active or trial subscription.
    const pending = all.find((subscription) => subscription.status === 'pending')!;
    await expect(subscriptionsApi.pause(pending.id)).rejects.toThrow(
      /cannot move from pending to paused/,
    );
  });

  it('derives subscription status, plan, activity and renewal figures from the records', async () => {
    const { summarizeSubscriptions } = await import('@/lib/embedded');
    const now = Date.parse('2026-06-15T12:00:00.000Z');
    const at = (days: number) => new Date(now + days * 24 * 60 * 60 * 1000).toISOString();
    const subscription = (
      id: string,
      fields: Partial<Subscription> & Pick<Subscription, 'status' | 'frequency' | 'plan'>,
    ): Subscription => ({
      id,
      customerId: `cus_${id}`,
      nextBillingDate: '',
      items: [],
      quantity: 1,
      totalAmount: 0,
      createdAt: at(-200),
      updatedAt: at(-1),
      ...fields,
    });
    const records = [
      subscription('a', {
        status: 'active',
        frequency: 'monthly',
        plan: 'Pro',
        totalAmountExact: '10.00',
        createdAt: at(-5),
        nextBillingDate: at(2),
      }),
      subscription('b', {
        status: 'active',
        frequency: 'quarterly',
        plan: 'Pro',
        totalAmountExact: '90.00',
        createdAt: at(-60),
        nextBillingDate: at(10),
      }),
      subscription('c', {
        status: 'active',
        frequency: 'annually',
        plan: 'Basic',
        totalAmountExact: '120.00',
        createdAt: at(-40),
        nextBillingDate: at(1),
      }),
      subscription('d', {
        status: 'cancelled',
        frequency: 'monthly',
        plan: 'Basic',
        totalAmountExact: '20.00',
        cancelledAt: at(-3),
      }),
      // An immediate cancellation expires the subscription; 45 days ago is
      // outside the 30-day window.
      subscription('e', {
        status: 'expired',
        frequency: 'monthly',
        plan: 'Basic',
        totalAmountExact: '15.00',
        cancelledAt: at(-45),
      }),
      subscription('f', {
        status: 'paused',
        frequency: 'monthly',
        plan: 'Pro',
        totalAmountExact: '30.00',
        createdAt: at(-10),
      }),
    ];

    const analytics = summarizeSubscriptions(records, { now });
    expect(analytics).toMatchObject({
      // 10 x 12 + 90 x 4 + 120 = 600 a year.
      mrrExact: '50.00',
      subscriptionsByStatus: { active: 3, cancelled: 1, expired: 1, paused: 1 },
      activeByPlan: [
        { plan: 'Pro', count: 2, mrrExact: '40.00' },
        { plan: 'Basic', count: 1, mrrExact: '10.00' },
      ],
      // a and f started in the last 30 days; only a is active, so new MRR is
      // its 10.00. d's cancellation took away 20.00.
      recentActivity: {
        days: 30,
        newSubscriptions: 2,
        newMrrExact: '10.00',
        cancellations: 1,
        churnedMrrExact: '20.00',
      },
      // c and a renew in the next 7 days, soonest first; b renews in 10.
      upcomingRenewals: {
        days: 7,
        count: 2,
        amountExact: '130.00',
        next: [
          {
            id: 'c',
            customerId: 'cus_c',
            plan: 'Basic',
            renewalDate: '2026-06-16',
            amountExact: '120.00',
          },
          {
            id: 'a',
            customerId: 'cus_a',
            plan: 'Pro',
            renewalDate: '2026-06-17',
            amountExact: '10.00',
          },
        ],
      },
    });

    // A cancelled subscription without a cancellation date makes the
    // cancellation figures unknown, not undercounted.
    const undated = summarizeSubscriptions(
      [...records, subscription('g', { status: 'cancelled', frequency: 'monthly', plan: 'Pro' })],
      { now },
    );
    expect(undated.recentActivity).toMatchObject({ cancellations: null, churnedMrrExact: null });
  });

  it('derives demo subscription activity on the demo clock and follows cancellations', async () => {
    useMockEngine();
    const { subscriptionsApi } = await import('@/lib/embedded');

    // Demo records are dated around the demo clock, so renewals fall due.
    const before = await subscriptionsApi.getAnalytics();
    expect(before.upcomingRenewals!.count).toBeGreaterThan(0);
    expect(before.recentActivity!.cancellations).not.toBeNull();

    const active = (await subscriptionsApi.list()).find(
      (entry) => entry.status === 'active' && entry.frequency === 'monthly',
    )!;
    const cancelled = await subscriptionsApi.cancel(active.id);
    expect(cancelled.cancelledAt).toBe(cancelled.updatedAt);

    const after = await subscriptionsApi.getAnalytics();
    expect(after.recentActivity!.cancellations).toBe(before.recentActivity!.cancellations! + 1);
    expect(Number(after.recentActivity!.churnedMrrExact)).toBeCloseTo(
      Number(before.recentActivity!.churnedMrrExact) + active.totalAmount,
      2,
    );
    expect(after.subscriptionsByStatus!.cancelled).toBe(
      before.subscriptionsByStatus!.cancelled + 1,
    );
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
