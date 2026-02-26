/**
 * Shipment Tools Module
 *
 * MCP tool definitions for shipment tracking and delivery operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';
import {
  createShippingLabel,
  ingestShippingProviderWebhook,
  listShippingLabels,
  listShippingProviders,
  quoteShippingRates,
  trackShippingLabel,
  voidShippingLabel,
} from './providers/shipping.js';
import { cancelPaymentIntent, refundPaymentIntent } from './providers/payments.js';
import { deterministicId } from './providers/runtime.js';

const addressSchema = z.object({
  name: z.string().max(255).optional().describe('Recipient or sender name'),
  line1: z.string().min(1).max(255).describe('Address line 1'),
  line2: z.string().max(255).optional().describe('Address line 2'),
  city: z.string().min(1).max(120).describe('City'),
  state: z.string().max(120).optional().describe('State/province'),
  postalCode: z.string().min(1).max(20).describe('Postal code'),
  country: z.string().min(2).max(3).describe('Country code'),
  phone: z.string().max(50).optional().describe('Phone number'),
});

const parcelSchema = z.object({
  weightGrams: z.number().positive().describe('Parcel weight in grams'),
  lengthCm: z.number().positive().optional().describe('Length in centimeters'),
  widthCm: z.number().positive().optional().describe('Width in centimeters'),
  heightCm: z.number().positive().optional().describe('Height in centimeters'),
});

const itemSchema = z.object({
  sku: z.string().min(1).describe('Item SKU'),
  quantity: z.number().int().positive().describe('Item quantity'),
  weightGrams: z.number().positive().optional().describe('Per-unit weight in grams'),
});

function parcelsFromInputs(parcels, items) {
  if (Array.isArray(parcels) && parcels.length > 0) {
    return parcels;
  }
  if (!Array.isArray(items) || items.length === 0) {
    return [];
  }
  return items.map((item) => ({
    weightGrams: (item.weightGrams || 500) * item.quantity,
  }));
}

function buildExceptionPlan(params) {
  const exceptionType = params.exceptionType;
  const stepsByType = {
    carrier_failure: [
      'Inspect latest tracking event and carrier status.',
      'Re-quote shipping rates for replacement service.',
      'Create replacement shipping label and notify customer.',
    ],
    partial_shipment: [
      'Identify remaining unfulfilled items.',
      'Create follow-up shipment for remaining quantities.',
      'Send customer partial shipment notification and ETA.',
    ],
    split_tender_failure: [
      'Inspect payment intent settlement status.',
      'Cancel or refund the failing tender path.',
      'Trigger customer outreach for alternate payment resolution.',
    ],
    returns_arbitration: [
      'Collect dispute evidence and return request context.',
      'Create/route arbitration case according to policy.',
      'Execute approved compensation decision.',
    ],
  };

  return {
    exceptionType,
    orderId: params.orderId,
    shipmentId: params.shipmentId || null,
    steps: stepsByType[exceptionType] || ['Inspect exception and route to operations'],
  };
}

/**
 * Shipment tool definitions
 */
