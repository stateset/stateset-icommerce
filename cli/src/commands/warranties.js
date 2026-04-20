/**
 * Warranties Commands Module
 */

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [customerId, status] = args;
      const warranties = await commerce.warranties.list();
      const filtered = warranties.filter(
        (warranty) =>
          (!customerId || warranty.customerId === customerId) &&
          (!status || warranty.status === status),
      );
      return formatWarrantyList(filtered, { output, jsonOutput });
    }

    case 'get': {
      const warrantyId = args[0];
      if (!warrantyId) throw new Error('Usage: warranties get <warrantyId>');
      const warranty = await commerce.warranties.get(warrantyId);
      if (!warranty) throw new Error(`Warranty not found: ${warrantyId}`);
      return formatWarrantyDetail(warranty, { jsonOutput });
    }

    case 'create': {
      const [
        customerId,
        orderId,
        productId,
        warrantyType = 'standard',
        durationMonthsRaw = '12',
        serialNumber,
      ] = args;
      if (!customerId) {
        throw new Error(
          'Usage: warranties create <customerId> [orderId] [productId] [warrantyType] [durationMonths] [serialNumber]',
        );
      }
      const durationMonths = Number.parseInt(durationMonthsRaw, 10);
      const warranty = await commerce.warranties.create({
        customerId,
        orderId,
        productId,
        warrantyType,
        durationMonths:
          Number.isInteger(durationMonths) && durationMonths > 0 ? durationMonths : 12,
        serialNumber,
      });
      return {
        warranty,
        formatted: `Created warranty ${warranty.id}`,
      };
    }

    case 'claim': {
      const [warrantyId, issueDescription, contactEmail, contactPhone] = args;
      if (!warrantyId || !issueDescription) {
        throw new Error(
          'Usage: warranties claim <warrantyId> <issueDescription> [contactEmail] [contactPhone]',
        );
      }
      const claim = await commerce.warranties.createClaim({
        warrantyId,
        issueDescription,
        contactEmail,
        contactPhone,
      });
      return {
        claim,
        formatted: `Filed warranty claim ${claim.id}`,
      };
    }

    case 'approve': {
      const claimId = args[0];
      if (!claimId) throw new Error('Usage: warranties approve <claimId>');
      const claim = await commerce.warranties.approveClaim(claimId);
      return {
        claim,
        formatted: `Approved warranty claim ${claim.id}`,
      };
    }

    case 'deny': {
      const [claimId, ...reasonParts] = args;
      if (!claimId || reasonParts.length === 0) {
        throw new Error('Usage: warranties deny <claimId> <reason>');
      }
      const claim = await commerce.warranties.denyClaim(claimId, reasonParts.join(' '));
      return {
        claim,
        formatted: `Denied warranty claim ${claim.id}`,
      };
    }

    case 'complete': {
      const [claimId, resolution] = args;
      if (!claimId || !resolution) {
        throw new Error('Usage: warranties complete <claimId> <resolution>');
      }
      const claim = await commerce.warranties.completeClaim(claimId, resolution);
      return {
        claim,
        formatted: `Completed warranty claim ${claim.id}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: warranties ${action}\n\n` +
          'Available actions:\n' +
          '  list [customerId] [status]                           List warranties\n' +
          '  get <warrantyId>                                     Get warranty details\n' +
          '  create <customerId> [orderId] [productId] [warrantyType] [durationMonths] [serialNumber]\n' +
          '  claim <warrantyId> <issueDescription> [contactEmail] [contactPhone] File claim\n' +
          '  approve <claimId>                                    Approve claim\n' +
          '  deny <claimId> <reason>                              Deny claim\n' +
          '  complete <claimId> <resolution>                      Complete claim',
      );
  }
}

function formatWarrantyList(warranties, { output, jsonOutput }) {
  if (jsonOutput) return warranties;
  if (warranties.length === 0) return { formatted: 'No warranties found.' };
  const formatted = output.table(warranties, [
    { key: 'id', header: 'Warranty' },
    { key: 'customerId', header: 'Customer' },
    { key: 'warrantyType', header: 'Type' },
    { key: 'status', header: 'Status' },
    { key: 'expiresAt', header: 'Expires' },
  ]);
  return { warranties, formatted };
}

function formatWarrantyDetail(warranty, { jsonOutput }) {
  if (jsonOutput) return warranty;
  return {
    warranty,
    formatted:
      `Warranty: ${warranty.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Customer:     ${warranty.customerId}\n` +
      `Order:        ${warranty.orderId || 'N/A'}\n` +
      `Product:      ${warranty.productId || 'N/A'}\n` +
      `Type:         ${warranty.warrantyType}\n` +
      `Status:       ${warranty.status}\n` +
      `Expires:      ${warranty.expiresAt || 'N/A'}`,
  };
}

export const metadata = {
  name: 'warranties',
  aliases: ['warranty', 'claims'],
  description: 'Warranty lifecycle and claim commands',
  actions: {
    list: { description: 'List warranties', args: ['[customerId]', '[status]'] },
    get: { description: 'Get warranty', args: ['<warrantyId>'] },
    create: {
      description: 'Create warranty',
      args: [
        '<customerId>',
        '[orderId]',
        '[productId]',
        '[warrantyType]',
        '[durationMonths]',
        '[serialNumber]',
      ],
    },
    claim: {
      description: 'Create warranty claim',
      args: ['<warrantyId>', '<issueDescription>', '[contactEmail]', '[contactPhone]'],
    },
    approve: { description: 'Approve claim', args: ['<claimId>'] },
    deny: { description: 'Deny claim', args: ['<claimId>', '<reason>'] },
    complete: { description: 'Complete claim', args: ['<claimId>', '<resolution>'] },
  },
};

export default { execute, metadata };
