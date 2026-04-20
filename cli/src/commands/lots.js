/**
 * Lots Commands Module
 */

function parseNumber(value, usage, { allowZero = false } = {}) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || (!allowZero && parsed <= 0) || (allowZero && parsed < 0)) {
    throw new Error(usage);
  }
  return parsed;
}

async function resolveLot(lotsApi, identifier) {
  return lotsApi.get(identifier) || lotsApi.getByNumber(identifier);
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const lots = await commerce.lots.list();
      return formatLots(lots, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: lots get <lotId|lotNumber>');
      const lot = await resolveLot(commerce.lots, identifier);
      if (!lot) throw new Error(`Lot not found: ${identifier}`);
      return formatLot(lot, { jsonOutput });
    }

    case 'create': {
      const [
        sku,
        quantityProducedRaw,
        lotNumber,
        productionDate,
        expirationDate,
        supplierLotNumber,
      ] = args;
      if (!sku || !quantityProducedRaw) {
        throw new Error(
          'Usage: lots create <sku> <quantityProduced> [lotNumber] [productionDate] [expirationDate] [supplierLotNumber]',
        );
      }
      const lot = await commerce.lots.create({
        sku,
        quantityProduced: parseNumber(
          quantityProducedRaw,
          'Usage: lots create <sku> <quantityProduced> [lotNumber] [productionDate] [expirationDate] [supplierLotNumber]',
        ),
        lotNumber: lotNumber || undefined,
        productionDate: productionDate || undefined,
        expirationDate: expirationDate || undefined,
        supplierLotNumber: supplierLotNumber || undefined,
      });
      return { lot, formatted: `Created lot ${lot.lotNumber || lot.id}` };
    }

    case 'active': {
      const sku = args[0];
      if (!sku) throw new Error('Usage: lots active <sku>');
      const lots = await commerce.lots.getActiveLots(sku);
      return formatLots(lots, { output, jsonOutput });
    }

    case 'available': {
      const sku = args[0];
      if (!sku) throw new Error('Usage: lots available <sku>');
      const lots = await commerce.lots.getAvailableLotsForSku(sku);
      return formatLots(lots, { output, jsonOutput });
    }

    case 'quarantine': {
      const [lotId, ...reasonParts] = args;
      if (!lotId || reasonParts.length === 0) {
        throw new Error('Usage: lots quarantine <lotId> <reason>');
      }
      const lot = await commerce.lots.quarantine(lotId, reasonParts.join(' '));
      return { lot, formatted: `Quarantined lot ${lot.lotNumber || lot.id}` };
    }

    case 'release': {
      const lotId = args[0];
      if (!lotId) throw new Error('Usage: lots release <lotId>');
      const lot = await commerce.lots.releaseQuarantine(lotId);
      return { lot, formatted: `Released lot ${lot.lotNumber || lot.id} from quarantine` };
    }

    case 'expiring': {
      const days = parseNumber(args[0] || '30', 'Usage: lots expiring [days]', { allowZero: true });
      const lots = await commerce.lots.getExpiringLots(days);
      return formatLots(lots, { output, jsonOutput });
    }

    case 'expired': {
      const lots = await commerce.lots.getExpiredLots();
      return formatLots(lots, { output, jsonOutput });
    }

    case 'quarantined': {
      const lots = await commerce.lots.getQuarantined();
      return formatLots(lots, { output, jsonOutput });
    }

    case 'count': {
      const count = await commerce.lots.count();
      return { count, formatted: `Lot count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: lots ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                   List lots\n' +
          '  get <lotId|lotNumber>                                                  Get lot\n' +
          '  create <sku> <quantityProduced> [lotNumber] [productionDate] [expirationDate] [supplierLotNumber]\n' +
          '  active <sku>                                                           List active lots for SKU\n' +
          '  available <sku>                                                        List available lots for SKU\n' +
          '  quarantine <lotId> <reason>                                            Quarantine lot\n' +
          '  release <lotId>                                                        Release lot quarantine\n' +
          '  expiring [days]                                                        List expiring lots\n' +
          '  expired                                                                List expired lots\n' +
          '  quarantined                                                            List quarantined lots\n' +
          '  count                                                                  Count lots',
      );
  }
}

function formatLots(lots, { output, jsonOutput }) {
  if (jsonOutput) return lots;
  if (lots.length === 0) return { formatted: 'No lots found.' };
  const formatted = output.table(lots, [
    { key: 'id', header: 'ID' },
    { key: 'lotNumber', header: 'Lot #' },
    { key: 'sku', header: 'SKU' },
    { key: 'status', header: 'Status' },
    { key: 'quantityProduced', header: 'Qty', align: 'right' },
    { key: 'expirationDate', header: 'Expires' },
  ]);
  return { lots, formatted };
}

function formatLot(lot, { jsonOutput }) {
  if (jsonOutput) return lot;
  return {
    lot,
    formatted:
      `Lot: ${lot.lotNumber || lot.id}\n` +
      `${'-'.repeat(28)}\n` +
      `SKU:             ${lot.sku}\n` +
      `Status:          ${lot.status}\n` +
      `Qty produced:    ${lot.quantityProduced}\n` +
      `Production:      ${lot.productionDate || 'N/A'}\n` +
      `Expiration:      ${lot.expirationDate || 'N/A'}\n` +
      `Supplier lot:    ${lot.supplierLotNumber || 'N/A'}`,
  };
}

export const metadata = {
  name: 'lots',
  aliases: ['lot', 'batches'],
  description: 'Lot and batch tracking commands',
  actions: {
    list: { description: 'List lots', args: [] },
    get: { description: 'Get lot', args: ['<lotId|lotNumber>'] },
    create: {
      description: 'Create lot',
      args: [
        '<sku>',
        '<quantityProduced>',
        '[lotNumber]',
        '[productionDate]',
        '[expirationDate]',
        '[supplierLotNumber]',
      ],
    },
    active: { description: 'List active lots', args: ['<sku>'] },
    available: { description: 'List available lots', args: ['<sku>'] },
    quarantine: { description: 'Quarantine lot', args: ['<lotId>', '<reason>'] },
    release: { description: 'Release lot quarantine', args: ['<lotId>'] },
    expiring: { description: 'List expiring lots', args: ['[days]'] },
    expired: { description: 'List expired lots', args: [] },
    quarantined: { description: 'List quarantined lots', args: [] },
    count: { description: 'Count lots', args: [] },
  },
};

export default { execute, metadata };