export const shipmentTools = [
  {
    name: 'list_shipments',
    description: 'List all shipments.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const shipments = await commerce.shipments.list();
      const count = await commerce.shipments.count();
      return { success: true, count, shipments };
    },
  },

  {
    name: 'create_shipment',
    description: 'Create a shipment for an order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
      carrier: z.string().optional().describe('Carrier: USPS, UPS, FedEx, DHL'),
      service: z.string().optional().describe('Service level'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipment', params);
      }

      const shipment = await commerce.shipments.create({
        orderId: params.orderId,
        carrier: params.carrier,
        service: params.service,
      });
      return { success: true, message: 'Shipment created', shipment };
    },
  },

  {
    name: 'deliver_shipment',
    description: 'Mark a shipment as delivered.',
    inputSchema: {
      shipmentId: z.string().min(1).describe('Shipment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { shipmentId } = params;
      if (!allowApply) {
        return applyRequired('Deliver shipment', params);
      }

      const shipment = await commerce.shipments.deliver(shipmentId);
      return { success: true, message: 'Shipment delivered', shipment };
    },
  },

  {
    name: 'list_shipping_providers',
    description: 'List shipping providers and capabilities for quoting, labeling, and tracking.',
    inputSchema: {
      capability: z
        .string()
        .optional()
        .describe('Optional capability filter (e.g., rate_quote, tracking, label_void)'),
      mode: z
        .enum(['sandbox', 'shadow', 'production'])
        .optional()
        .describe('Optional provider mode filter'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const providers = listShippingProviders({
        capability: params.capability,
        mode: params.mode,
      });
      return {
        success: true,
        count: providers.length,
        providers,
      };
    },
  },

  {
    name: 'quote_shipping_rates',
    description:
      'Quote carrier rates from provider adapters using structured parcel data and destination address.',
    inputSchema: {
      providerId: z.string().optional().describe('Shipping provider ID'),
      originAddress: addressSchema.optional().describe('Origin address'),
      destinationAddress: addressSchema.describe('Destination address'),
      parcels: z.array(parcelSchema).min(1).optional().describe('Parcel list'),
      items: z
        .array(itemSchema)
        .min(1)
        .optional()
        .describe('Alternative input: items to derive parcel weights'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      serviceCodes: z.array(z.string().min(1)).optional().describe('Optional service code filter'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const parcels = parcelsFromInputs(params.parcels, params.items);
      if (parcels.length === 0) {
        throw new Error('Provide either parcels or items to quote shipping rates');
      }

      const result = quoteShippingRates({
        providerId: params.providerId,
        originAddress: params.originAddress || {},
        destinationAddress: params.destinationAddress,
        parcels,
        currency: params.currency || 'USD',
        serviceCodes: params.serviceCodes,
      });

      return {
        success: true,
        provider: result.provider,
        count: result.rates.length,
        rates: result.rates,
      };
    },
  },

  {
    name: 'create_shipping_label',
    description: 'Create a carrier label from quoted rates or explicit service code.',
    inputSchema: {
      providerId: z.string().optional().describe('Shipping provider ID'),
      rateId: z.string().optional().describe('Quoted rate ID'),
      serviceCode: z.string().optional().describe('Service code when no rateId is provided'),
      orderId: z.string().optional().describe('Order ID'),
      shipmentId: z.string().optional().describe('Shipment ID'),
      originAddress: addressSchema.optional().describe('Origin address'),
      destinationAddress: addressSchema.optional().describe('Destination address'),
      parcels: z.array(parcelSchema).min(1).optional().describe('Parcel list'),
      items: z
        .array(itemSchema)
        .min(1)
        .optional()
        .describe('Alternative input: items to derive parcel weights'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      metadata: z.record(z.string(), z.any()).optional().describe('Additional metadata'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipping label', params);
      }

      const parcels = parcelsFromInputs(params.parcels, params.items);
      const result = createShippingLabel({
        providerId: params.providerId,
        rateId: params.rateId,
        serviceCode: params.serviceCode,
        orderId: params.orderId,
        shipmentId: params.shipmentId,
        originAddress: params.originAddress || {},
        destinationAddress: params.destinationAddress || {},
        parcels,
        currency: params.currency || 'USD',
        metadata: params.metadata || {},
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent
          ? 'Shipping label reused via idempotency key'
          : 'Shipping label created',
        provider: result.provider,
        label: result.label,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'void_shipping_label',
    description: 'Void a shipping label before final delivery.',
    inputSchema: {
      labelId: z.string().min(1).describe('Shipping label ID'),
      reason: z.string().max(500).optional().describe('Void reason'),
    },
    permission: 'delete',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Void shipping label', params);
      }

      const result = voidShippingLabel({
        labelId: params.labelId,
        reason: params.reason,
      });

      return {
        success: true,
        message: result.idempotent ? 'Shipping label already voided' : 'Shipping label voided',
        label: result.label,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'track_shipping_label',
    description: 'Track a shipping label by label ID or tracking number.',
    inputSchema: {
      labelId: z.string().optional().describe('Shipping label ID'),
      trackingNumber: z.string().optional().describe('Carrier tracking number'),
      advanceStatus: z
        .boolean()
        .optional()
        .describe('Advance simulated tracking status for deterministic replay testing'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      if (!params.labelId && !params.trackingNumber) {
        throw new Error('Provide either labelId or trackingNumber');
      }

      const tracking = trackShippingLabel({
        labelId: params.labelId,
        trackingNumber: params.trackingNumber,
        advanceStatus: Boolean(params.advanceStatus),
      });

      return {
        success: true,
        label: tracking.label,
        latestEvent: tracking.latestEvent,
      };
    },
  },

  {
    name: 'list_shipping_labels',
    description: 'List provider-backed shipping labels with optional filtering.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z.string().optional().describe('Filter by label status'),
      orderId: z.string().optional().describe('Filter by order ID'),
      shipmentId: z.string().optional().describe('Filter by shipment ID'),
      trackingNumber: z.string().optional().describe('Filter by tracking number'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum labels to return'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const labels = listShippingLabels({
        providerId: params.providerId,
        status: params.status,
        orderId: params.orderId,
        shipmentId: params.shipmentId,
        trackingNumber: params.trackingNumber,
        limit: params.limit,
      });

      return {
        success: true,
        count: labels.length,
        labels,
      };
    },
  },

  {
    name: 'ingest_shipping_provider_webhook',
    description:
      'Ingest a shipping provider webhook event and reconcile label/tracking state for shadow mode operations.',
    inputSchema: {
      providerId: z.string().optional().describe('Provider ID (default: deterministic-mock)'),
      eventType: z.string().min(1).describe('Webhook event type'),
      eventId: z
        .string()
        .optional()
        .describe('Optional provider event ID for idempotent ingestion'),
      payload: z
        .record(z.string(), z.any())
        .optional()
        .describe('Webhook payload object from provider'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Ingest shipping provider webhook', params);
      }

      const result = ingestShippingProviderWebhook({
        providerId: params.providerId,
        eventType: params.eventType,
        eventId: params.eventId,
        payload: params.payload || {},
      });

      return {
        success: true,
        message: result.applied
          ? 'Shipping webhook ingested'
          : 'Shipping webhook processed with no mutation',
        webhook: result,
      };
    },
  },

  {
    name: 'handle_fulfillment_exception',
    description:
      'Execute governed fulfillment exception workflows for carrier failure, partial shipment, split tender, and returns arbitration.',
    inputSchema: {
      exceptionType: z
        .enum([
          'carrier_failure',
          'partial_shipment',
          'split_tender_failure',
          'returns_arbitration',
        ])
        .describe('Exception workflow type'),
      orderId: z.string().min(1).describe('Order ID'),
      shipmentId: z.string().optional().describe('Shipment ID'),
      labelId: z.string().optional().describe('Shipping label ID'),
      paymentIntentId: z.string().optional().describe('Payment intent ID'),
      providerId: z.string().optional().describe('Preferred provider ID for compensation actions'),
      autoExecuteCompensation: z
        .boolean()
        .optional()
        .describe('When true, execute compensation actions instead of planning only'),
      details: z.record(z.string(), z.any()).optional().describe('Workflow-specific metadata'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const workflowPlan = buildExceptionPlan(params);

      if (!allowApply) {
        return applyRequired('Handle fulfillment exception', {
          ...params,
          workflowPlan,
        });
      }

      const details = params.details || {};
      const execution = [];
      const artifacts = {};
      const autoExecute = Boolean(params.autoExecuteCompensation);

      if (params.exceptionType === 'carrier_failure') {
        if (params.labelId) {
          const tracking = trackShippingLabel({ labelId: params.labelId });
          artifacts.tracking = tracking.label;
          execution.push({
            action: 'inspect_tracking',
            status: 'completed',
            labelId: params.labelId,
            latestStatus: tracking.label.status,
          });
        }

        if (autoExecute) {
          const replacementParcels = parcelsFromInputs(details.parcels, details.items);
          if (replacementParcels.length > 0 && (details.destinationAddress || details.rateId)) {
            const replacement = createShippingLabel({
              providerId: params.providerId || details.providerId,
              rateId: details.rateId,
              serviceCode: details.serviceCode,
              orderId: params.orderId,
              shipmentId: params.shipmentId,
              originAddress: details.originAddress || {},
              destinationAddress: details.destinationAddress || {},
              parcels: replacementParcels,
              currency: details.currency || 'USD',
              metadata: {
                ...details.metadata,
                exceptionType: params.exceptionType,
              },
              idempotencyKey: details.idempotencyKey,
            });
            artifacts.replacementLabel = replacement.label;
            execution.push({
              action: 'create_replacement_label',
              status: 'completed',
              labelId: replacement.label.id,
            });
          } else {
            execution.push({
              action: 'create_replacement_label',
              status: 'skipped',
              reason:
                'Missing replacement shipment payload (destinationAddress + parcels or rateId)',
            });
          }
        }
      }

      if (params.exceptionType === 'partial_shipment') {
        if (autoExecute && commerce?.shipments?.create) {
          const followUpShipment = await commerce.shipments.create({
            orderId: params.orderId,
            carrier: details.carrier,
            service: details.service,
            parentShipmentId: params.shipmentId,
            items: details.remainingItems || [],
            reason: 'partial_shipment_compensation',
          });
          artifacts.followUpShipment = followUpShipment;
          execution.push({
            action: 'create_follow_up_shipment',
            status: 'completed',
            shipmentId: followUpShipment.id || null,
          });
        } else {
          execution.push({
            action: 'create_follow_up_shipment',
            status: 'skipped',
            reason: 'autoExecuteCompensation disabled or shipments.create unavailable',
          });
        }
      }

      if (params.exceptionType === 'split_tender_failure') {
        if (autoExecute && params.paymentIntentId) {
          if (details.refundAmount !== null && details.refundAmount !== undefined) {
            const paymentCompensation = refundPaymentIntent({
              intentId: params.paymentIntentId,
              amount: details.refundAmount,
              reason: details.reason || 'split_tender_failure',
              idempotencyKey: details.idempotencyKey,
            });
            artifacts.paymentCompensation = paymentCompensation;
            execution.push({
              action: 'refund_payment_intent',
              status: 'completed',
              intentId: params.paymentIntentId,
            });
          } else {
            const paymentCompensation = cancelPaymentIntent({
              intentId: params.paymentIntentId,
              reason: details.reason || 'split_tender_failure',
            });
            artifacts.paymentCompensation = paymentCompensation;
            execution.push({
              action: 'cancel_payment_intent',
              status: 'completed',
              intentId: params.paymentIntentId,
            });
          }
        } else {
          execution.push({
            action: 'resolve_split_tender_failure',
            status: 'skipped',
            reason: 'autoExecuteCompensation disabled or paymentIntentId missing',
          });
        }
      }

      if (params.exceptionType === 'returns_arbitration') {
        if (autoExecute && commerce?.returns?.create && details.returnRequest) {
          const returnCase = await commerce.returns.create({
            ...details.returnRequest,
            orderId: params.orderId,
            arbitrationReason: details.reason || 'returns_arbitration',
          });
          artifacts.returnCase = returnCase;
          execution.push({
            action: 'create_return_arbitration_case',
            status: 'completed',
            returnId: returnCase.id || null,
          });
        } else {
          execution.push({
            action: 'create_return_arbitration_case',
            status: 'skipped',
            reason:
              'autoExecuteCompensation disabled, returns.create unavailable, or returnRequest missing',
          });
        }
      }

      const caseId = deterministicId('fx', {
        orderId: params.orderId,
        exceptionType: params.exceptionType,
        shipmentId: params.shipmentId || null,
        labelId: params.labelId || null,
        paymentIntentId: params.paymentIntentId || null,
      });

      return {
        success: true,
        message: 'Fulfillment exception workflow executed',
        caseId,
        workflowPlan,
        autoExecuteCompensation: autoExecute,
        execution,
        artifacts,
      };
    },
  },
];

export default shipmentTools;
