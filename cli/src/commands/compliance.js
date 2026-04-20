/**
 * Compliance Commands Module
 */

let complianceSvcPromise = null;

async function getComplianceSvc() {
  if (!complianceSvcPromise) {
    complianceSvcPromise = (async () => {
      const { A2AStore } = await import('../a2a/store.js');
      const { createComplianceService } = await import('../compliance/exports.js');
      const path = await import('node:path');
      const store = new A2AStore();
      store.init();
      const commerceDbPath = store.dbPath
        ? path.resolve(path.dirname(store.dbPath), 'store.db')
        : './store.db';
      return createComplianceService(store, { commerceDbPath });
    })();
  }
  return complianceSvcPromise;
}

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { jsonOutput }) {
  const svc = await getComplianceSvc();

  switch (action) {
    case 'audit-trail': {
      const payload = args[0] ? parseJsonArg(args[0], 'payload') : {};
      const result = svc.exportAuditTrail(payload);
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Compliance audit trail\n` +
              `${'-'.repeat(34)}\n` +
              `Format:      ${result.format || payload.format || 'json'}\n` +
              `Records:     ${result.records?.length ?? result.count ?? 'N/A'}`,
          };
    }

    case '1099k': {
      const [yearRaw, agentAddress] = args;
      if (!yearRaw || !agentAddress)
        throw new Error('Usage: compliance 1099k <year> <agentAddress>');
      const result = svc.generate1099K({
        year: Number.parseInt(yearRaw, 10),
        agentAddress,
      });
      return jsonOutput
        ? result
        : { result, formatted: `Generated 1099-K for ${agentAddress} (${yearRaw})` };
    }

    case 'export-gdpr': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: compliance export-gdpr <customerId>');
      const result = svc.generateGDPRExport(customerId);
      return jsonOutput ? result : { result, formatted: `Exported GDPR data for ${customerId}` };
    }

    case 'delete-gdpr': {
      const [customerId, keepTransactionsRaw] = args;
      if (!customerId)
        throw new Error('Usage: compliance delete-gdpr <customerId> [keepTransactions]');
      const result = svc.deleteGDPRData(customerId, {
        keepTransactions: ['true', '1', 'yes', 'y'].includes(
          String(keepTransactionsRaw || '').toLowerCase(),
        ),
      });
      return jsonOutput ? result : { result, formatted: `Deleted GDPR data for ${customerId}` };
    }

    case 'summary': {
      const [period = 'month', agentName] = args;
      const result = svc.generateComplianceSummary({
        period,
        agentName: agentName || undefined,
      });
      return jsonOutput
        ? result
        : { result, formatted: `Compliance summary generated for period ${period}` };
    }

    case 'soc2': {
      const controlsJson = args[0];
      if (!controlsJson) throw new Error('Usage: compliance soc2 <controlsJson>');
      const result = svc.generateSOC2Evidence({
        controls: parseJsonArg(controlsJson, 'controls'),
      });
      return jsonOutput
        ? result
        : {
            result,
            formatted: `Generated SOC2 evidence for ${result.controls?.length || 'requested'} controls`,
          };
    }

    default:
      throw new Error(
        `Unknown action: compliance ${action}\n\n` +
          'Available actions:\n' +
          '  audit-trail [payloadJson]             Export audit trail\n' +
          '  1099k <year> <agentAddress>           Generate 1099-K report\n' +
          '  export-gdpr <customerId>              Export GDPR data\n' +
          '  delete-gdpr <customerId> [keepTransactions]  Delete GDPR data\n' +
          '  summary [period] [agentName]          Generate compliance summary\n' +
          '  soc2 <controlsJson>                   Generate SOC2 evidence',
      );
  }
}

export const metadata = {
  name: 'compliance',
  aliases: ['cmp', 'regulatory'],
  description: 'Compliance export and reporting commands',
  actions: {
    'audit-trail': { description: 'Export audit trail', args: ['[payloadJson]'] },
    '1099k': { description: 'Generate 1099-K report', args: ['<year>', '<agentAddress>'] },
    'export-gdpr': { description: 'Export GDPR data', args: ['<customerId>'] },
    'delete-gdpr': {
      description: 'Delete GDPR data',
      args: ['<customerId>', '[keepTransactions]'],
    },
    summary: { description: 'Generate compliance summary', args: ['[period]', '[agentName]'] },
    soc2: { description: 'Generate SOC2 evidence', args: ['<controlsJson>'] },
  },
};

export default { execute, metadata };
