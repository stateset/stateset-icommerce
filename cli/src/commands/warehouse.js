/**
 * Warehouse Commands Module
 */

function parseIntArg(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const warehouses = await commerce.warehouse.listWarehouses();
      return formatWarehouses(warehouses, { output, jsonOutput });
    }

    case 'get': {
      const [identifier] = args;
      if (!identifier) throw new Error('Usage: warehouse get <warehouseId|code>');
      const warehouse = /^\d+$/.test(identifier)
        ? await commerce.warehouse.getWarehouse(
            parseIntArg(identifier, 'Usage: warehouse get <warehouseId|code>'),
          )
        : await commerce.warehouse.getWarehouseByCode(identifier);
      if (!warehouse) throw new Error(`Warehouse not found: ${identifier}`);
      return formatWarehouse(warehouse, { jsonOutput });
    }

    case 'create': {
      const [code, name, warehouseType, timezone] = args;
      if (!code || !name) {
        throw new Error('Usage: warehouse create <code> <name> [warehouseType] [timezone]');
      }
      const warehouse = await commerce.warehouse.createWarehouse({
        code,
        name,
        warehouseType: warehouseType || undefined,
        timezone: timezone || undefined,
      });
      return {
        warehouse,
        formatted: `Created warehouse ${warehouse.code || warehouse.id}`,
      };
    }

    case 'locations': {
      const [warehouseIdRaw] = args;
      const locations = await commerce.warehouse.listLocations(
        warehouseIdRaw
          ? parseIntArg(warehouseIdRaw, 'Usage: warehouse locations [warehouseId]')
          : undefined,
      );
      return formatLocations(locations, { output, jsonOutput });
    }

    case 'location': {
      const [locationIdRaw] = args;
      if (!locationIdRaw) throw new Error('Usage: warehouse location <locationId>');
      const location = await commerce.warehouse.getLocation(
        parseIntArg(locationIdRaw, 'Usage: warehouse location <locationId>'),
      );
      if (!location) throw new Error(`Location not found: ${locationIdRaw}`);
      return formatLocation(location, { jsonOutput });
    }

    case 'create-location': {
      const [warehouseIdRaw, locationType, zone, aisle, rack, bin, isPickableRaw, isReceivableRaw] =
        args;
      if (!warehouseIdRaw || !locationType) {
        throw new Error(
          'Usage: warehouse create-location <warehouseId> <locationType> [zone] [aisle] [rack] [bin] [isPickable] [isReceivable]',
        );
      }
      const location = await commerce.warehouse.createLocation({
        warehouseId: parseIntArg(
          warehouseIdRaw,
          'Usage: warehouse create-location <warehouseId> <locationType> [zone] [aisle] [rack] [bin] [isPickable] [isReceivable]',
        ),
        locationType,
        zone: zone || undefined,
        aisle: aisle || undefined,
        rack: rack || undefined,
        bin: bin || undefined,
        isPickable:
          isPickableRaw === undefined
            ? undefined
            : ['true', '1', 'yes'].includes(String(isPickableRaw).toLowerCase()),
        isReceivable:
          isReceivableRaw === undefined
            ? undefined
            : ['true', '1', 'yes'].includes(String(isReceivableRaw).toLowerCase()),
      });
      return {
        location,
        formatted: `Created warehouse location ${location.id}`,
      };
    }

    case 'pickable': {
      const [warehouseIdRaw, sku] = args;
      if (!warehouseIdRaw || !sku) throw new Error('Usage: warehouse pickable <warehouseId> <sku>');
      const locations = await commerce.warehouse.getPickableLocations(
        parseIntArg(warehouseIdRaw, 'Usage: warehouse pickable <warehouseId> <sku>'),
        sku,
      );
      return formatLocations(locations, { output, jsonOutput });
    }

    case 'available': {
      const [warehouseIdRaw, sku] = args;
      if (!warehouseIdRaw || !sku)
        throw new Error('Usage: warehouse available <warehouseId> <sku>');
      const quantity = await commerce.warehouse.getTotalAvailable(
        parseIntArg(warehouseIdRaw, 'Usage: warehouse available <warehouseId> <sku>'),
        sku,
      );
      return jsonOutput
        ? {
            warehouseId: parseIntArg(
              warehouseIdRaw,
              'Usage: warehouse available <warehouseId> <sku>',
            ),
            sku,
            quantity,
          }
        : { formatted: `Warehouse ${warehouseIdRaw} available quantity for ${sku}: ${quantity}` };
    }

    case 'count': {
      const count = await commerce.warehouse.countWarehouses();
      return { count, formatted: `Warehouse count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: warehouse ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                 List warehouses\n' +
          '  get <warehouseId|code>                                               Get warehouse\n' +
          '  create <code> <name> [warehouseType] [timezone]                      Create warehouse\n' +
          '  locations [warehouseId]                                              List locations\n' +
          '  location <locationId>                                                Get location\n' +
          '  create-location <warehouseId> <locationType> [zone] [aisle] [rack] [bin] [isPickable] [isReceivable]\n' +
          '  pickable <warehouseId> <sku>                                         List pickable locations\n' +
          '  available <warehouseId> <sku>                                        Get available quantity\n' +
          '  count                                                                Count warehouses',
      );
  }
}

function formatWarehouses(warehouses, { output, jsonOutput }) {
  if (jsonOutput) return warehouses;
  if (warehouses.length === 0) return { formatted: 'No warehouses found.' };
  const formatted = output.table(warehouses, [
    { key: 'id', header: 'ID' },
    { key: 'code', header: 'Code' },
    { key: 'name', header: 'Name' },
    { key: 'warehouseType', header: 'Type' },
    { key: 'timezone', header: 'Timezone' },
  ]);
  return { warehouses, formatted };
}

function formatWarehouse(warehouse, { jsonOutput }) {
  if (jsonOutput) return warehouse;
  return {
    warehouse,
    formatted:
      `Warehouse: ${warehouse.name}\n` +
      `${'-'.repeat(36)}\n` +
      `ID:          ${warehouse.id}\n` +
      `Code:        ${warehouse.code}\n` +
      `Type:        ${warehouse.warehouseType || 'N/A'}\n` +
      `Timezone:    ${warehouse.timezone || 'N/A'}`,
  };
}

function formatLocations(locations, { output, jsonOutput }) {
  if (jsonOutput) return locations;
  if (locations.length === 0) return { formatted: 'No warehouse locations found.' };
  const formatted = output.table(locations, [
    { key: 'id', header: 'ID' },
    { key: 'warehouseId', header: 'Warehouse', align: 'right' },
    { key: 'locationType', header: 'Type' },
    { key: 'zone', header: 'Zone' },
    { key: 'aisle', header: 'Aisle' },
    { key: 'bin', header: 'Bin' },
  ]);
  return { locations, formatted };
}

function formatLocation(location, { jsonOutput }) {
  if (jsonOutput) return location;
  return {
    location,
    formatted:
      `Location: ${location.id}\n` +
      `${'-'.repeat(34)}\n` +
      `Warehouse:    ${location.warehouseId}\n` +
      `Type:         ${location.locationType}\n` +
      `Zone:         ${location.zone || 'N/A'}\n` +
      `Aisle:        ${location.aisle || 'N/A'}\n` +
      `Rack:         ${location.rack || 'N/A'}\n` +
      `Bin:          ${location.bin || 'N/A'}`,
  };
}

export const metadata = {
  name: 'warehouse',
  aliases: ['wh', 'warehouses'],
  description: 'Warehouse and storage location commands',
  actions: {
    list: { description: 'List warehouses', args: [] },
    get: { description: 'Get warehouse', args: ['<warehouseId|code>'] },
    create: {
      description: 'Create warehouse',
      args: ['<code>', '<name>', '[warehouseType]', '[timezone]'],
    },
    locations: { description: 'List locations', args: ['[warehouseId]'] },
    location: { description: 'Get location', args: ['<locationId>'] },
    'create-location': {
      description: 'Create location',
      args: [
        '<warehouseId>',
        '<locationType>',
        '[zone]',
        '[aisle]',
        '[rack]',
        '[bin]',
        '[isPickable]',
        '[isReceivable]',
      ],
    },
    pickable: { description: 'List pickable locations', args: ['<warehouseId>', '<sku>'] },
    available: { description: 'Get available quantity', args: ['<warehouseId>', '<sku>'] },
    count: { description: 'Count warehouses', args: [] },
  },
};

export default { execute, metadata };
