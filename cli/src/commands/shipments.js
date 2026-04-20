/**
 * Shipments Commands Module
 */

import {
  createShippingLabel,
  ingestShippingProviderWebhook,
  listShippingLabels,
  listShippingProviders,
  quoteShippingRates,
  trackShippingLabel,
  voidShippingLabel,
} from '../tools/providers/shipping.js';

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseLimit(value, fallback = 100) {
  const limit = Number.parseInt(value || String(fallback), 10);
  return Number.isInteger(limit) && limit > 0 ? limit : fallback;
}

function parseBoolean(value, fallback = false) {
  if (value === undefined) return fallback;
  return ['true', '1', 'yes', 'y'].includes(String(value).toLowerCase());
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [orderId, status] = args;
      const shipments = await commerce.shipments.list();
      const filtered = shipments.filter(
        (shipment) =>
          (!orderId || shipment.orderId === orderId) && (!status || shipment.status === status),
      );
      return formatShipmentList(filtered, { output, jsonOutput });
    }

    case 'get': {
      const shipmentId = args[0];
      if (!shipmentId) throw new Error('Usage: shipments get <shipmentId>');
      const shipment = await commerce.shipments.get(shipmentId);
      if (!shipment) throw new Error(`Shipment not found: ${shipmentId}`);
      return formatShipmentDetail(shipment, { jsonOutput });
    }

    case 'create': {
      const [orderId, carrier, service] = args;
      if (!orderId) throw new Error('Usage: shipments create <orderId> [carrier] [service]');
      const shipment = await commerce.shipments.create({ orderId, carrier, service });
      return {
        shipment,
        formatted: `Created shipment ${shipment.id} for order ${shipment.orderId}`,
      };
    }

    case 'ship': {
      const [shipmentId, trackingNumber] = args;
      if (!shipmentId) throw new Error('Usage: shipments ship <shipmentId> [trackingNumber]');
      const shipment = await commerce.shipments.ship(shipmentId, trackingNumber);
      return {
        shipment,
        formatted: `Marked shipment ${shipment.id} as shipped`,
      };
    }

    case 'deliver': {
      const shipmentId = args[0];
      if (!shipmentId) throw new Error('Usage: shipments deliver <shipmentId>');
      const shipment = await commerce.shipments.deliver(shipmentId);
      return {
        shipment,
        formatted: `Delivered shipment ${shipment.id}`,
      };
    }

    case 'cancel': {
      const shipmentId = args[0];
      if (!shipmentId) throw new Error('Usage: shipments cancel <shipmentId>');
      const shipment = await commerce.shipments.cancel(shipmentId);
      return {
        shipment,
        formatted: `Cancelled shipment ${shipment.id}`,
      };
    }

    case 'providers': {
      const [capability, mode] = args;
      const providers = listShippingProviders({ capability, mode });
      return formatProviders(providers, { output, jsonOutput });
    }

    case 'rates': {
      const [
        destinationJson,
        parcelsJson,
        providerId,
        currency = 'USD',
        serviceCodesCsv,
        originJson,
      ] = args;
      if (!destinationJson || !parcelsJson) {
        throw new Error(
          'Usage: shipments rates <destinationJson> <parcelsJson> [providerId] [currency] [serviceCodesCsv] [originJson]',
        );
      }
      const result = quoteShippingRates({
        providerId,
        destinationAddress: parseJsonArg(destinationJson, 'destination'),
        parcels: parseJsonArg(parcelsJson, 'parcels'),
        currency: currency.toUpperCase(),
        serviceCodes: serviceCodesCsv
          ? serviceCodesCsv
              .split(',')
              .map((code) => code.trim())
              .filter(Boolean)
          : undefined,
        originAddress: originJson ? parseJsonArg(originJson, 'origin') : {},
      });
      return formatRates(result, { output, jsonOutput });
    }

    case 'label': {
      const [
        destinationJson,
        parcelsJson,
        serviceCode,
        providerId,
        currency = 'USD',
        orderId,
        shipmentId,
        originJson,
      ] = args;
      if (!destinationJson || !parcelsJson) {
        throw new Error(
          'Usage: shipments label <destinationJson> <parcelsJson> [serviceCode] [providerId] [currency] [orderId] [shipmentId] [originJson]',
        );
      }
      const result = createShippingLabel({
        providerId,
        serviceCode,
        currency: currency.toUpperCase(),
        orderId,
        shipmentId,
        destinationAddress: parseJsonArg(destinationJson, 'destination'),
        parcels: parseJsonArg(parcelsJson, 'parcels'),
        originAddress: originJson ? parseJsonArg(originJson, 'origin') : {},
      });
      return formatLabelResult(result, { jsonOutput });
    }

    case 'labels': {
      const [providerId, status, orderId, limitRaw] = args;
      const labels = listShippingLabels({
        providerId,
        status,
        orderId,
        limit: parseLimit(limitRaw),
      });
      return formatLabelList(labels, { output, jsonOutput });
    }

    case 'track': {
      const [identifier, advanceRaw] = args;
      if (!identifier) throw new Error('Usage: shipments track <labelId|trackingNumber> [advance]');
      const result = trackShippingLabel({
        labelId: identifier.startsWith('lbl_') ? identifier : undefined,
        trackingNumber: identifier.startsWith('SS') ? identifier : undefined,
        advanceStatus: parseBoolean(advanceRaw, false),
      });
      return formatTracking(result, { jsonOutput });
    }

    case 'void-label': {
      const [labelId, ...reasonParts] = args;
      if (!labelId) throw new Error('Usage: shipments void-label <labelId> [reason]');
      const result = voidShippingLabel({
        labelId,
        reason: reasonParts.join(' ') || undefined,
      });
      return formatVoidResult(result, { jsonOutput });
    }

    case 'webhook': {
      const [eventType, identifier, providerId] = args;
      if (!eventType || !identifier) {
        throw new Error(
          'Usage: shipments webhook <eventType> <labelId|trackingNumber> [providerId]',
        );
      }
      const payload = identifier.startsWith('SS')
        ? { trackingNumber: identifier }
        : { labelId: identifier };
      const result = ingestShippingProviderWebhook({
        providerId,
        eventType,
        payload,
      });
      return formatWebhookResult(result, { jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: shipments ${action}\n\n` +
          'Available actions:\n' +
          '  list [orderId] [status]                        List shipments\n' +
          '  get <shipmentId>                               Get shipment details\n' +
          '  create <orderId> [carrier] [service]           Create shipment\n' +
          '  ship <shipmentId> [trackingNumber]             Mark shipment shipped\n' +
          '  deliver <shipmentId>                           Mark shipment delivered\n' +
          '  cancel <shipmentId>                            Cancel shipment\n' +
          '  providers [capability] [mode]                  List shipping providers\n' +
          '  rates <destinationJson> <parcelsJson> [providerId] [currency] [serviceCodesCsv] [originJson]\n' +
          '  label <destinationJson> <parcelsJson> [serviceCode] [providerId] [currency] [orderId] [shipmentId] [originJson]\n' +
          '  labels [providerId] [status] [orderId] [limit] List shipping labels\n' +
          '  track <labelId|trackingNumber> [advance]       Track shipping label\n' +
          '  void-label <labelId> [reason]                  Void shipping label\n' +
          '  webhook <eventType> <labelId|trackingNumber> [providerId]  Ingest shipping webhook',
      );
  }
}

function formatShipmentList(shipments, { output, jsonOutput }) {
  if (jsonOutput) return shipments;
  if (shipments.length === 0) return { formatted: 'No shipments found.' };
  const formatted = output.table(shipments, [
    { key: 'id', header: 'ID' },
    { key: 'orderId', header: 'Order' },
    { key: 'status', header: 'Status' },
    { key: 'carrier', header: 'Carrier' },
    { key: 'service', header: 'Service' },
    { key: 'trackingNumber', header: 'Tracking' },
  ]);
  return { shipments, formatted };
}

function formatShipmentDetail(shipment, { jsonOutput }) {
  if (jsonOutput) return shipment;
  return {
    shipment,
    formatted:
      `Shipment: ${shipment.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Order:       ${shipment.orderId}\n` +
      `Status:      ${shipment.status}\n` +
      `Carrier:     ${shipment.carrier || 'N/A'}\n` +
      `Service:     ${shipment.service || 'N/A'}\n` +
      `Tracking:    ${shipment.trackingNumber || 'N/A'}\n` +
      `Created:     ${shipment.createdAt || 'N/A'}`,
  };
}

function formatProviders(providers, { output, jsonOutput }) {
  if (jsonOutput) return providers;
  if (providers.length === 0) return { formatted: 'No shipping providers found.' };
  const formatted = output.table(
    providers.map((provider) => ({
      id: provider.id,
      mode: provider.mode,
      status: provider.status,
      services: (provider.services || []).map((service) => service.code).join(','),
    })),
    [
      { key: 'id', header: 'Provider' },
      { key: 'mode', header: 'Mode' },
      { key: 'status', header: 'Status' },
      { key: 'services', header: 'Services' },
    ],
  );
  return { providers, formatted };
}

function formatRates(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  if (result.rates.length === 0) return { formatted: 'No shipping rates found.' };
  const formatted = output.table(result.rates, [
    { key: 'rateId', header: 'Rate' },
    { key: 'serviceCode', header: 'Service' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'minDeliveryDays', header: 'Min Days', align: 'right' },
    { key: 'maxDeliveryDays', header: 'Max Days', align: 'right' },
  ]);
  return { result, formatted };
}

function formatLabelResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted:
      `Shipping label ${result.label.id}\n` +
      `${'-'.repeat(30)}\n` +
      `Provider:    ${result.provider.id}\n` +
      `Tracking:    ${result.label.trackingNumber}\n` +
      `Status:      ${result.label.status}\n` +
      `Idempotent:  ${result.idempotent ? 'yes' : 'no'}`,
  };
}

function formatLabelList(labels, { output, jsonOutput }) {
  if (jsonOutput) return labels;
  if (labels.length === 0) return { formatted: 'No shipping labels found.' };
  const formatted = output.table(labels, [
    { key: 'id', header: 'Label' },
    { key: 'providerId', header: 'Provider' },
    { key: 'status', header: 'Status' },
    { key: 'trackingNumber', header: 'Tracking' },
    { key: 'serviceCode', header: 'Service' },
    { key: 'amount', header: 'Amount', align: 'right' },
  ]);
  return { labels, formatted };
}

function formatTracking(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Tracking ${result.label.trackingNumber}\n` +
      `${'-'.repeat(28)}\n` +
      `Label:       ${result.label.id}\n` +
      `Status:      ${result.label.status}\n` +
      `Latest:      ${result.latestEvent?.description || 'N/A'}\n` +
      `Updated:     ${result.latestEvent?.timestamp || result.label.updatedAt}`,
  };
}

function formatVoidResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted: `Voided shipping label ${result.label.id}${result.idempotent ? ' (idempotent)' : ''}`,
  };
}

function formatWebhookResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Shipping webhook\n` +
      `${'-'.repeat(24)}\n` +
      `Provider:    ${result.provider.id}\n` +
      `Event type:  ${result.eventType}\n` +
      `Action:      ${result.action}\n` +
      `Applied:     ${result.applied ? 'yes' : 'no'}\n` +
      `Label:       ${result.label?.id || 'N/A'}`,
  };
}

export const metadata = {
  name: 'shipments',
  aliases: ['ship', 'ships', 'shp'],
  description: 'Shipments, labels, tracking, and carrier webhooks',
  actions: {
    list: { description: 'List shipments', args: ['[orderId]', '[status]'] },
    get: { description: 'Get shipment', args: ['<shipmentId>'] },
    create: { description: 'Create shipment', args: ['<orderId>', '[carrier]', '[service]'] },
    ship: { description: 'Mark shipment shipped', args: ['<shipmentId>', '[trackingNumber]'] },
    deliver: { description: 'Mark shipment delivered', args: ['<shipmentId>'] },
    cancel: { description: 'Cancel shipment', args: ['<shipmentId>'] },
    providers: { description: 'List shipping providers', args: ['[capability]', '[mode]'] },
    rates: {
      description: 'Quote shipping rates',
      args: [
        '<destinationJson>',
        '<parcelsJson>',
        '[providerId]',
        '[currency]',
        '[serviceCodesCsv]',
        '[originJson]',
      ],
    },
    label: {
      description: 'Create shipping label',
      args: [
        '<destinationJson>',
        '<parcelsJson>',
        '[serviceCode]',
        '[providerId]',
        '[currency]',
        '[orderId]',
        '[shipmentId]',
        '[originJson]',
      ],
    },
    labels: {
      description: 'List shipping labels',
      args: ['[providerId]', '[status]', '[orderId]', '[limit]'],
    },
    track: { description: 'Track shipping label', args: ['<labelId|trackingNumber>', '[advance]'] },
    'void-label': { description: 'Void shipping label', args: ['<labelId>', '[reason]'] },
    webhook: {
      description: 'Ingest shipping webhook',
      args: ['<eventType>', '<labelId|trackingNumber>', '[providerId]'],
    },
  },
};

export default { execute, metadata };
