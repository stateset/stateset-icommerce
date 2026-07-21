/**
 * Tests for the operations server actions (purchasing, warehouse,
 * manufacturing).
 *
 * Server actions bypass the API middleware, so every exported action in
 * `@/app/actions/operations` must enforce the admin session itself via
 * `requireAdminSession()`. These tests lock down that contract, plus input
 * validation and filter passthrough.
 *
 * @module tests/unit/app/actions/operations
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

// Enumerate every named export explicitly (a Proxy-based mock makes the
// module look thenable and hangs the import).
vi.mock('@/lib/embedded', () => ({
  purchaseOrdersApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    listSuppliers: vi.fn().mockResolvedValue([]),
  },
  warehouseApi: {
    listWarehouses: vi.fn().mockResolvedValue([]),
    listLocations: vi.fn().mockResolvedValue([]),
  },
  cycleCountsApi: {
    list: vi.fn().mockResolvedValue([]),
  },
  workOrdersApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
  },
  qualityApi: {
    listInspections: vi.fn().mockResolvedValue([]),
    listNcrs: vi.fn().mockResolvedValue([]),
  },
  fulfillmentApi: {
    listWaves: vi.fn().mockResolvedValue([]),
    listPicks: vi.fn().mockResolvedValue([]),
  },
  lotsApi: {
    list: vi.fn().mockResolvedValue([]),
  },
  serialsApi: {
    list: vi.fn().mockResolvedValue([]),
  },
  receivingApi: {
    listReceipts: vi.fn().mockResolvedValue([]),
  },
}));

import {
  getPurchaseOrders,
  getPurchaseOrder,
  getSuppliers,
  getPurchasingPageData,
  getWarehouses,
  getWarehouseLocations,
  getCycleCounts,
  getWarehousePageData,
  getWorkOrders,
  getWorkOrder,
  getQualityInspections,
  getNonConformanceReports,
  getManufacturingPageData,
  getFulfillmentWaves,
  getPickTasks,
  getFulfillmentPageData,
  getLots,
  getSerials,
  getReceipts,
  getTraceabilityPageData,
} from '@/app/actions/operations';
import {
  purchaseOrdersApi,
  warehouseApi,
  cycleCountsApi,
  workOrdersApi,
  qualityApi,
  fulfillmentApi,
  lotsApi,
  serialsApi,
  receivingApi,
} from '@/lib/embedded';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('operations actions auth guard', () => {
  describe('without a session', () => {
    it('rejects purchasing reads and never reaches the embedded engine', async () => {
      await expect(getPurchaseOrders()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getPurchaseOrder('po_1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getSuppliers()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getPurchasingPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(purchaseOrdersApi.list).not.toHaveBeenCalled();
      expect(purchaseOrdersApi.listSuppliers).not.toHaveBeenCalled();
    });

    it('rejects warehouse reads', async () => {
      await expect(getWarehouses()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getWarehouseLocations(1)).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getCycleCounts()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getWarehousePageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(warehouseApi.listWarehouses).not.toHaveBeenCalled();
      expect(cycleCountsApi.list).not.toHaveBeenCalled();
    });

    it('rejects fulfillment reads', async () => {
      await expect(getFulfillmentWaves()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getPickTasks()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getFulfillmentPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(fulfillmentApi.listWaves).not.toHaveBeenCalled();
      expect(fulfillmentApi.listPicks).not.toHaveBeenCalled();
    });

    it('rejects traceability reads', async () => {
      await expect(getLots()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getSerials()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getReceipts()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getTraceabilityPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(lotsApi.list).not.toHaveBeenCalled();
      expect(serialsApi.list).not.toHaveBeenCalled();
      expect(receivingApi.listReceipts).not.toHaveBeenCalled();
    });

    it('rejects manufacturing and quality reads', async () => {
      await expect(getWorkOrders()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getWorkOrder('wo_1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getQualityInspections()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getNonConformanceReports()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getManufacturingPageData()).rejects.toMatchObject(UNAUTHORIZED);
      expect(workOrdersApi.list).not.toHaveBeenCalled();
      expect(qualityApi.listNcrs).not.toHaveBeenCalled();
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('aggregates purchase orders + suppliers in one action', async () => {
      await expect(getPurchasingPageData()).resolves.toEqual({
        purchaseOrders: [],
        suppliers: [],
      });
      expect(purchaseOrdersApi.list).toHaveBeenCalled();
      expect(purchaseOrdersApi.listSuppliers).toHaveBeenCalled();
    });

    it('rejects an empty purchase order id before touching the engine', async () => {
      await expect(getPurchaseOrder('  ')).rejects.toThrow(/id is required/);
      expect(purchaseOrdersApi.get).not.toHaveBeenCalled();
    });

    it('passes the warehouse id through to the location accessor', async () => {
      await getWarehouseLocations(3);
      expect(warehouseApi.listLocations).toHaveBeenCalledWith(3);

      await getWarehouseLocations();
      expect(warehouseApi.listLocations).toHaveBeenLastCalledWith(undefined);
    });

    it('rejects a non-positive warehouse id', async () => {
      await expect(getWarehouseLocations(0)).rejects.toThrow(/positive integer/);
      await expect(getCycleCounts({ warehouseId: -2 })).rejects.toThrow(/positive integer/);
      expect(cycleCountsApi.list).not.toHaveBeenCalled();
    });

    it('passes the cycle count filter through untouched', async () => {
      const filter = { warehouseId: 2, status: 'completed', limit: 10, offset: 5 };
      await getCycleCounts(filter);
      expect(cycleCountsApi.list).toHaveBeenCalledWith(filter);
    });

    it('aggregates warehouses + locations + cycle counts', async () => {
      await expect(getWarehousePageData()).resolves.toEqual({
        warehouses: [],
        locations: [],
        cycleCounts: [],
      });
      expect(warehouseApi.listWarehouses).toHaveBeenCalled();
      expect(warehouseApi.listLocations).toHaveBeenCalled();
      expect(cycleCountsApi.list).toHaveBeenCalled();
    });

    it('aggregates work orders + inspections + NCRs', async () => {
      await expect(getManufacturingPageData()).resolves.toEqual({
        workOrders: [],
        inspections: [],
        ncrs: [],
      });
      expect(workOrdersApi.list).toHaveBeenCalled();
      expect(qualityApi.listInspections).toHaveBeenCalled();
      expect(qualityApi.listNcrs).toHaveBeenCalled();
    });

    it('rejects an empty work order id before touching the engine', async () => {
      await expect(getWorkOrder('')).rejects.toThrow(/id is required/);
      expect(workOrdersApi.get).not.toHaveBeenCalled();
    });

    it('aggregates waves + pick tasks', async () => {
      await expect(getFulfillmentPageData()).resolves.toEqual({ waves: [], picks: [] });
      expect(fulfillmentApi.listWaves).toHaveBeenCalled();
      expect(fulfillmentApi.listPicks).toHaveBeenCalled();
    });

    it('filters waves by status and rejects a blank status', async () => {
      vi.mocked(fulfillmentApi.listWaves).mockResolvedValueOnce([
        { id: 'w1', waveNumber: 'WAVE-1', warehouseId: 1, orderCount: 2, status: 'released', createdAt: 'x' },
        { id: 'w2', waveNumber: 'WAVE-2', warehouseId: 1, orderCount: 3, status: 'completed', createdAt: 'x' },
      ]);
      await expect(getFulfillmentWaves('released')).resolves.toMatchObject([{ id: 'w1' }]);

      await expect(getFulfillmentWaves('   ')).rejects.toThrow(/status is required/);
    });

    it('filters lots by status and rejects a blank status', async () => {
      vi.mocked(lotsApi.list).mockResolvedValueOnce([
        {
          id: 'l1', lotNumber: 'LOT-1', sku: 'SKU-1', quantityProduced: 10,
          quantityAvailable: 10, quantityReserved: 0, status: 'active', createdAt: 'x',
        },
        {
          id: 'l2', lotNumber: 'LOT-2', sku: 'SKU-2', quantityProduced: 5,
          quantityAvailable: 0, quantityReserved: 0, status: 'expired', createdAt: 'x',
        },
      ]);
      await expect(getLots('expired')).resolves.toMatchObject([{ id: 'l2' }]);

      await expect(getLots('')).rejects.toThrow(/status is required/);
    });

    it('aggregates lots + serials + receipts', async () => {
      await expect(getTraceabilityPageData()).resolves.toEqual({
        lots: [],
        serials: [],
        receipts: [],
      });
      expect(lotsApi.list).toHaveBeenCalled();
      expect(serialsApi.list).toHaveBeenCalled();
      expect(receivingApi.listReceipts).toHaveBeenCalled();
    });
  });
});
