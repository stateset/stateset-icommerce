/**
 * Supply Chain Parity Tools Test Suite
 *
 * Tests for the nine tool modules added to bring the MCP surface to parity
 * with the commerce binding: edi-documents, prepayments, vendor-credits,
 * price-schedules, price-levels, transfer-orders, production-batches,
 * supplier-skus, and inbound-shipments, plus the list_gl_periods extension.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { ediDocumentTools } from '../../src/tools/edi-documents.js';
import { prepaymentTools } from '../../src/tools/prepayments.js';
import { vendorCreditTools } from '../../src/tools/vendor-credits.js';
import { priceScheduleTools } from '../../src/tools/price-schedules.js';
import { priceLevelTools } from '../../src/tools/price-levels.js';
import { transferOrderTools } from '../../src/tools/transfer-orders.js';
import { productionBatchTools } from '../../src/tools/production-batches.js';
import { supplierSkuTools } from '../../src/tools/supplier-skus.js';
import { inboundShipmentTools } from '../../src/tools/inbound-shipments.js';
import { generalLedgerTools } from '../../src/tools/general-ledger.js';
import { DOMAIN_TOOL_ARRAYS, TOOL_POLICY_DOMAIN_BY_NAME } from '../../src/tools/domain-registry.js';

const NEW_MODULES = [
  ['edi-documents', ediDocumentTools, 'edi_documents'],
  ['prepayments', prepaymentTools, 'prepayments'],
  ['vendor-credits', vendorCreditTools, 'vendor_credits'],
  ['price-schedules', priceScheduleTools, 'price_schedules'],
  ['price-levels', priceLevelTools, 'price_levels'],
  ['transfer-orders', transferOrderTools, 'transfer_orders'],
  ['production-batches', productionBatchTools, 'production_batches'],
  ['supplier-skus', supplierSkuTools, 'supplier_skus'],
  ['inbound-shipments', inboundShipmentTools, 'inbound_shipments'],
];

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// REGISTRY
// ============================================================================

describe('Supply chain parity domain registry', () => {
  it('registers all nine new modules', () => {
    for (const [moduleName, tools] of NEW_MODULES) {
      assert.equal(DOMAIN_TOOL_ARRAYS[moduleName], tools, moduleName);
    }
  });

  it('assigns policy domains to every tool', () => {
    for (const [moduleName, tools, policyDomain] of NEW_MODULES) {
      for (const tool of tools) {
        assert.equal(tool.policyDomain, policyDomain, `${moduleName}:${tool.name}`);
        assert.equal(TOOL_POLICY_DOMAIN_BY_NAME[tool.name], policyDomain, tool.name);
      }
    }
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.list_gl_periods, 'general_ledger');
  });

  it('marks read tools read and write tools write', () => {
    for (const [moduleName, tools] of NEW_MODULES) {
      for (const tool of tools) {
        const expected =
          tool.name.startsWith('list_') ||
          tool.name.startsWith('get_') ||
          tool.name.startsWith('check_') ||
          tool.name.startsWith('resolve_')
            ? 'read'
            : 'write';
        assert.equal(tool.permission, expected, `${moduleName}:${tool.name}`);
      }
    }
    assert.equal(findTool(generalLedgerTools, 'list_gl_periods').permission, 'read');
  });
});

// ============================================================================
// APPLY GUARD
// ============================================================================

describe('Supply chain parity apply guard', () => {
  it('write tools refuse to mutate without allowApply', async () => {
    const apiKeys = [
      'ediDocuments',
      'prepayments',
      'vendorCredits',
      'priceSchedules',
      'priceLevels',
      'transferOrders',
      'productionBatches',
      'supplierSkus',
      'inboundShipments',
    ];
    for (const [moduleName, tools] of NEW_MODULES) {
      for (const tool of tools.filter((t) => t.permission === 'write')) {
        let called = false;
        const trap = new Proxy(
          {},
          {
            get: () => async () => {
              called = true;
              return {};
            },
          },
        );
        const commerce = Object.fromEntries(apiKeys.map((key) => [key, trap]));
        const result = await tool.handler({ commerce, params: {}, allowApply: false });
        assert.equal(result.success, false, `${moduleName}:${tool.name}`);
        assert.equal(called, false, `${tool.name} must not call the API without allowApply`);
      }
    }
  });
});

// ============================================================================
// EDI DOCUMENTS
// ============================================================================

describe('EDI Document Tools', () => {
  const mockDocument = {
    id: 'edi_001',
    documentType: '850',
    direction: 'inbound',
    status: 'pending',
    partner: 'ACME',
    createdAt: '2026-07-01T00:00:00Z',
    updatedAt: '2026-07-01T00:00:00Z',
  };

  function makeCommerce(overrides = {}) {
    return {
      ediDocuments: {
        list: async () => [mockDocument],
        get: async () => mockDocument,
        create: async (data) => ({ ...mockDocument, ...data }),
        setStatus: async (id, status, errorMessage) => ({
          ...mockDocument,
          id,
          status,
          errorMessage,
        }),
        summary: async () => ({ total: 1, byStatus: [], byType: [] }),
        ...overrides,
      },
    };
  }

  it('list_edi_documents forwards the filter', async () => {
    let received;
    const commerce = makeCommerce({
      list: async (filter) => {
        received = filter;
        return [mockDocument];
      },
    });
    const result = await findTool(ediDocumentTools, 'list_edi_documents').handler({
      commerce,
      params: { documentType: '850', direction: 'inbound', status: 'pending', limit: 10 },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(received.documentType, '850');
    assert.equal(received.limit, 10);
  });

  it('get_edi_document returns error when not found', async () => {
    const result = await findTool(ediDocumentTools, 'get_edi_document').handler({
      commerce: makeCommerce({ get: async () => null }),
      params: { documentId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'EDI document not found');
  });

  it('set_edi_document_status forwards error message', async () => {
    const result = await findTool(ediDocumentTools, 'set_edi_document_status').handler({
      commerce: makeCommerce(),
      params: { documentId: 'edi_001', status: 'error', errorMessage: 'Bad segment' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.document.status, 'error');
    assert.equal(result.document.errorMessage, 'Bad segment');
  });

  it('get_edi_summary returns the summary', async () => {
    const result = await findTool(ediDocumentTools, 'get_edi_summary').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.summary.total, 1);
  });
});

// ============================================================================
// PREPAYMENTS / VENDOR CREDITS
// ============================================================================

describe('Prepayment Tools', () => {
  const mockPrepayment = {
    id: 'pp_001',
    number: 'PP-1',
    supplierId: 'sup_001',
    amount: '1000.00',
    remaining: '1000.00',
    currency: 'USD',
    status: 'open',
  };

  function makeCommerce(overrides = {}) {
    return {
      prepayments: {
        isSupported: async () => true,
        list: async () => [mockPrepayment],
        get: async () => mockPrepayment,
        create: async (data) => ({ ...mockPrepayment, ...data }),
        apply: async (id, data) => ({ ...mockPrepayment, id, ...data }),
        listApplications: async () => [{ id: 'app_001', amount: '250.00', reversed: false }],
        reverseApplication: async () => mockPrepayment,
        refund: async (id) => ({ ...mockPrepayment, id, status: 'refunded' }),
        ...overrides,
      },
    };
  }

  it('check_prepayments_supported reports support', async () => {
    const result = await findTool(prepaymentTools, 'check_prepayments_supported').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.supported, true);
  });

  it('create_prepayment passes decimal strings through', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockPrepayment;
      },
    });
    const result = await findTool(prepaymentTools, 'create_prepayment').handler({
      commerce,
      params: { supplierId: 'sup_001', amount: '1000.00', currency: 'USD', method: 'wire' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.amount, '1000.00');
    assert.equal(received.method, 'wire');
  });

  it('apply_prepayment forwards the application input', async () => {
    let received;
    const commerce = makeCommerce({
      apply: async (id, data) => {
        received = { id, ...data };
        return mockPrepayment;
      },
    });
    const result = await findTool(prepaymentTools, 'apply_prepayment').handler({
      commerce,
      params: {
        prepaymentId: 'pp_001',
        targetType: 'bill',
        targetId: 'bill_001',
        amount: '250.00',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received, {
      id: 'pp_001',
      targetType: 'bill',
      targetId: 'bill_001',
      amount: '250.00',
    });
  });

  it('refund_prepayment refunds by id', async () => {
    const result = await findTool(prepaymentTools, 'refund_prepayment').handler({
      commerce: makeCommerce(),
      params: { prepaymentId: 'pp_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.prepayment.status, 'refunded');
  });
});

describe('Vendor Credit Tools', () => {
  const mockCredit = {
    id: 'vc_001',
    supplierId: 'sup_001',
    amount: '250.00',
    status: 'open',
  };

  function makeCommerce(overrides = {}) {
    return {
      vendorCredits: {
        isSupported: async () => true,
        list: async () => [mockCredit],
        get: async () => mockCredit,
        create: async (data) => ({ ...mockCredit, ...data }),
        apply: async (id, data) => ({ ...mockCredit, id, ...data }),
        listApplications: async () => [],
        reverseApplication: async () => mockCredit,
        cancel: async (id) => ({ ...mockCredit, id, status: 'cancelled' }),
        ...overrides,
      },
    };
  }

  it('get_vendor_credit returns error when not found', async () => {
    const result = await findTool(vendorCreditTools, 'get_vendor_credit').handler({
      commerce: makeCommerce({ get: async () => null }),
      params: { vendorCreditId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Vendor credit not found');
  });

  it('reverse_vendor_credit_application forwards both ids', async () => {
    let received;
    const commerce = makeCommerce({
      reverseApplication: async (id, applicationId) => {
        received = { id, applicationId };
        return mockCredit;
      },
    });
    const result = await findTool(vendorCreditTools, 'reverse_vendor_credit_application').handler({
      commerce,
      params: { vendorCreditId: 'vc_001', applicationId: 'app_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received, { id: 'vc_001', applicationId: 'app_001' });
  });

  it('cancel_vendor_credit cancels by id', async () => {
    const result = await findTool(vendorCreditTools, 'cancel_vendor_credit').handler({
      commerce: makeCommerce(),
      params: { vendorCreditId: 'vc_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.vendorCredit.status, 'cancelled');
  });
});

// ============================================================================
// PRICE SCHEDULES / PRICE LEVELS
// ============================================================================

describe('Price Schedule Tools', () => {
  const mockSchedule = { id: 'ps_001', name: 'Summer', isActive: true, priority: 0 };

  function makeCommerce(overrides = {}) {
    return {
      priceSchedules: {
        isSupported: async () => true,
        list: async () => [mockSchedule],
        get: async () => mockSchedule,
        create: async (data) => ({ ...mockSchedule, ...data }),
        update: async (id, data) => ({ ...mockSchedule, id, ...data }),
        delete: async () => undefined,
        setEntry: async (id, productId, price) => ({ scheduleId: id, productId, price }),
        deleteEntry: async () => undefined,
        listEntries: async () => [{ productId: 'prod_001', price: '19.99' }],
        resolvePrice: async () => '19.99',
        ...overrides,
      },
    };
  }

  it('set_price_schedule_entry passes exact decimal string price', async () => {
    const result = await findTool(priceScheduleTools, 'set_price_schedule_entry').handler({
      commerce: makeCommerce(),
      params: { priceScheduleId: 'ps_001', productId: 'prod_001', price: '19.99' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.entry.price, '19.99');
  });

  it('resolve_scheduled_price returns applies=false when no schedule applies', async () => {
    const result = await findTool(priceScheduleTools, 'resolve_scheduled_price').handler({
      commerce: makeCommerce({ resolvePrice: async () => null }),
      params: { productId: 'prod_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.price, null);
    assert.equal(result.applies, false);
  });

  it('delete_price_schedule deletes by id', async () => {
    let received;
    const commerce = makeCommerce({
      delete: async (id) => {
        received = id;
      },
    });
    const result = await findTool(priceScheduleTools, 'delete_price_schedule').handler({
      commerce,
      params: { priceScheduleId: 'ps_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received, 'ps_001');
  });
});

describe('Price Level Tools', () => {
  const mockLevel = { id: 'pl_001', name: 'Wholesale', code: 'WH', isActive: true };

  function makeCommerce(overrides = {}) {
    return {
      priceLevels: {
        isSupported: async () => true,
        list: async () => [mockLevel],
        get: async () => mockLevel,
        create: async (data) => ({ ...mockLevel, ...data }),
        update: async (id, data) => ({ ...mockLevel, id, ...data }),
        delete: async () => undefined,
        setEntry: async (id, productId, price) => ({ levelId: id, productId, price }),
        deleteEntry: async () => undefined,
        listEntries: async () => [{ productId: 'prod_001', price: '15.00' }],
        ...overrides,
      },
    };
  }

  it('create_price_level forwards adjustment fields', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockLevel;
      },
    });
    const result = await findTool(priceLevelTools, 'create_price_level').handler({
      commerce,
      params: {
        name: 'Wholesale',
        code: 'WH',
        adjustmentType: 'percentage_discount',
        adjustmentValue: '10',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.adjustmentType, 'percentage_discount');
    assert.equal(received.adjustmentValue, '10');
  });

  it('list_price_level_entries returns entries with count', async () => {
    const result = await findTool(priceLevelTools, 'list_price_level_entries').handler({
      commerce: makeCommerce(),
      params: { priceLevelId: 'pl_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.entries[0].price, '15.00');
  });
});

// ============================================================================
// TRANSFER ORDERS / INBOUND SHIPMENTS
// ============================================================================

describe('Transfer Order Tools', () => {
  const mockTransfer = { id: 'to_001', status: 'draft' };

  function makeCommerce(overrides = {}) {
    return {
      transferOrders: {
        isSupported: async () => true,
        list: async () => [mockTransfer],
        get: async () => mockTransfer,
        create: async (data) => ({ ...mockTransfer, ...data }),
        ship: async (id) => ({ ...mockTransfer, id, status: 'in_transit' }),
        receiveLine: async (id, itemId, quantity) => ({
          ...mockTransfer,
          id,
          receivedItemId: itemId,
          receivedQuantity: quantity,
        }),
        cancel: async (id) => ({ ...mockTransfer, id, status: 'cancelled' }),
        ...overrides,
      },
    };
  }

  it('create_transfer_order forwards items with decimal quantities', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockTransfer;
      },
    });
    const result = await findTool(transferOrderTools, 'create_transfer_order').handler({
      commerce,
      params: {
        sourceWarehouseId: 'wh_001',
        destinationWarehouseId: 'wh_002',
        items: [{ productId: 'prod_001', quantity: '5' }],
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.items[0].quantity, '5');
    assert.equal(received.destinationWarehouseId, 'wh_002');
  });

  it('receive_transfer_order_line forwards id, item, and quantity', async () => {
    const result = await findTool(transferOrderTools, 'receive_transfer_order_line').handler({
      commerce: makeCommerce(),
      params: { transferOrderId: 'to_001', itemId: 'item_001', quantity: '3' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.transferOrder.receivedItemId, 'item_001');
    assert.equal(result.transferOrder.receivedQuantity, '3');
  });
});

describe('Inbound Shipment Tools', () => {
  const mockShipment = { id: 'is_001', status: 'pending' };

  function makeCommerce(overrides = {}) {
    return {
      inboundShipments: {
        isSupported: async () => true,
        list: async () => [mockShipment],
        get: async () => mockShipment,
        create: async (data) => ({ ...mockShipment, ...data }),
        markInTransit: async (id) => ({ ...mockShipment, id, status: 'in_transit' }),
        markArrived: async (id) => ({ ...mockShipment, id, status: 'arrived' }),
        receiveLine: async (id, itemId, quantity) => ({
          ...mockShipment,
          id,
          receivedItemId: itemId,
          receivedQuantity: quantity,
        }),
        cancel: async (id) => ({ ...mockShipment, id, status: 'cancelled' }),
        ...overrides,
      },
    };
  }

  it('create_inbound_shipment forwards expected line items', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockShipment;
      },
    });
    const result = await findTool(inboundShipmentTools, 'create_inbound_shipment').handler({
      commerce,
      params: {
        supplierId: 'sup_001',
        warehouseId: 'wh_001',
        items: [{ productId: 'prod_001', sku: 'SKU-1', quantityExpected: '10' }],
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.items[0].quantityExpected, '10');
  });

  it('mark_inbound_shipment_arrived updates status', async () => {
    const result = await findTool(inboundShipmentTools, 'mark_inbound_shipment_arrived').handler({
      commerce: makeCommerce(),
      params: { shipmentId: 'is_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.inboundShipment.status, 'arrived');
  });
});

// ============================================================================
// PRODUCTION BATCHES / SUPPLIER SKUS
// ============================================================================

describe('Production Batch Tools', () => {
  const mockBatch = { id: 'pb_001', name: 'Batch 1', status: 'planned' };

  function makeCommerce(overrides = {}) {
    return {
      productionBatches: {
        isSupported: async () => true,
        list: async () => [mockBatch],
        get: async () => mockBatch,
        create: async (data) => ({ ...mockBatch, ...data }),
        update: async (id, data) => ({ ...mockBatch, id, ...data }),
        delete: async () => undefined,
        addWorkOrders: async (id, workOrderIds) => ({ ...mockBatch, id, workOrderIds }),
        removeWorkOrder: async (id) => ({ ...mockBatch, id }),
        ...overrides,
      },
    };
  }

  it('add_production_batch_work_orders links work orders', async () => {
    const result = await findTool(productionBatchTools, 'add_production_batch_work_orders').handler(
      {
        commerce: makeCommerce(),
        params: { batchId: 'pb_001', workOrderIds: ['wo_001', 'wo_002'] },
        allowApply: true,
      },
    );
    assert.equal(result.success, true);
    assert.deepEqual(result.productionBatch.workOrderIds, ['wo_001', 'wo_002']);
  });

  it('get_production_batch returns error when not found', async () => {
    const result = await findTool(productionBatchTools, 'get_production_batch').handler({
      commerce: makeCommerce({ get: async () => null }),
      params: { batchId: 'missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Production batch not found');
  });
});

describe('Supplier SKU Tools', () => {
  const mockSku = { id: 'ssku_001', productId: 'prod_001', supplierId: 'sup_001', sku: 'ACME-1' };

  function makeCommerce(overrides = {}) {
    return {
      supplierSkus: {
        isSupported: async () => true,
        list: async () => [mockSku],
        get: async () => mockSku,
        create: async (data) => ({ ...mockSku, ...data }),
        update: async (id, data) => ({ ...mockSku, id, ...data }),
        delete: async () => undefined,
        bulkUpsert: async (supplierId, items) => items.length,
        ...overrides,
      },
    };
  }

  it('create_supplier_sku passes cost fields through as decimal strings', async () => {
    let received;
    const commerce = makeCommerce({
      create: async (data) => {
        received = data;
        return mockSku;
      },
    });
    const result = await findTool(supplierSkuTools, 'create_supplier_sku').handler({
      commerce,
      params: {
        productId: 'prod_001',
        supplierId: 'sup_001',
        sku: 'ACME-1',
        unitCost: '4.25',
        minOrderQty: '100',
        leadTimeDays: 14,
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.unitCost, '4.25');
    assert.equal(received.minOrderQty, '100');
    assert.equal(received.leadTimeDays, 14);
  });

  it('bulk_upsert_supplier_skus returns the upserted count', async () => {
    const result = await findTool(supplierSkuTools, 'bulk_upsert_supplier_skus').handler({
      commerce: makeCommerce(),
      params: {
        supplierId: 'sup_001',
        items: [
          { productId: 'prod_001', sku: 'ACME-1', unitCost: '4.25' },
          { productId: 'prod_002', sku: 'ACME-2' },
        ],
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.upserted, 2);
  });
});

// ============================================================================
// GENERAL LEDGER PERIODS
// ============================================================================

describe('list_gl_periods', () => {
  it('forwards the filter and returns periods with count', async () => {
    let received;
    const commerce = {
      generalLedger: {
        listPeriods: async (filter) => {
          received = filter;
          return [{ id: 'per_001', periodName: 'January 2026', status: 'open' }];
        },
      },
    };
    const result = await findTool(generalLedgerTools, 'list_gl_periods').handler({
      commerce,
      params: { fiscalYear: 2026, status: 'open', limit: 5 },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(received.fiscalYear, 2026);
    assert.equal(received.status, 'open');
  });
});
