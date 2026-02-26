/**
 * Shipment Tools — Comprehensive Test Suite
 *
 * Tests tool exports from src/tools/shipments.js including:
 *   legacy APIs (list_shipments, create_shipment, deliver_shipment)
 *   provider APIs (list/quote/create/void/track label)
 *   fulfillment exception workflow tool
 */

import { beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { shipmentTools } from '../../src/tools/shipments.js';
import { createPaymentIntent, __resetPaymentProviderState } from '../../src/tools/providers/payments.js';
import { __resetShippingProviderState } from '../../src/tools/providers/shipping.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = shipmentTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in shipmentTools`);
  return tool;
}

function makeShipment(overrides = {}) {
  return {
    id: 'ship_001',
    orderId: 'ord_001',
    carrier: 'FedEx',
    service: 'Ground',
    trackingNumber: 'FDX-123456789',
    status: 'in_transit',
    createdAt: '2026-02-20T00:00:00Z',
    ...overrides,
  };
}

function makeCommerce(overrides = {}) {
  return {
    shipments: {
      list: async () => [makeShipment()],
      count: async () => 1,
      create: async (data) => makeShipment({ id: 'ship_new', ...data }),
      deliver: async (id) => makeShipment({ id, status: 'delivered' }),
      ...overrides,
    },
    returns: {
      create: async (data) => ({ id: 'ret_001', ...data }),
    },
  };
}

const originAddress = {
  line1: '1 Warehouse Way',
  city: 'Los Angeles',
  state: 'CA',
  postalCode: '90001',
  country: 'US',
};

const destinationAddress = {
  line1: '100 Market St',
  city: 'San Francisco',
  state: 'CA',
  postalCode: '94105',
  country: 'US',
};

const parcels = [{ weightGrams: 850 }];

beforeEach(() => {
  __resetShippingProviderState();
  __resetPaymentProviderState();
});

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Shipment Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(shipmentTools));
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of shipmentTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = shipmentTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });

  it('contains expected legacy and provider tool names', () => {
    const names = shipmentTools.map((t) => t.name);
    const expected = [
      'list_shipments',
      'create_shipment',
      'deliver_shipment',
      'list_shipping_providers',
      'quote_shipping_rates',
      'create_shipping_label',
      'void_shipping_label',
      'track_shipping_label',
      'list_shipping_labels',
      'ingest_shipping_provider_webhook',
      'handle_fulfillment_exception',
    ];
    for (const name of expected) {
      assert.ok(names.includes(name), `missing tool: ${name}`);
    }
  });
});

// ---------------------------------------------------------------------------
// list_shipments
// ---------------------------------------------------------------------------

describe('list_shipments', () => {
  const tool = findTool('list_shipments');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns success with shipments and count', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
    assert.ok(Array.isArray(result.shipments));
    assert.strictEqual(result.shipments.length, 1);
  });

  it('returns empty list when no shipments', async () => {
    const commerce = makeCommerce({ list: async () => [], count: async () => 0 });
    const result = await tool.handler({ commerce, params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 0);
    assert.strictEqual(result.shipments.length, 0);
  });
});

// ---------------------------------------------------------------------------
// create_shipment
// ---------------------------------------------------------------------------

describe('create_shipment', () => {
  const tool = findTool('create_shipment');
  const params = { orderId: 'ord_001', carrier: 'FedEx', service: 'Ground' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview from applyRequired when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldDo);
  });

  it('creates shipment when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.strictEqual(result.shipment.orderId, 'ord_001');
  });
});

// ---------------------------------------------------------------------------
// deliver_shipment
// ---------------------------------------------------------------------------

describe('deliver_shipment', () => {
  const tool = findTool('deliver_shipment');
  const params = { shipmentId: 'ship_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview from applyRequired when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldDo);
  });

  it('delivers shipment when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.shipment.status, 'delivered');
  });
});

// ---------------------------------------------------------------------------
// Provider-backed shipping tools
// ---------------------------------------------------------------------------

