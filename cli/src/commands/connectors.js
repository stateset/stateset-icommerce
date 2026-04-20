/**
 * Connectors Commands Module
 */

import {
  assessConnectorSafety,
  certifyConnector,
  executeInstalledConnectorAction,
  getInstalledConnector,
  installConnector,
  listConnectorMarketplace,
  listInstalledConnectors,
  publishConnector,
  signConnectorAttestation,
  uninstallConnector,
  verifyConnectorAttestation,
} from '../connectors/wasm-marketplace.js';

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { output, jsonOutput }) {
  switch (action) {
    case 'marketplace': {
      const [connectorId, query, tag, limitRaw] = args;
      const result = await listConnectorMarketplace({
        connectorId: connectorId || undefined,
        query: query || undefined,
        tag: tag || undefined,
        limit: limitRaw ? Number.parseInt(limitRaw, 10) : undefined,
      });
      return formatConnectorList(result.connectors || result, { output, jsonOutput });
    }

    case 'publish': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: connectors publish <payloadJson>');
      const result = await publishConnector(parseJsonArg(payloadJson, 'payload'));
      return { result, formatted: `Published connector ${result.connectorId || result.id}` };
    }

    case 'install': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: connectors install <payloadJson>');
      const result = await installConnector(parseJsonArg(payloadJson, 'payload'));
      return { result, formatted: `Installed connector ${result.connectorId || result.id}` };
    }

    case 'assess': {
      const [connectorId, version] = args;
      if (!connectorId) throw new Error('Usage: connectors assess <connectorId> [version]');
      const result = await assessConnectorSafety({ connectorId, version: version || null });
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Connector safety\n` +
              `${'-'.repeat(28)}\n` +
              `Connector:    ${connectorId}\n` +
              `Version:      ${result.version || version || 'latest'}\n` +
              `Score:        ${result.safetyScore ?? 'N/A'}\n` +
              `Status:       ${result.status || 'N/A'}`,
          };
    }

    case 'certify': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: connectors certify <payloadJson>');
      const result = await certifyConnector(parseJsonArg(payloadJson, 'payload'));
      return { result, formatted: `Certified connector ${result.connectorId || result.id}` };
    }

    case 'sign': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: connectors sign <payloadJson>');
      const result = await signConnectorAttestation(parseJsonArg(payloadJson, 'payload'));
      return {
        result,
        formatted: `Signed connector attestation for ${result.connectorId || result.id}`,
      };
    }

    case 'verify': {
      const [connectorId, version] = args;
      if (!connectorId) throw new Error('Usage: connectors verify <connectorId> [version]');
      const result = await verifyConnectorAttestation({ connectorId, version: version || null });
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Connector attestation\n` +
              `${'-'.repeat(34)}\n` +
              `Connector:    ${connectorId}\n` +
              `Verified:     ${result.verified ? 'yes' : 'no'}\n` +
              `Version:      ${result.version || version || 'latest'}`,
          };
    }

    case 'uninstall': {
      const [connectorId, version] = args;
      if (!connectorId) throw new Error('Usage: connectors uninstall <connectorId> [version]');
      const result = await uninstallConnector({ connectorId, version: version || null });
      return { result, formatted: `Uninstalled connector ${connectorId}` };
    }

    case 'installed': {
      const connectorId = args[0];
      const result = await listInstalledConnectors({ connectorId: connectorId || null });
      return formatConnectorList(result.connectors || result, { output, jsonOutput });
    }

    case 'get': {
      const [connectorId, version] = args;
      if (!connectorId) throw new Error('Usage: connectors get <connectorId> [version]');
      const result = await getInstalledConnector({ connectorId, version: version || null });
      if (!result) throw new Error(`Installed connector not found: ${connectorId}`);
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Installed connector\n` +
              `${'-'.repeat(32)}\n` +
              `Connector:    ${result.connectorId || connectorId}\n` +
              `Version:      ${result.version || version || 'latest'}\n` +
              `Runtime:      ${result.runtimeKind || 'N/A'}\n` +
              `Actions:      ${Array.isArray(result.actions) ? result.actions.length : 0}`,
          };
    }

    case 'execute': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: connectors execute <payloadJson>');
      const result = await executeInstalledConnectorAction(parseJsonArg(payloadJson, 'payload'));
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Connector execution complete\n` +
              `${'-'.repeat(34)}\n` +
              `Connector:    ${result.connectorId || 'N/A'}\n` +
              `Action:       ${result.action || 'N/A'}\n` +
              `Success:      ${result.success === false ? 'no' : 'yes'}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: connectors ${action}\n\n` +
          'Available actions:\n' +
          '  marketplace [connectorId] [query] [tag] [limit]  List connector marketplace\n' +
          '  publish <payloadJson>                            Publish WASM connector\n' +
          '  install <payloadJson>                            Install WASM connector\n' +
          '  assess <connectorId> [version]                   Assess connector safety\n' +
          '  certify <payloadJson>                            Certify connector\n' +
          '  sign <payloadJson>                               Sign connector attestation\n' +
          '  verify <connectorId> [version]                   Verify connector attestation\n' +
          '  uninstall <connectorId> [version]                Uninstall connector\n' +
          '  installed [connectorId]                          List installed connectors\n' +
          '  get <connectorId> [version]                      Get installed connector\n' +
          '  execute <payloadJson>                            Execute installed connector action',
      );
  }
}

function formatConnectorList(connectors, { output, jsonOutput }) {
  const rows = Array.isArray(connectors) ? connectors : [];
  if (jsonOutput) return rows;
  if (rows.length === 0) return { formatted: 'No connectors found.' };
  const formatted = output.table(
    rows.map((connector) => ({
      connectorId: connector.connectorId || connector.id,
      version: connector.version,
      name: connector.name,
      runtimeKind: connector.runtimeKind,
      publisher: connector.publisher,
    })),
    [
      { key: 'connectorId', header: 'Connector' },
      { key: 'version', header: 'Version' },
      { key: 'name', header: 'Name' },
      { key: 'runtimeKind', header: 'Runtime' },
      { key: 'publisher', header: 'Publisher' },
    ],
  );
  return { connectors: rows, formatted };
}

export const metadata = {
  name: 'connectors',
  aliases: ['conn', 'wasm'],
  description: 'WASM connector marketplace and execution commands',
  actions: {
    marketplace: {
      description: 'List connector marketplace',
      args: ['[connectorId]', '[query]', '[tag]', '[limit]'],
    },
    publish: { description: 'Publish connector', args: ['<payloadJson>'] },
    install: { description: 'Install connector', args: ['<payloadJson>'] },
    assess: { description: 'Assess connector safety', args: ['<connectorId>', '[version]'] },
    certify: { description: 'Certify connector', args: ['<payloadJson>'] },
    sign: { description: 'Sign connector attestation', args: ['<payloadJson>'] },
    verify: { description: 'Verify connector attestation', args: ['<connectorId>', '[version]'] },
    uninstall: { description: 'Uninstall connector', args: ['<connectorId>', '[version]'] },
    installed: { description: 'List installed connectors', args: ['[connectorId]'] },
    get: { description: 'Get installed connector', args: ['<connectorId>', '[version]'] },
    execute: { description: 'Execute installed connector', args: ['<payloadJson>'] },
  },
};

export default { execute, metadata };
