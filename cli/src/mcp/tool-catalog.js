// Tool catalog / discovery / runtime-contract builders for the MCP orchestrator.
//
// Everything here is a *read-only projection* of the tool registry:
//   - `buildPaymentDiscovery`: MPP discovery document (json or openapi)
//   - `buildToolCatalog`: tool list in generic / openai / mcp formats with
//     pricing + payment info attached
//   - `getToolDiscoveryEngine`: lazily-built semantic discovery engine
//   - `getAgenticRuntimeContract`: the hashed runtime contract document
//   - `getToolDefinitions` / `getRawToolDefinitions`: server.getToolDefinitions
//     and server.getRawToolDefinitions bodies
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { ToolDiscoveryEngine } from '../mcp-tool-discovery.js';
import {
  MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  buildPaymentInfoFromPricing,
  createPaymentDiscoveryDocument,
  listPaymentMethodAdapters,
} from '../mpp/index.js';
import { inputSchemaDefToJsonSchema } from '../tool-schema.js';
import { replayEventHash } from './audit-envelope.js';
import { normalizeToolName } from './policy-helpers.js';
import { stableStringify } from './replay-sanitizer.js';

/**
 * Build the catalog helpers for one server instance.
 *
 * @param {{
 *   allToolDefs: Array<object>,
 *   toolDomainByName: Record<string, string>,
 *   serviceInfo: object,
 *   resultSchemaVersion: string,
 *   getAgenticToolPricing: (toolName: string) => Promise<object | null>,
 *   getToolRuntimeMeta: (toolName: string) => object,
 *   inferPolicyDomain: (toolName: string) => string,
 * }} deps
 * @returns {{
 *   buildPaymentDiscovery: (opts?: {format?: string, tool?: string | null, pricedOnly?: boolean}) => Promise<object>,
 *   buildToolCatalog: (opts?: {format?: string, mcpPrefix?: string | null, tool?: string | null, payableOnly?: boolean}) => Promise<object>,
 *   getToolDiscoveryEngine: () => Promise<ToolDiscoveryEngine>,
 *   getAgenticRuntimeContract: (opts?: {tool?: string, includeLegacyDefaults?: boolean}) => Promise<object>,
 *   getToolDefinitions: (opts?: {format?: string, mcpPrefix?: string | null}) => Array<object>,
 *   getRawToolDefinitions: () => Array<object>,
 * }}
 */
