/**
 * Serials Commands Module
 */

function parseIntArg(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) throw new Error(usage);
  return parsed;
}

async function resolveSerial(serialsApi, identifier) {
  return serialsApi.get(identifier) || serialsApi.getBySerial(identifier);
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const serials = await commerce.serials.list();
      return formatSerials(serials, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: serials get <serialId|serial>');
      const serial = await resolveSerial(commerce.serials, identifier);
      if (!serial) throw new Error(`Serial not found: ${identifier}`);
      return formatSerial(serial, { jsonOutput });
    }

    case 'create': {
      const [sku, serialValue, lotNumber, manufacturedAt] = args;
      if (!sku)
        throw new Error('Usage: serials create <sku> [serial] [lotNumber] [manufacturedAt]');
      const serial = await commerce.serials.create({
        sku,
        serial: serialValue || undefined,
        lotNumber: lotNumber || undefined,
        manufacturedAt: manufacturedAt || undefined,
      });
      return { serial, formatted: `Created serial ${serial.serial || serial.id}` };
    }

    case 'available': {
      const [sku, limitRaw] = args;
      if (!sku) throw new Error('Usage: serials available <sku> [limit]');
      const serials = await commerce.serials.getAvailable(
        sku,
        limitRaw ? parseIntArg(limitRaw, 'Usage: serials available <sku> [limit]') : 50,
      );
      return formatSerials(serials, { output, jsonOutput });
    }

    case 'sold': {
      const [serialId, customerId, orderId] = args;
      if (!serialId || !customerId) {
        throw new Error('Usage: serials sold <serialId> <customerId> [orderId]');
      }
      const serial = await commerce.serials.markSold(serialId, customerId, orderId || undefined);
      return { serial, formatted: `Marked serial ${serial.serial || serial.id} as sold` };
    }

    case 'quarantine': {
      const [serialId, ...reasonParts] = args;
      if (!serialId || reasonParts.length === 0) {
        throw new Error('Usage: serials quarantine <serialId> <reason>');
      }
      const serial = await commerce.serials.quarantine(serialId, reasonParts.join(' '));
      return { serial, formatted: `Quarantined serial ${serial.serial || serial.id}` };
    }

    case 'check': {
      const serialValue = args[0];
      if (!serialValue) throw new Error('Usage: serials check <serial>');
      const available = await commerce.serials.isAvailable(serialValue);
      return jsonOutput
        ? { serial: serialValue, available }
        : { formatted: `Serial ${serialValue} available: ${available ? 'yes' : 'no'}` };
    }

    case 'count': {
      const count = await commerce.serials.count();
      return { count, formatted: `Serial count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: serials ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                   List serials\n' +
          '  get <serialId|serial>                                                  Get serial\n' +
          '  create <sku> [serial] [lotNumber] [manufacturedAt]                     Create serial\n' +
          '  available <sku> [limit]                                                List available serials\n' +
          '  sold <serialId> <customerId> [orderId]                                 Mark serial sold\n' +
          '  quarantine <serialId> <reason>                                         Quarantine serial\n' +
          '  check <serial>                                                         Check serial availability\n' +
          '  count                                                                  Count serials',
      );
  }
}

function formatSerials(serials, { output, jsonOutput }) {
  if (jsonOutput) return serials;
  if (serials.length === 0) return { formatted: 'No serials found.' };
  const formatted = output.table(serials, [
    { key: 'id', header: 'ID' },
    { key: 'serial', header: 'Serial' },
    { key: 'sku', header: 'SKU' },
    { key: 'status', header: 'Status' },
    { key: 'lotNumber', header: 'Lot' },
    { key: 'customerId', header: 'Customer' },
  ]);
  return { serials, formatted };
}

function formatSerial(serial, { jsonOutput }) {
  if (jsonOutput) return serial;
  return {
    serial,
    formatted:
      `Serial: ${serial.serial || serial.id}\n` +
      `${'-'.repeat(32)}\n` +
      `SKU:             ${serial.sku}\n` +
      `Status:          ${serial.status}\n` +
      `Lot:             ${serial.lotNumber || 'N/A'}\n` +
      `Customer:        ${serial.customerId || 'N/A'}\n` +
      `Manufactured:    ${serial.manufacturedAt || 'N/A'}`,
  };
}

export const metadata = {
  name: 'serials',
  aliases: ['serial', 'sn'],
  description: 'Serial-number tracking commands',
  actions: {
    list: { description: 'List serials', args: [] },
    get: { description: 'Get serial', args: ['<serialId|serial>'] },
    create: {
      description: 'Create serial',
      args: ['<sku>', '[serial]', '[lotNumber]', '[manufacturedAt]'],
    },
    available: { description: 'List available serials', args: ['<sku>', '[limit]'] },
    sold: { description: 'Mark serial sold', args: ['<serialId>', '<customerId>', '[orderId]'] },
    quarantine: { description: 'Quarantine serial', args: ['<serialId>', '<reason>'] },
    check: { description: 'Check serial availability', args: ['<serial>'] },
    count: { description: 'Count serials', args: [] },
  },
};

export default { execute, metadata };
