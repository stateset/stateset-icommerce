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
  createOrder,
  updateOrderStatus,
  cancelOrder,
  adjustInventory,
  reserveInventory,
  releaseInventory,
  createReturn,
  approveReturn,
  rejectReturn,
  receiveReturn,
  processRefund,
  createCustomer,
  updateCustomer,
  createSubscription,
  pauseSubscription,
  resumeSubscription,
  cancelSubscription,
  createProduct,
  updateProduct,
  deleteProduct,
} from '@/app/actions/commerce';
import {
  ordersApi,
  inventoryApi,
  returnsApi,
  customersApi,
  subscriptionsApi,
  productsApi,
} from '@/lib/embedded';

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

// ===========================================================================
// Input validation
// ===========================================================================
//
// Server actions are a public RPC surface — Next exposes each one at a stable
// action id and the browser posts JSON to it, so the declared parameter types
// are erased before the engine ever sees the payload. Every mutating action
// must therefore validate at runtime. One invalid-input case per action,
// each asserting the typed ValidationError *and* that the embedded engine was
// never reached.

const VALIDATION_ERROR = {
  name: 'ValidationError',
  statusCode: 422,
  code: 'VALIDATION_ERROR',
};

describe('commerce action input validation', () => {
  beforeEach(() => {
    cookieStore.set(ADMIN_SESSION_COOKIE, { value: 'test-session-token' });
  });

  it('createOrder rejects a negative line quantity', async () => {
    await expect(
      createOrder({
        customerId: 'cust-1',
        items: [{ productId: 'prod-1', quantity: -1 }],
      })
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(ordersApi.create).not.toHaveBeenCalled();
  });

  it('createOrder rejects an empty basket', async () => {
    await expect(createOrder({ customerId: 'cust-1', items: [] })).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(ordersApi.create).not.toHaveBeenCalled();
  });

  it('updateOrderStatus rejects a status outside the state machine', async () => {
    await expect(
      updateOrderStatus('order-1', 'teleported' as never)
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(ordersApi.updateStatus).not.toHaveBeenCalled();
  });

  it('cancelOrder rejects a missing id', async () => {
    await expect(cancelOrder('', 'no longer wanted')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(ordersApi.cancel).not.toHaveBeenCalled();
  });

  it('adjustInventory rejects a fractional unit count', async () => {
    // A negative delta is a legitimate write-off; a fractional one is not —
    // the engine counts discrete units.
    await expect(adjustInventory('SKU-1', 2.5, 'recount')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(inventoryApi.adjust).not.toHaveBeenCalled();
  });

  it('adjustInventory rejects a zero-unit no-op', async () => {
    await expect(adjustInventory('SKU-1', 0, 'recount')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(inventoryApi.adjust).not.toHaveBeenCalled();
  });

  it('adjustInventory still allows a negative delta (shrinkage)', async () => {
    await adjustInventory('SKU-1', -3, 'damaged in transit');
    expect(inventoryApi.adjust).toHaveBeenCalledWith('SKU-1', -3, 'damaged in transit');
  });

  it('reserveInventory rejects a negative quantity', async () => {
    await expect(reserveInventory('SKU-1', -3, 'order-1')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(inventoryApi.reserve).not.toHaveBeenCalled();
  });

  it('releaseInventory rejects a zero quantity', async () => {
    await expect(releaseInventory('SKU-1', 0, 'order-1')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(inventoryApi.release).not.toHaveBeenCalled();
  });

  it('createReturn rejects an unknown reason category', async () => {
    await expect(
      createReturn({
        orderId: 'order-1',
        items: [{ productId: 'prod-1', quantity: 1 }],
        reason: 'arrived broken',
        reasonCategory: 'because' as never,
      })
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(returnsApi.create).not.toHaveBeenCalled();
  });

  it('approveReturn rejects a missing id', async () => {
    await expect(approveReturn('')).rejects.toMatchObject(VALIDATION_ERROR);
    expect(returnsApi.approve).not.toHaveBeenCalled();
  });

  it('rejectReturn rejects an empty reason', async () => {
    await expect(rejectReturn('ret-1', '')).rejects.toMatchObject(VALIDATION_ERROR);
    expect(returnsApi.reject).not.toHaveBeenCalled();
  });

  it('receiveReturn rejects an unknown item condition', async () => {
    await expect(
      receiveReturn('ret-1', [{ productId: 'prod-1', condition: 'pristine' }])
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(returnsApi.receive).not.toHaveBeenCalled();
  });

  it('processRefund rejects an unknown refund method', async () => {
    await expect(processRefund('ret-1', 'bitcoin' as never)).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(returnsApi.processRefund).not.toHaveBeenCalled();
  });

  it('processRefund rejects a path-traversal id', async () => {
    await expect(processRefund('../../admin', 'original')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(returnsApi.processRefund).not.toHaveBeenCalled();
  });

  it('createCustomer rejects a malformed email', async () => {
    await expect(createCustomer({ email: 'not-an-email' })).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(customersApi.create).not.toHaveBeenCalled();
  });

  it('updateCustomer rejects a missing id', async () => {
    await expect(updateCustomer('', { firstName: 'Ada' })).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(customersApi.update).not.toHaveBeenCalled();
  });

  it('createSubscription rejects a fractional quantity', async () => {
    await expect(
      createSubscription({ customerId: 'cust-1', quantity: 1.5 })
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(subscriptionsApi.create).not.toHaveBeenCalled();
  });

  it('pauseSubscription rejects a missing id', async () => {
    await expect(pauseSubscription('')).rejects.toMatchObject(VALIDATION_ERROR);
    expect(subscriptionsApi.pause).not.toHaveBeenCalled();
  });

  it('resumeSubscription rejects an id with path separators', async () => {
    await expect(resumeSubscription('sub/../other')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(subscriptionsApi.resume).not.toHaveBeenCalled();
  });

  it('cancelSubscription rejects a missing id', async () => {
    await expect(cancelSubscription('', 'too expensive')).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(subscriptionsApi.cancel).not.toHaveBeenCalled();
  });

  it('createProduct rejects a price with sub-cent float precision', async () => {
    await expect(
      createProduct({ sku: 'SKU-1', name: 'Widget', price: 10.999999999999998 })
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(productsApi.create).not.toHaveBeenCalled();
  });

  it('createProduct rejects a non-finite price', async () => {
    await expect(
      createProduct({ sku: 'SKU-1', name: 'Widget', price: Number.NaN })
    ).rejects.toMatchObject(VALIDATION_ERROR);
    expect(productsApi.create).not.toHaveBeenCalled();
  });

  it('updateProduct rejects a negative price', async () => {
    await expect(updateProduct('prod-1', { price: -5 })).rejects.toMatchObject(
      VALIDATION_ERROR
    );
    expect(productsApi.update).not.toHaveBeenCalled();
  });

  it('deleteProduct rejects a missing id', async () => {
    await expect(deleteProduct('')).rejects.toMatchObject(VALIDATION_ERROR);
    expect(productsApi.delete).not.toHaveBeenCalled();
  });

  it('names the offending field in the error details', async () => {
    await expect(
      createOrder({ customerId: 'cust-1', items: [{ productId: 'p1', quantity: -1 }] })
    ).rejects.toMatchObject({
      details: [{ field: 'items.0.quantity' }],
    });
  });

  it('rejects unauthenticated callers before it validates', async () => {
    // Auth first: an anonymous caller must not be able to probe the schema
    // by watching which payloads come back 401 vs 422.
    cookieStore.clear();
    await expect(adjustInventory('SKU-1', 2.5, 'recount')).rejects.toMatchObject({
      statusCode: 401,
    });
  });

  it('lets well-formed payloads through untouched', async () => {
    await createOrder({
      customerId: 'cust-1',
      items: [{ productId: 'prod-1', quantity: 2 }],
      shippingAddress: {
        line1: '1 Market St',
        city: 'San Francisco',
        state: 'CA',
        postalCode: '94105',
        country: 'US',
      },
    });
    expect(ordersApi.create).toHaveBeenCalledWith({
      customerId: 'cust-1',
      items: [{ productId: 'prod-1', quantity: 2 }],
      shippingAddress: {
        line1: '1 Market St',
        city: 'San Francisco',
        state: 'CA',
        postalCode: '94105',
        country: 'US',
      },
    });

    await receiveReturn('ret-1', [{ productId: 'prod-1', condition: 'opened' }]);
    expect(returnsApi.receive).toHaveBeenCalledWith('ret-1', [
      { productId: 'prod-1', condition: 'opened' },
    ]);

    await createProduct({ sku: 'SKU-1', name: 'Widget', price: 10.99, currency: 'USD' });
    expect(productsApi.create).toHaveBeenCalledWith({
      sku: 'SKU-1',
      name: 'Widget',
      price: 10.99,
      currency: 'USD',
    });
  });
});
