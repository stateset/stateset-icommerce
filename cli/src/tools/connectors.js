import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';
import {
  assessConnectorSafety,
  certifyConnector,
  CONNECTOR_RUNTIME_KINDS,
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

const ConnectorActionSchema = z.object({
  name: z.string().min(2).max(64).describe('Connector action name'),
  description: z.string().max(400).optional().describe('Action description'),
  exportName: z
    .string()
    .min(1)
    .max(128)
    .optional()
    .describe('WASM export name for native-export runtime'),
  args: z
    .array(z.string().min(1).max(128))
    .optional()
    .describe('Ordered argument names for native-export runtime'),
  commandArgs: z
    .array(z.string().min(1).max(256))
    .optional()
    .describe('Command-line args for wasi-command runtime'),
  timeoutMs: z
    .number()
    .int()
    .min(50)
    .max(300000)
    .optional()
    .describe('Optional action timeout in milliseconds'),
  inputSchema: z
    .record(z.string(), z.any())
    .optional()
    .describe('Optional loose input schema metadata'),
});

export const connectorTools = [
  {
    name: 'list_connector_marketplace',
    description: 'List available WASM connectors in the local marketplace catalog.',
    inputSchema: {
      connectorId: z.string().optional().describe('Optional connector ID filter'),
      query: z.string().optional().describe('Optional text query across name/description/tags'),
      tag: z.string().optional().describe('Optional tag filter'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum results to return'),
    },
    permission: 'read',
    policyDomain: 'connectors',
    handler: async ({ params }) => {
      return listConnectorMarketplace({
        connectorId: params.connectorId,
        query: params.query,
        tag: params.tag,
        limit: params.limit,
      });
    },
  },
  {
    name: 'publish_wasm_connector',
    description:
      'Publish a WASM connector to the local marketplace catalog (app-store style ecosystem index).',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z.string().min(1).max(64).optional().describe('Connector version (default: 0.1.0)'),
      name: z.string().min(1).max(120).optional().describe('Display name'),
      description: z.string().max(1000).optional().describe('Connector description'),
      wasmPath: z.string().min(1).describe('Path to .wasm module file'),
      runtimeKind: z
        .enum(CONNECTOR_RUNTIME_KINDS)
        .optional()
        .describe('Execution runtime kind (native-export or wasi-command)'),
      publisher: z.string().max(120).optional().describe('Publisher name or ID'),
      tags: z.array(z.string().min(1).max(40)).optional().describe('Marketplace tags'),
      actions: z.array(ConnectorActionSchema).optional().describe('Connector actions'),
      force: z.boolean().optional().describe('Overwrite existing catalog entry'),
    },
    permission: 'admin',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Publish WASM connector', params);
      }

      return publishConnector({
        connectorId: params.connectorId,
        version: params.version || '0.1.0',
        name: params.name,
        description: params.description,
        wasmPath: params.wasmPath,
        runtimeKind: params.runtimeKind || 'native-export',
        publisher: params.publisher,
        tags: params.tags || [],
        actions: params.actions || [],
        force: params.force === true,
      });
    },
  },
  {
    name: 'install_wasm_connector',
    description: 'Install a connector from marketplace catalog into the local connector runtime.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z.string().min(1).max(64).optional().describe('Specific version to install'),
      force: z.boolean().optional().describe('Reinstall even if already installed'),
      verifyStrict: z
        .boolean()
        .optional()
        .describe('Override strict trust verification for this call'),
      requireCertified: z
        .boolean()
        .optional()
        .describe('Require connector certification before install'),
      minSafetyScore: z
        .number()
        .int()
        .min(0)
        .max(100)
        .optional()
        .describe('Minimum connector safety score required for install'),
    },
    permission: 'write',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Install WASM connector', params);
      }
      return installConnector({
        connectorId: params.connectorId,
        version: params.version || null,
        force: params.force === true,
        verifyStrict: params.verifyStrict,
        requireCertified: params.requireCertified,
        minSafetyScore: params.minSafetyScore,
      });
    },
  },
  {
    name: 'assess_wasm_connector_safety',
    description:
      'Compute connector safety scorecard and risk signals for marketplace governance and installation policy.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version to assess (defaults to latest in catalog)'),
    },
    permission: 'read',
    policyDomain: 'connectors',
    handler: async ({ params }) => {
      return assessConnectorSafety({
        connectorId: params.connectorId,
        version: params.version || null,
      });
    },
  },
  {
    name: 'certify_wasm_connector',
    description:
      'Issue marketplace certification metadata for a connector version using automated safety score + trust policy.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version to certify (defaults to latest in catalog)'),
      status: z
        .enum(['candidate', 'certified', 'revoked'])
        .optional()
        .describe('Certification status to set'),
      level: z.string().min(2).max(40).optional().describe('Optional certification level label'),
      assessor: z.string().max(120).optional().describe('Optional assessor identity'),
      notes: z.string().max(1000).optional().describe('Optional certification notes'),
      minSafetyScore: z
        .number()
        .int()
        .min(0)
        .max(100)
        .optional()
        .describe('Minimum score required when issuing certified status'),
      force: z
        .boolean()
        .optional()
        .describe('Bypass automated safety/attestation certification blockers'),
    },
    permission: 'admin',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Certify WASM connector', params);
      }
      return certifyConnector({
        connectorId: params.connectorId,
        version: params.version || null,
        status: params.status || 'certified',
        level: params.level || null,
        assessor: params.assessor || null,
        notes: params.notes || null,
        minSafetyScore: params.minSafetyScore ?? 70,
        force: params.force === true,
      });
    },
  },
  {
    name: 'sign_wasm_connector_attestation',
    description:
      'Sign a marketplace connector attestation using local signing key material for trustable install/execute verification.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version to sign (defaults to latest in catalog)'),
      keyId: z.string().max(120).optional().describe('Optional signer key identifier'),
      signedBy: z.string().max(120).optional().describe('Optional signer identity label'),
    },
    permission: 'admin',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Sign WASM connector attestation', params);
      }
      return signConnectorAttestation({
        connectorId: params.connectorId,
        version: params.version || null,
        keyId: params.keyId || null,
        signedBy: params.signedBy || null,
      });
    },
  },
  {
    name: 'verify_wasm_connector_attestation',
    description:
      'Verify connector trust attestation in the marketplace catalog before installation or execution.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version to verify (defaults to latest in catalog)'),
    },
    permission: 'read',
    policyDomain: 'connectors',
    handler: async ({ params }) => {
      return verifyConnectorAttestation({
        connectorId: params.connectorId,
        version: params.version || null,
      });
    },
  },
  {
    name: 'uninstall_wasm_connector',
    description: 'Uninstall a connector version from the local connector runtime.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version (defaults to latest installed version)'),
    },
    permission: 'delete',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Uninstall WASM connector', params);
      }
      return uninstallConnector({
        connectorId: params.connectorId,
        version: params.version || null,
      });
    },
  },
  {
    name: 'list_installed_connectors',
    description: 'List installed connectors available to agentic runtime execution.',
    inputSchema: {
      connectorId: z.string().optional().describe('Optional connector ID filter'),
    },
    permission: 'read',
    policyDomain: 'connectors',
    handler: async ({ params }) => {
      return listInstalledConnectors({
        connectorId: params.connectorId || null,
      });
    },
  },
  {
    name: 'get_installed_connector',
    description: 'Get details for an installed connector and its action contract.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version (defaults to latest installed version)'),
    },
    permission: 'read',
    policyDomain: 'connectors',
    handler: async ({ params }) => {
      return getInstalledConnector({
        connectorId: params.connectorId,
        version: params.version || null,
      });
    },
  },
  {
    name: 'execute_wasm_connector',
    description:
      'Execute an installed WASM connector action so agents can orchestrate ecosystem apps through iCommerce.',
    inputSchema: {
      connectorId: z.string().min(2).max(64).describe('Connector ID'),
      version: z
        .string()
        .min(1)
        .max(64)
        .optional()
        .describe('Specific version (defaults to latest installed version)'),
      action: z.string().min(2).max(64).describe('Connector action name'),
      params: z.record(z.string(), z.any()).optional().describe('Action parameters'),
      context: z.record(z.string(), z.any()).optional().describe('Execution context payload'),
      timeoutMs: z
        .number()
        .int()
        .min(50)
        .max(300000)
        .optional()
        .describe('Override action timeout in milliseconds'),
      verifyStrict: z
        .boolean()
        .optional()
        .describe('Override strict trust verification for this call'),
      requireCertified: z
        .boolean()
        .optional()
        .describe('Require connector certification before execution'),
      minSafetyScore: z
        .number()
        .int()
        .min(0)
        .max(100)
        .optional()
        .describe('Minimum connector safety score required for execution'),
    },
    permission: 'write',
    policyDomain: 'connectors',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Execute WASM connector', params);
      }
      return executeInstalledConnectorAction({
        connectorId: params.connectorId,
        version: params.version || null,
        action: params.action,
        params: params.params || {},
        context: params.context || {},
        timeoutMs: params.timeoutMs || null,
        verifyStrict: params.verifyStrict,
        requireCertified: params.requireCertified,
        minSafetyScore: params.minSafetyScore,
      });
    },
  },
];

export default connectorTools;