export function createToolCatalogHelpers({
  allToolDefs: ALL_TOOL_DEFS,
  toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
  serviceInfo: MPP_SERVICE_INFO,
  resultSchemaVersion: AGENTIC_TOOL_RESULT_SCHEMA_VERSION,
  getAgenticToolPricing,
  getToolRuntimeMeta,
  inferPolicyDomain,
}) {
  const buildPaymentDiscovery = async ({
    format = 'json',
    tool = null,
    pricedOnly = false,
  } = {}) => {
    const normalizedTool = normalizeToolName(tool || '');
    const tools = [];

    for (const toolDef of ALL_TOOL_DEFS) {
      if (normalizedTool && toolDef.name !== normalizedTool) continue;
      const pricing = await getAgenticToolPricing(toolDef.name);
      if (pricedOnly && !pricing) continue;
      tools.push({
        name: toolDef.name,
        description: toolDef.description,
        inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}),
        runtime: getToolRuntimeMeta(toolDef.name),
        pricing,
        paymentInfo: buildPaymentInfoFromPricing({
          toolName: toolDef.name,
          description: toolDef.description,
          pricing,
        }),
      });
    }

    if (format === 'openapi') {
      return createPaymentDiscoveryDocument({
        serviceInfo: MPP_SERVICE_INFO,
        tools,
        serverUrl: '/mcp',
      });
    }

    return {
      protocol: 'mpp',
      protocolVersion: MPP_SERVICE_INFO.protocolVersion,
      service: MPP_SERVICE_INFO,
      tools: tools.map((entry) => ({
        name: entry.name,
        description: entry.description,
        runtime: entry.runtime,
        pricing: entry.pricing,
        paymentInfo: entry.paymentInfo,
      })),
    };
  };

  const buildToolCatalog = async ({
    format = 'generic',
    mcpPrefix = null,
    tool = null,
    payableOnly = false,
  } = {}) => {
    const normalizedTool = normalizeToolName(tool || '');
    const normalizedFormat = format || 'generic';
    const tools = [];

    for (const toolDef of ALL_TOOL_DEFS) {
      if (normalizedTool && toolDef.name !== normalizedTool) continue;
      const runtime = getToolRuntimeMeta(toolDef.name);
      const parameters = inputSchemaDefToJsonSchema(toolDef.inputSchema || {});
      const pricing = await getAgenticToolPricing(toolDef.name);
      const paymentInfo = buildPaymentInfoFromPricing({
        toolName: toolDef.name,
        description: toolDef.description,
        pricing,
      });
      const payable = Boolean(paymentInfo);
      if (payableOnly && !payable) continue;

      const resolvedName = mcpPrefix ? `${mcpPrefix}${toolDef.name}` : toolDef.name;
      if (normalizedFormat === 'openai') {
        tools.push({
          type: 'function',
          function: {
            name: toolDef.name,
            description: toolDef.description,
            parameters,
          },
          stateset: {
            permission: toolDef.permission || runtime.permission,
            policyDomain: runtime.policyDomain,
            payable,
            payment: paymentInfo,
          },
        });
        continue;
      }

      tools.push({
        name: normalizedFormat === 'mcp' ? `mcp__stateset-commerce__${toolDef.name}` : resolvedName,
        toolName: toolDef.name,
        description: toolDef.description,
        inputSchema: parameters,
        permission: toolDef.permission || runtime.permission,
        policyDomain: runtime.policyDomain,
        runtime,
        payable,
        paymentInfo,
      });
    }

    return {
      format: normalizedFormat,
      service: MPP_SERVICE_INFO,
      count: tools.length,
      tools,
    };
  };

  let toolDiscoveryEnginePromise = null;
  const getToolDiscoveryEngine = async () => {
    if (toolDiscoveryEnginePromise) return toolDiscoveryEnginePromise;
    toolDiscoveryEnginePromise = (async () => {
      const engine = new ToolDiscoveryEngine();
      const catalog = await buildToolCatalog({ format: 'generic', payableOnly: false });
      for (const tool of catalog.tools) {
        engine.registerTool(tool.toolName || tool.name, {
          name: tool.toolName || tool.name,
          description: tool.description,
          category: tool.policyDomain || 'general',
          purpose: tool.description,
          whenToUse: tool.description,
          inputSchema: tool.inputSchema,
          permission: tool.permission,
          payable: tool.payable || false,
          paymentInfo: tool.paymentInfo || null,
        });
      }
      return engine;
    })();
    return toolDiscoveryEnginePromise;
  };

  const getAgenticRuntimeContract = async ({ tool, includeLegacyDefaults = false } = {}) => {
    const targetTool = tool ? normalizeToolName(tool) : null;
    const normalizedTools = await Promise.all(
      ALL_TOOL_DEFS.filter((candidate) => !targetTool || candidate?.name === targetTool)
        .sort((a, b) => a.name.localeCompare(b.name))
        .map(async (candidate) => {
          const meta = getToolRuntimeMeta(candidate?.name);
          const pricing = await getAgenticToolPricing(candidate?.name);
          return {
            ...meta,
            pricing: pricing
              ? {
                  enabled: pricing.enabled,
                  chainId: pricing.chainId,
                  tokenSymbol: pricing.tokenSymbol,
                  amount: pricing.amount,
                  amountSmallest: pricing.amountSmallest,
                }
              : null,
          };
        }),
    );

    const includeLegacy = includeLegacyDefaults
      ? ['create', 'read', 'update', 'delete', 'list']
      : [];
    const contract = {
      engine: 'stateset-icommerce',
      agenticToolResultSchema: {
        version: AGENTIC_TOOL_RESULT_SCHEMA_VERSION,
        envelope: 'mcp_tool_result',
        metadata: [
          'schemaVersion',
          'status',
          'tool',
          'requestId',
          'sessionId',
          'policy',
          'permission',
          'charge',
          'mutation',
          'timing',
        ],
      },
      mpp: {
        enabled: true,
        service: MPP_SERVICE_INFO,
        transport: {
          jsonrpc: {
            paymentRequiredCode: MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
            paymentRequiredMessage: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
            credentialMetaKey: 'payment',
            receiptMetaKey: 'payment',
          },
          http: {
            paymentRequiredStatus: 402,
            discoveryExtensions: ['x-payment-info', 'x-service-info'],
          },
        },
        intents: ['charge', 'session'],
        methodAdapters: listPaymentMethodAdapters(),
      },
      purpose: 'agentic_runtime_contract',
      generatedAt: new Date().toISOString(),
      includeLegacyDefaults,
      legacyDefaults: includeLegacy,
      totalTools: normalizedTools.length,
      tools: normalizedTools,
    };
    if (includeLegacy) {
      contract.legacy = {
        deprecatedPrefixes: includeLegacy,
      };
    }
    contract.contractHash = replayEventHash(stableStringify({ tools: contract.tools }));
    return contract;
  };

  const getToolDefinitions = ({ format = 'generic', mcpPrefix = null } = {}) => {
    return ALL_TOOL_DEFS.map((toolDef) => {
      const runtime = getToolRuntimeMeta(toolDef.name);
      const parameters = inputSchemaDefToJsonSchema(toolDef.inputSchema || {});
      const baseName = toolDef.name;
      const resolvedName = mcpPrefix ? `${mcpPrefix}${baseName}` : baseName;
      const descriptor = {
        name: resolvedName,
        toolName: baseName,
        description: toolDef.description,
        inputSchema: parameters,
        permission: toolDef.permission || runtime.permission,
        policyDomain: runtime.policyDomain,
        runtime,
      };

      if (format === 'openai') {
        return {
          type: 'function',
          function: {
            name: baseName,
            description: toolDef.description,
            parameters,
          },
          stateset: {
            permission: descriptor.permission,
            policyDomain: descriptor.policyDomain,
          },
        };
      }

      if (format === 'anthropic') {
        return {
          name: baseName,
          description: toolDef.description,
          input_schema: parameters,
          stateset: {
            permission: descriptor.permission,
            policyDomain: descriptor.policyDomain,
          },
        };
      }

      if (format === 'mcp') {
        return {
          ...descriptor,
          name: `mcp__stateset-commerce__${baseName}`,
        };
      }

      return descriptor;
    });
  };

  const getRawToolDefinitions = () => {
    return ALL_TOOL_DEFS.map((toolDef) => ({
      name: toolDef.name,
      description: toolDef.description,
      inputSchema: toolDef.inputSchema || {},
      permission: toolDef.permission || 'unknown',
      policyDomain:
        toolDef.policyDomain ||
        TOOL_DOMAIN_BY_TOOL_NAME[toolDef.name] ||
        inferPolicyDomain(toolDef.name),
      runtime: getToolRuntimeMeta(toolDef.name),
    }));
  };

  return {
    buildPaymentDiscovery,
    buildToolCatalog,
    getToolDiscoveryEngine,
    getAgenticRuntimeContract,
    getToolDefinitions,
    getRawToolDefinitions,
  };
}
