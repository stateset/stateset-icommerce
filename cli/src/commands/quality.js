/**
 * Quality Commands Module
 */

function parsePositive(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

function parseOptionalInt(value, usage) {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed)) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'inspections': {
      const inspections = await commerce.quality.listInspections();
      return formatInspections(inspections, { output, jsonOutput });
    }

    case 'inspection': {
      const inspectionId = args[0];
      if (!inspectionId) throw new Error('Usage: quality inspection <inspectionId>');
      const inspection = await commerce.quality.getInspection(inspectionId);
      if (!inspection) throw new Error(`Inspection not found: ${inspectionId}`);
      return formatInspection(inspection, { jsonOutput });
    }

    case 'create-inspection': {
      const [inspectionType, referenceType, referenceId, warehouseIdRaw, assignedTo, ...noteParts] =
        args;
      if (!inspectionType || !referenceType || !referenceId) {
        throw new Error(
          'Usage: quality create-inspection <inspectionType> <referenceType> <referenceId> [warehouseId] [assignedTo] [notes]',
        );
      }
      const inspection = await commerce.quality.createInspection({
        inspectionType,
        referenceType,
        referenceId,
        warehouseId: parseOptionalInt(warehouseIdRaw, 'warehouseId must be an integer'),
        assignedTo: assignedTo || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return { inspection, formatted: `Created inspection ${inspection.id}` };
    }

    case 'start-inspection': {
      const inspectionId = args[0];
      if (!inspectionId) throw new Error('Usage: quality start-inspection <inspectionId>');
      const inspection = await commerce.quality.startInspection(inspectionId);
      return { inspection, formatted: `Started inspection ${inspection.id}` };
    }

    case 'complete-inspection': {
      const inspectionId = args[0];
      if (!inspectionId) throw new Error('Usage: quality complete-inspection <inspectionId>');
      const inspection = await commerce.quality.completeInspection(inspectionId);
      return { inspection, formatted: `Completed inspection ${inspection.id}` };
    }

    case 'ncrs': {
      const ncrs = await commerce.quality.listNcrs();
      return formatNcrs(ncrs, { output, jsonOutput });
    }

    case 'ncr': {
      const ncrId = args[0];
      if (!ncrId) throw new Error('Usage: quality ncr <ncrId>');
      const ncr = await commerce.quality.getNcr(ncrId);
      if (!ncr) throw new Error(`NCR not found: ${ncrId}`);
      return formatNcr(ncr, { jsonOutput });
    }

    case 'create-ncr': {
      const [source, severity, sku, quantityAffectedRaw, description, lotNumber, locationIdRaw] =
        args;
      if (!source || !severity || !sku || !quantityAffectedRaw || !description) {
        throw new Error(
          'Usage: quality create-ncr <source> <severity> <sku> <quantityAffected> <description> [lotNumber] [locationId]',
        );
      }
      const ncr = await commerce.quality.createNcr({
        source,
        severity,
        sku,
        quantityAffected: parsePositive(
          quantityAffectedRaw,
          'Usage: quality create-ncr <source> <severity> <sku> <quantityAffected> <description> [lotNumber] [locationId]',
        ),
        description,
        lotNumber: lotNumber || undefined,
        locationId: parseOptionalInt(locationIdRaw, 'locationId must be an integer'),
      });
      return { ncr, formatted: `Created NCR ${ncr.id}` };
    }

    case 'close-ncr': {
      const ncrId = args[0];
      if (!ncrId) throw new Error('Usage: quality close-ncr <ncrId>');
      const ncr = await commerce.quality.closeNcr(ncrId);
      return { ncr, formatted: `Closed NCR ${ncr.id}` };
    }

    case 'holds': {
      const holds = await commerce.quality.listHolds();
      return formatHolds(holds, { output, jsonOutput });
    }

    case 'hold': {
      const holdId = args[0];
      if (!holdId) throw new Error('Usage: quality hold <holdId>');
      const hold = await commerce.quality.getHold(holdId);
      if (!hold) throw new Error(`Quality hold not found: ${holdId}`);
      return formatHold(hold, { jsonOutput });
    }

    case 'create-hold': {
      const [sku, lotNumber, quantityHeldRaw, reason, holdType, placedBy, locationIdRaw] = args;
      if (!sku || !quantityHeldRaw || !reason || !holdType) {
        throw new Error(
          'Usage: quality create-hold <sku> [lotNumber] <quantityHeld> <reason> <holdType> [placedBy] [locationId]',
        );
      }
      const hold = await commerce.quality.createHold({
        sku,
        lotNumber: lotNumber || undefined,
        quantityHeld: parsePositive(
          quantityHeldRaw,
          'Usage: quality create-hold <sku> [lotNumber] <quantityHeld> <reason> <holdType> [placedBy] [locationId]',
        ),
        reason,
        holdType,
        placedBy: placedBy || undefined,
        locationId: parseOptionalInt(locationIdRaw, 'locationId must be an integer'),
      });
      return { hold, formatted: `Created quality hold ${hold.id}` };
    }

    case 'release-hold': {
      const [holdId, releasedBy, ...noteParts] = args;
      if (!holdId || !releasedBy) {
        throw new Error('Usage: quality release-hold <holdId> <releasedBy> [notes]');
      }
      const hold = await commerce.quality.releaseHold(
        holdId,
        releasedBy,
        noteParts.join(' ') || undefined,
      );
      return { hold, formatted: `Released quality hold ${hold.id}` };
    }

    case 'active-holds': {
      const holds = await commerce.quality.getActiveHolds();
      return formatHolds(holds, { output, jsonOutput });
    }

    case 'count': {
      const count = await commerce.quality.countActiveHolds();
      return { count, formatted: `Active quality hold count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: quality ${action}\n\n` +
          'Available actions:\n' +
          '  inspections                                                            List inspections\n' +
          '  inspection <inspectionId>                                              Get inspection\n' +
          '  create-inspection <inspectionType> <referenceType> <referenceId> [warehouseId] [assignedTo] [notes]\n' +
          '  start-inspection <inspectionId>                                        Start inspection\n' +
          '  complete-inspection <inspectionId>                                     Complete inspection\n' +
          '  ncrs                                                                   List NCRs\n' +
          '  ncr <ncrId>                                                            Get NCR\n' +
          '  create-ncr <source> <severity> <sku> <quantityAffected> <description> [lotNumber] [locationId]\n' +
          '  close-ncr <ncrId>                                                      Close NCR\n' +
          '  holds                                                                  List quality holds\n' +
          '  hold <holdId>                                                          Get quality hold\n' +
          '  create-hold <sku> [lotNumber] <quantityHeld> <reason> <holdType> [placedBy] [locationId]\n' +
          '  release-hold <holdId> <releasedBy> [notes]                             Release quality hold\n' +
          '  active-holds                                                           List active quality holds\n' +
          '  count                                                                  Count active quality holds',
      );
  }
}

function formatInspections(inspections, { output, jsonOutput }) {
  if (jsonOutput) return inspections;
  if (inspections.length === 0) return { formatted: 'No inspections found.' };
  const formatted = output.table(inspections, [
    { key: 'id', header: 'ID' },
    { key: 'inspectionType', header: 'Type' },
    { key: 'referenceType', header: 'Ref Type' },
    { key: 'referenceId', header: 'Ref ID' },
    { key: 'status', header: 'Status' },
    { key: 'assignedTo', header: 'Assigned To' },
  ]);
  return { inspections, formatted };
}

function formatInspection(inspection, { jsonOutput }) {
  if (jsonOutput) return inspection;
  return {
    inspection,
    formatted:
      `Inspection: ${inspection.id}\n` +
      `${'-'.repeat(34)}\n` +
      `Type:           ${inspection.inspectionType}\n` +
      `Reference:      ${inspection.referenceType}:${inspection.referenceId}\n` +
      `Warehouse:      ${inspection.warehouseId || 'N/A'}\n` +
      `Assigned:       ${inspection.assignedTo || 'N/A'}\n` +
      `Status:         ${inspection.status}`,
  };
}

function formatNcrs(ncrs, { output, jsonOutput }) {
  if (jsonOutput) return ncrs;
  if (ncrs.length === 0) return { formatted: 'No NCRs found.' };
  const formatted = output.table(ncrs, [
    { key: 'id', header: 'ID' },
    { key: 'source', header: 'Source' },
    { key: 'severity', header: 'Severity' },
    { key: 'sku', header: 'SKU' },
    { key: 'quantityAffected', header: 'Qty', align: 'right' },
    { key: 'status', header: 'Status' },
  ]);
  return { ncrs, formatted };
}

function formatNcr(ncr, { jsonOutput }) {
  if (jsonOutput) return ncr;
  return {
    ncr,
    formatted:
      `NCR: ${ncr.id}\n` +
      `${'-'.repeat(22)}\n` +
      `Source:         ${ncr.source}\n` +
      `Severity:       ${ncr.severity}\n` +
      `SKU:            ${ncr.sku}\n` +
      `Qty affected:   ${ncr.quantityAffected}\n` +
      `Status:         ${ncr.status}`,
  };
}

function formatHolds(holds, { output, jsonOutput }) {
  if (jsonOutput) return holds;
  if (holds.length === 0) return { formatted: 'No quality holds found.' };
  const formatted = output.table(holds, [
    { key: 'id', header: 'ID' },
    { key: 'sku', header: 'SKU' },
    { key: 'lotNumber', header: 'Lot' },
    { key: 'quantityHeld', header: 'Qty', align: 'right' },
    { key: 'holdType', header: 'Type' },
    { key: 'status', header: 'Status' },
  ]);
  return { holds, formatted };
}

function formatHold(hold, { jsonOutput }) {
  if (jsonOutput) return hold;
  return {
    hold,
    formatted:
      `Quality hold: ${hold.id}\n` +
      `${'-'.repeat(30)}\n` +
      `SKU:            ${hold.sku}\n` +
      `Lot:            ${hold.lotNumber || 'N/A'}\n` +
      `Quantity held:  ${hold.quantityHeld}\n` +
      `Type:           ${hold.holdType}\n` +
      `Status:         ${hold.status}\n` +
      `Reason:         ${hold.reason}`,
  };
}

export const metadata = {
  name: 'quality',
  aliases: ['qa', 'ncr'],
  description: 'Quality inspections, NCRs, and holds',
  actions: {
    inspections: { description: 'List inspections', args: [] },
    inspection: { description: 'Get inspection', args: ['<inspectionId>'] },
    'create-inspection': {
      description: 'Create inspection',
      args: [
        '<inspectionType>',
        '<referenceType>',
        '<referenceId>',
        '[warehouseId]',
        '[assignedTo]',
        '[notes]',
      ],
    },
    'start-inspection': { description: 'Start inspection', args: ['<inspectionId>'] },
    'complete-inspection': { description: 'Complete inspection', args: ['<inspectionId>'] },
    ncrs: { description: 'List NCRs', args: [] },
    ncr: { description: 'Get NCR', args: ['<ncrId>'] },
    'create-ncr': {
      description: 'Create NCR',
      args: [
        '<source>',
        '<severity>',
        '<sku>',
        '<quantityAffected>',
        '<description>',
        '[lotNumber]',
        '[locationId]',
      ],
    },
    'close-ncr': { description: 'Close NCR', args: ['<ncrId>'] },
    holds: { description: 'List quality holds', args: [] },
    hold: { description: 'Get quality hold', args: ['<holdId>'] },
    'create-hold': {
      description: 'Create quality hold',
      args: [
        '<sku>',
        '[lotNumber]',
        '<quantityHeld>',
        '<reason>',
        '<holdType>',
        '[placedBy]',
        '[locationId]',
      ],
    },
    'release-hold': {
      description: 'Release quality hold',
      args: ['<holdId>', '<releasedBy>', '[notes]'],
    },
    'active-holds': { description: 'List active quality holds', args: [] },
    count: { description: 'Count active quality holds', args: [] },
  },
};

export default { execute, metadata };
