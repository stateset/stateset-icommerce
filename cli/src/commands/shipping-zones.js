/**
 * Shipping Zones Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'zones': {
      const limit = args[0] ? Number.parseInt(args[0], 10) : undefined;
      const zones = await commerce.shippingZones.list();
      const rows = Number.isInteger(limit) && limit > 0 ? zones.slice(0, limit) : zones;
      return formatZoneList(rows, { output, jsonOutput });
    }

    case 'zone': {
      const zoneId = args[0];
      if (!zoneId) throw new Error('Usage: shipping-zones zone <zoneId>');
      const zone = await commerce.shippingZones.get(zoneId);
      if (!zone) throw new Error(`Shipping zone not found: ${zoneId}`);
      return formatZoneDetail(zone, { jsonOutput });
    }

    case 'create-zone': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: shipping-zones create-zone <payloadJson>');
      const zone = await commerce.shippingZones.create(parseJsonArg(payloadJson, 'payload'));
      return { zone, formatted: `Created shipping zone ${zone.name || zone.id}` };
    }

    case 'update-zone': {
      const [zoneId, payloadJson] = args;
      if (!zoneId || !payloadJson) {
        throw new Error('Usage: shipping-zones update-zone <zoneId> <payloadJson>');
      }
      const zone = await commerce.shippingZones.update(
        zoneId,
        parseJsonArg(payloadJson, 'payload'),
      );
      return { zone, formatted: `Updated shipping zone ${zone.name || zone.id}` };
    }

    case 'create-method': {
      const [zoneId, payloadJson] = args;
      if (!zoneId || !payloadJson) {
        throw new Error('Usage: shipping-zones create-method <zoneId> <payloadJson>');
      }
      const method = await commerce.shippingZones.createMethod(
        zoneId,
        parseJsonArg(payloadJson, 'payload'),
      );
      return { method, formatted: `Created shipping method ${method.name || method.id}` };
    }

    case 'methods': {
      const zoneId = args[0];
      if (!zoneId) throw new Error('Usage: shipping-zones methods <zoneId>');
      const methods = await commerce.shippingZones.listMethods(zoneId);
      return formatMethodList(methods, { output, jsonOutput });
    }

    case 'rates': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: shipping-zones rates <payloadJson>');
      const rates = await commerce.shippingZones.calculateRates(
        parseJsonArg(payloadJson, 'payload'),
      );
      return formatRates(rates, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: shipping-zones ${action}\n\n` +
          'Available actions:\n' +
          '  zones [limit]                          List shipping zones\n' +
          '  zone <zoneId>                          Get shipping zone\n' +
          '  create-zone <payloadJson>              Create shipping zone\n' +
          '  update-zone <zoneId> <payloadJson>     Update shipping zone\n' +
          '  create-method <zoneId> <payloadJson>   Create shipping method\n' +
          '  methods <zoneId>                       List shipping methods\n' +
          '  rates <payloadJson>                    Calculate shipping rates',
      );
  }
}

function formatZoneList(zones, { output, jsonOutput }) {
  if (jsonOutput) return zones;
  if (zones.length === 0) return { formatted: 'No shipping zones found.' };
  const formatted = output.table(zones, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'countries', header: 'Countries' },
    { key: 'status', header: 'Status' },
  ]);
  return { zones, formatted };
}

function formatZoneDetail(zone, { jsonOutput }) {
  if (jsonOutput) return zone;
  return {
    zone,
    formatted:
      `Shipping zone: ${zone.name}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:           ${zone.id}\n` +
      `Countries:    ${Array.isArray(zone.countries) ? zone.countries.join(', ') : 'N/A'}\n` +
      `Regions:      ${Array.isArray(zone.regions) ? zone.regions.join(', ') : 'N/A'}\n` +
      `Methods:      ${zone.methodCount ?? (Array.isArray(zone.methods) ? zone.methods.length : 0)}\n` +
      `Status:       ${zone.status || 'N/A'}`,
  };
}

function formatMethodList(methods, { output, jsonOutput }) {
  if (jsonOutput) return methods;
  if (methods.length === 0) return { formatted: 'No shipping methods found.' };
  const formatted = output.table(methods, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'carrier', header: 'Carrier' },
    { key: 'baseRate', header: 'Base Rate', align: 'right' },
    { key: 'currency', header: 'Currency' },
  ]);
  return { methods, formatted };
}

function formatRates(rates, { output, jsonOutput }) {
  if (jsonOutput) return rates;
  if (rates.length === 0) return { formatted: 'No shipping rates found.' };
  const formatted = output.table(
    rates.map((rate) => ({
      methodName: rate.methodName,
      carrier: rate.carrier,
      rate: rate.rate,
      currency: rate.currency,
      eta: `${rate.minDeliveryDays ?? '?'}-${rate.maxDeliveryDays ?? '?'}d`,
      free: rate.isFreeShipping ? 'yes' : 'no',
    })),
    [
      { key: 'methodName', header: 'Method' },
      { key: 'carrier', header: 'Carrier' },
      { key: 'rate', header: 'Rate', align: 'right' },
      { key: 'currency', header: 'Currency' },
      { key: 'eta', header: 'ETA' },
      { key: 'free', header: 'Free' },
    ],
  );
  return { rates, formatted };
}

export const metadata = {
  name: 'shipping-zones',
  aliases: ['zones', 'shipzones'],
  description: 'Shipping zone and method commands',
  actions: {
    zones: { description: 'List shipping zones', args: ['[limit]'] },
    zone: { description: 'Get shipping zone', args: ['<zoneId>'] },
    'create-zone': { description: 'Create shipping zone', args: ['<payloadJson>'] },
    'update-zone': { description: 'Update shipping zone', args: ['<zoneId>', '<payloadJson>'] },
    'create-method': { description: 'Create shipping method', args: ['<zoneId>', '<payloadJson>'] },
    methods: { description: 'List shipping methods', args: ['<zoneId>'] },
    rates: { description: 'Calculate shipping rates', args: ['<payloadJson>'] },
  },
};

export default { execute, metadata };
