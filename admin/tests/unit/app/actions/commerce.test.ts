/**
 * Tests for the commerce server actions auth guard
 *
 * Server actions bypass the API middleware, so every exported action in
 * `@/app/actions/commerce` must enforce the admin session itself via
 * `requireAdminSession()`. These tests lock down that contract.
 *
 * @module tests/unit/app/actions/commerce
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

// Mock the embedded commerce engine so actions never touch a real backend.
vi.mock('@/lib/embedded', () => ({
  ordersApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    create: vi.fn(),
    updateStatus: vi.fn().mockResolvedValue({ id: 'order-1', status: 'confirmed' }),
    cancel: vi.fn().mockResolvedValue({ id: 'order-1', status: 'cancelled' }),
    getAnalytics: vi.fn(),
  },
  inventoryApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    adjust: vi.fn().mockResolvedValue({ id: 'adj-1', sku: 'SKU-1', newQuantity: 5 }),
    reserve: vi.fn(),
    release: vi.fn(),
    getLowStock: vi.fn(),
    getAnalytics: vi.fn(),
    forecast: vi.fn(),
  },
  returnsApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    create: vi.fn(),
    approve: vi.fn().mockResolvedValue({ id: 'ret-1', status: 'approved' }),
    reject: vi.fn(),
    receive: vi.fn(),
    processRefund: vi.fn().mockResolvedValue({ id: 'ret-1', status: 'refunded' }),
    getAnalytics: vi.fn(),
  },
  customersApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    getByEmail: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    getOrders: vi.fn(),
    getHealthScore: vi.fn(),
    getSegments: vi.fn(),
    getAnalytics: vi.fn(),
  },
  subscriptionsApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    create: vi.fn(),
    pause: vi.fn(),
    resume: vi.fn(),
    cancel: vi.fn(),
    getAnalytics: vi.fn(),
  },
  analyticsApi: {
    getDashboardMetrics: vi.fn().mockResolvedValue({ totalOrders: 0 }),
    getHourlyActivity: vi.fn(),
    getSystemHealth: vi.fn(),
    getRevenueByPeriod: vi.fn(),
    getTopProducts: vi.fn(),
    getConversionFunnel: vi.fn(),
  },
  productsApi: {
    list: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue(null),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn().mockResolvedValue({ deleted: true }),
  },
}));

import {
  getDashboardMetrics,
  getOrders,
  updateOrderStatus,
  cancelOrder,
  adjustInventory,
  approveReturn,
  processRefund,
  deleteProduct,
} from '@/app/actions/commerce';
import { ordersApi, inventoryApi, returnsApi, productsApi } from '@/lib/embedded';

beforeEach(() => {
  cookieStore.clear();
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const UNAUTHORIZED = { statusCode: 401, code: 'UNAUTHORIZED' };

describe('commerce actions auth guard', () => {
  describe('without a session', () => {
    it('rejects updateOrderStatus and never reaches the embedded engine', async () => {
      await expect(updateOrderStatus('order-1', 'confirmed')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(ordersApi.updateStatus).not.toHaveBeenCalled();
    });

    it('rejects cancelOrder', async () => {
      await expect(cancelOrder('order-1', 'changed mind')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(ordersApi.cancel).not.toHaveBeenCalled();
    });

    it('rejects adjustInventory', async () => {
      await expect(adjustInventory('SKU-1', 5, 'recount')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(inventoryApi.adjust).not.toHaveBeenCalled();
    });

    it('rejects approveReturn and processRefund', async () => {
      await expect(approveReturn('ret-1')).rejects.toMatchObject(UNAUTHORIZED);
      await expect(processRefund('ret-1', 'original')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(returnsApi.approve).not.toHaveBeenCalled();
      expect(returnsApi.processRefund).not.toHaveBeenCalled();
    });

    it('rejects deleteProduct', async () => {
      await expect(deleteProduct('prod-1')).rejects.toMatchObject(UNAUTHORIZED);
      expect(productsApi.delete).not.toHaveBeenCalled();
    });

    it('rejects read actions too', async () => {
      await expect(getOrders()).rejects.toMatchObject(UNAUTHORIZED);
      await expect(getDashboardMetrics()).rejects.toMatchObject(UNAUTHORIZED);
      expect(ordersApi.list).not.toHaveBeenCalled();
    });

    it('ignores a whitespace-only session cookie', async () => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: '   ' });
      await expect(updateOrderStatus('order-1', 'confirmed')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(ordersApi.updateStatus).not.toHaveBeenCalled();
    });
  });

  describe('with a valid session cookie', () => {
    beforeEach(() => {
      cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
    });

    it('allows updateOrderStatus through to the embedded engine', async () => {
      const result = await updateOrderStatus('order-1', 'confirmed');

      expect(result).toMatchObject({ id: 'order-1', status: 'confirmed' });
      expect(ordersApi.updateStatus).toHaveBeenCalledWith('order-1', 'confirmed');
    });

    it('allows adjustInventory', async () => {
      await adjustInventory('SKU-1', 5, 'recount');
      expect(inventoryApi.adjust).toHaveBeenCalledWith('SKU-1', 5, 'recount');
    });

    it('allows approveReturn', async () => {
      const result = await approveReturn('ret-1');

      expect(result).toMatchObject({ id: 'ret-1', status: 'approved' });
      expect(returnsApi.approve).toHaveBeenCalledWith('ret-1');
    });

    it('allows read actions', async () => {
      await expect(getOrders({ limit: 10 })).resolves.toEqual([]);
      expect(ordersApi.list).toHaveBeenCalledWith({ limit: 10 });
    });
  });

  describe('when admin auth is disabled (dev mode)', () => {
    it('skips the session requirement, mirroring the middleware bypass', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      const result = await updateOrderStatus('order-1', 'confirmed');

      expect(result).toMatchObject({ id: 'order-1', status: 'confirmed' });
      expect(ordersApi.updateStatus).toHaveBeenCalledWith('order-1', 'confirmed');
    });

    it('still requires a session in production even with the flag set', async () => {
      vi.stubEnv('NODE_ENV', 'production');
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      await expect(updateOrderStatus('order-1', 'confirmed')).rejects.toMatchObject(
        UNAUTHORIZED
      );
      expect(ordersApi.updateStatus).not.toHaveBeenCalled();
    });
  });
});
