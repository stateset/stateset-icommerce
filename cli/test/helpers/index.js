/**
 * Test Helpers
 *
 * Re-exports all test utilities for convenient importing.
 *
 * Usage:
 *   import { createMockCommerce, testCustomer, assertSuccess } from '../helpers/index.js';
 */

export {
  createMockCommerce,
  createMockLogger,
  createMockTelemetry,
  createMockPermissionGate,
} from './mocks.js';

export {
  testCustomer,
  testCustomer2,
  testOrder,
  testProduct,
  testProduct2,
  testInventoryItem,
  testReturn,
  testCart,
  testCartItem,
  testShippingAddress,
  testPayment,
  testShipment,
  testSupplier,
  testPurchaseOrder,
  testInvoice,
  testWarranty,
  testPromotion,
  testCoupon,
  testSubscriptionPlan,
  testBom,
  testWorkOrder,
} from './fixtures.js';

export {
  assertSuccess,
  assertError,
  assertPreview,
  assertHasField,
  assertListCount,
} from './assertions.js';