describe('provider-backed shipping tools', () => {
  const listProvidersTool = findTool('list_shipping_providers');
  const quoteRatesTool = findTool('quote_shipping_rates');
  const createLabelTool = findTool('create_shipping_label');
  const trackLabelTool = findTool('track_shipping_label');
  const listLabelsTool = findTool('list_shipping_labels');
  const voidLabelTool = findTool('void_shipping_label');
  const ingestWebhookTool = findTool('ingest_shipping_provider_webhook');

  it('list_shipping_providers returns available providers', async () => {
    const result = await listProvidersTool.handler({ params: {} });
    assert.strictEqual(result.success, true);
    assert.ok(result.count >= 2);
    assert.ok(result.providers.some((provider) => provider.id === 'deterministic-mock'));
  });

  it('quote_shipping_rates returns rates', async () => {
    const result = await quoteRatesTool.handler({
      params: {
        providerId: 'deterministic-mock',
        originAddress,
        destinationAddress,
        parcels,
        currency: 'USD',
      },
    });
    assert.strictEqual(result.success, true);
    assert.ok(result.count > 0);
    assert.ok(result.rates[0].rateId);
  });

  it('create_shipping_label requires --apply', async () => {
    const result = await createLabelTool.handler({
      params: {
        providerId: 'deterministic-mock',
        originAddress,
        destinationAddress,
        parcels,
      },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('create/track/void shipping label lifecycle works', async () => {
    const created = await createLabelTool.handler({
      params: {
        providerId: 'deterministic-mock',
        originAddress,
        destinationAddress,
        parcels,
        orderId: 'ord_001',
      },
      allowApply: true,
    });
    assert.strictEqual(created.success, true);
    assert.ok(created.label.id);
    assert.equal(created.label.status, 'label_created');

    const tracked = await trackLabelTool.handler({
      params: {
        labelId: created.label.id,
        advanceStatus: true,
      },
    });
    assert.strictEqual(tracked.success, true);
    assert.ok(['in_transit', 'out_for_delivery', 'delivered'].includes(tracked.label.status));

    const voided = await voidLabelTool.handler({
      params: {
        labelId: created.label.id,
        reason: 'customer_changed_address',
      },
      allowApply: true,
    });
    assert.strictEqual(voided.success, true);
    assert.equal(voided.label.status, 'voided');
  });

  it('list_shipping_labels returns created labels', async () => {
    const created = await createLabelTool.handler({
      params: {
        providerId: 'deterministic-mock',
        originAddress,
        destinationAddress,
        parcels,
        orderId: 'ord_list_ship_1',
      },
      allowApply: true,
    });

    const listed = await listLabelsTool.handler({
      params: { orderId: 'ord_list_ship_1' },
    });
    assert.strictEqual(listed.success, true);
    assert.ok(listed.count >= 1);
    assert.ok(listed.labels.some((label) => label.id === created.label.id));
  });

  it('ingest_shipping_provider_webhook updates label status', async () => {
    const created = await createLabelTool.handler({
      params: {
        providerId: 'carrier-hub',
        originAddress,
        destinationAddress,
        parcels,
      },
      allowApply: true,
    });

    const result = await ingestWebhookTool.handler({
      params: {
        providerId: 'carrier-hub',
        eventType: 'shipment.delivered',
        eventId: 'ship_evt_1',
        payload: {
          tracking_number: created.label.trackingNumber,
          description: 'Delivered by carrier',
        },
      },
      allowApply: true,
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.webhook.action, 'status_updated');
    assert.strictEqual(result.webhook.label.status, 'delivered');
  });

  it('ingest_shipping_provider_webhook is idempotent for duplicate events', async () => {
    const created = await createLabelTool.handler({
      params: {
        providerId: 'carrier-hub',
        originAddress,
        destinationAddress,
        parcels,
      },
      allowApply: true,
    });

    const params = {
      providerId: 'carrier-hub',
      eventType: 'shipment.in_transit',
      eventId: 'ship_evt_dup',
      payload: {
        tracking_number: created.label.trackingNumber,
      },
    };
    const first = await ingestWebhookTool.handler({ params, allowApply: true });
    const second = await ingestWebhookTool.handler({ params, allowApply: true });

    assert.strictEqual(first.webhook.idempotent, false);
    assert.strictEqual(second.webhook.idempotent, true);
  });
});

// ---------------------------------------------------------------------------
// Fulfillment exception workflow
// ---------------------------------------------------------------------------

describe('handle_fulfillment_exception', () => {
  const tool = findTool('handle_fulfillment_exception');
  const createLabelTool = findTool('create_shipping_label');

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeCommerce(),
      params: {
        exceptionType: 'carrier_failure',
        orderId: 'ord_123',
      },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldDo.workflowPlan);
  });

  it('executes carrier_failure workflow and creates replacement label when autoExecuteCompensation is true', async () => {
    const originalLabel = await createLabelTool.handler({
      params: {
        providerId: 'deterministic-mock',
        originAddress,
        destinationAddress,
        parcels,
        orderId: 'ord_123',
      },
      allowApply: true,
    });

    const result = await tool.handler({
      commerce: makeCommerce(),
      params: {
        exceptionType: 'carrier_failure',
        orderId: 'ord_123',
        labelId: originalLabel.label.id,
        autoExecuteCompensation: true,
        details: {
          providerId: 'deterministic-mock',
          originAddress,
          destinationAddress,
          parcels,
          serviceCode: 'ground',
        },
      },
      allowApply: true,
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.artifacts.tracking);
    assert.ok(result.artifacts.replacementLabel);
    assert.ok(result.execution.some((step) => step.action === 'create_replacement_label'));
  });

  it('executes split_tender_failure workflow by canceling payment intent', async () => {
    const intent = createPaymentIntent({
      providerId: 'deterministic-mock',
      amount: 50,
      currency: 'USD',
      captureMethod: 'manual',
      orderId: 'ord_987',
    }).intent;

    const result = await tool.handler({
      commerce: makeCommerce(),
      params: {
        exceptionType: 'split_tender_failure',
        orderId: 'ord_987',
        paymentIntentId: intent.id,
        autoExecuteCompensation: true,
        details: {
          reason: 'secondary_tender_timeout',
        },
      },
      allowApply: true,
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.artifacts.paymentCompensation);
    assert.strictEqual(result.artifacts.paymentCompensation.intent.status, 'canceled');
  });

  it('executes partial_shipment workflow by creating follow-up shipment', async () => {
    let calledWith = null;
    const commerce = makeCommerce({
      create: async (data) => {
        calledWith = data;
        return makeShipment({ id: 'ship_follow_up', ...data });
      },
    });

    const result = await tool.handler({
      commerce,
      params: {
        exceptionType: 'partial_shipment',
        orderId: 'ord_partial_1',
        shipmentId: 'ship_001',
        autoExecuteCompensation: true,
        details: {
          carrier: 'UPS',
          service: 'Ground',
          remainingItems: [{ sku: 'SKU-1', quantity: 1 }],
        },
      },
      allowApply: true,
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.artifacts.followUpShipment);
    assert.strictEqual(calledWith.orderId, 'ord_partial_1');
    assert.strictEqual(calledWith.reason, 'partial_shipment_compensation');
  });
});
