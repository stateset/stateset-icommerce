/**
 * MCP Server for StateSet Commerce operations
 *
 * Thin orchestrator that loads tools from domain modules and wraps them
 * with permission checks, telemetry, treasury charging, and error handling.
 */

import { createSdkMcpServer, tool as sdkTool } from '@anthropic-ai/claude-agent-sdk';
import { getSharedRuntime } from './channels/plugin-runtime.js';
import { A2AStore } from './a2a/store.js';

// Domain tool modules
import { customerTools } from './tools/customers.js';
import { orderTools } from './tools/orders.js';
import { vectorTools } from './tools/vector.js';
import { productTools } from './tools/products.js';
import { inventoryTools } from './tools/inventory.js';
import { customObjectTools } from './tools/custom-objects.js';
import { returnTools } from './tools/returns.js';
import { cartTools } from './tools/carts.js';
import { analyticsTools } from './tools/analytics.js';
import { currencyTools } from './tools/currency.js';
import { taxTools } from './tools/tax.js';
import { promotionTools } from './tools/promotions.js';
import { subscriptionTools } from './tools/subscriptions.js';
import { syncTools } from './tools/sync.js';
import { manufacturingTools } from './tools/manufacturing.js';
import { paymentTools } from './tools/payments.js';
import { stablecoinTools } from './tools/stablecoin.js';
import { treasuryTools } from './tools/treasury.js';
import { erc8004Tools } from './tools/erc8004.js';
import { x402Tools } from './tools/x402.js';
import { agentCardTools } from './tools/agent-cards.js';
import { a2aTools } from './tools/a2a.js';
import { shipmentTools } from './tools/shipments.js';
import { supplierTools } from './tools/suppliers.js';
import { invoiceTools } from './tools/invoices.js';
import { warrantyTools } from './tools/warranties.js';

/**
 * All domain tool definitions, collected from modules.
 */
const ALL_TOOL_DEFS = [
  ...customerTools,
  ...orderTools,
  ...productTools,
  ...inventoryTools,
  ...customObjectTools,
  ...returnTools,
  ...cartTools,
  ...analyticsTools,
  ...currencyTools,
  ...taxTools,
  ...promotionTools,
  ...subscriptionTools,
  ...syncTools,
  ...manufacturingTools,
  ...paymentTools,
  ...stablecoinTools,
  ...treasuryTools,
  ...erc8004Tools,
  ...x402Tools,
  ...agentCardTools,
  ...a2aTools,
  ...shipmentTools,
  ...supplierTools,
  ...invoiceTools,
  ...warrantyTools,
  ...vectorTools,
];

/**
 * Set of read-only tool names, derived from module permission metadata.
 */
const READ_ONLY_TOOLS = new Set(
  ALL_TOOL_DEFS.filter((t) => t.permission === 'read').map((t) => t.name),
);

/**
 * Auto-index a newly created entity if vectorAutoIndex is enabled.
 * Runs in the background — failures are logged but do not block the response.
 * @param {'product'|'customer'|'order'} entityType
 * @param {Object} entity - The created entity (must have .id)
 */
function autoIndexEntity(entityType, entity) {
  const vectorAutoIndex = getSharedRuntime()?.vectorAutoIndex;
  if (!vectorAutoIndex || !entity?.id) return;
  const indexFn = {
    product: () => vectorAutoIndex.indexProduct(entity.id.toString()),
    customer: () => vectorAutoIndex.indexCustomer(entity.id.toString()),
    order: () => vectorAutoIndex.indexOrder(entity.id.toString()),
  }[entityType];
  if (indexFn) {
    indexFn().catch((err) =>
      console.error(`[AutoIndex] Failed to index ${entityType} ${entity.id}: ${err.message}`),
    );
  }
}

/**
 * Create the StateSet Commerce MCP server
 * @param {Object} options
 * @param {import('@stateset/embedded').Commerce} options.commerce - Commerce instance
 * @param {boolean} options.allowApply - Whether to allow destructive operations
 * @param {import('./telemetry.js').AgentTelemetry} options.telemetry - Telemetry instance
 * @param {import('./permissions.js').PermissionGate} options.permissionGate - Permission gate instance
 * @param {import('./channels/plugin-api.js').HookRunner} options.hookRunner - Hook runner instance
 * @param {string} options.dbPath - Commerce database path (used for ERC-8004 lookups)
 * @param {Object} options.treasury - Treasury configuration (agentId, dbPath, ERC-8004 registry)
 * @param {Object} options.agentConfig - Agent configuration for A2A payments
 * @param {string} options.agentConfig.agentId - This agent's ID
 * @param {string} options.agentConfig.walletAddress - This agent's wallet address
 * @param {Object} options.agentConfig.signingKey - Ed25519 signing key { privateKey, publicKey }
 */
export function createStatesetMcpServer({
  commerce,
  allowApply = false,
  telemetry = null,
  permissionGate = null,
  hookRunner = null,
  dbPath = './store.db',
  treasury = null,
  agentConfig = null,
}) {
  // ---------------------------------------------------------------------------
  // A2A Store initialization
  // ---------------------------------------------------------------------------
  const a2aStore = new A2AStore({ dbPath: dbPath.replace('.db', '-a2a.db') });

  // Create a commerce wrapper that includes A2A methods
  const commerceWithA2A = {
    ...commerce,
    a2a: () => ({
      createPayment: (p) => a2aStore.createPayment(p),
      getPayment: (id) => a2aStore.getPayment(id),
      updatePayment: (id, u) => a2aStore.updatePayment(id, u),
      listPayments: (f) => a2aStore.listPayments(f),
      sumPayments: (f) => a2aStore.sumPayments(f),
      createPaymentRequest: (r) => a2aStore.createPaymentRequest(r),
      getPaymentRequest: (id) => a2aStore.getPaymentRequest(id),
      updatePaymentRequest: (id, u) => a2aStore.updatePaymentRequest(id, u),
      listPaymentRequests: (f) => a2aStore.listPaymentRequests(f),
      createQuote: (q) => a2aStore.createQuote(q),
      getQuote: (id) => a2aStore.getQuote(id),
      updateQuote: (id, u) => a2aStore.updateQuote(id, u),
      listQuotes: (f) => a2aStore.listQuotes(f),
    }),
  };
  // ---------------------------------------------------------------------------
  // Permission helpers
  // ---------------------------------------------------------------------------

  const isReadOnly = (toolName) => READ_ONLY_TOOLS.has(toolName);

  const checkPermission = async (toolName, params) => {
    if (permissionGate) {
      const result = await permissionGate.checkPermission(toolName, params);
      if (telemetry) {
        telemetry.logCustomEvent('permission_decision', {
          tool: toolName,
          allowed: result.allowed,
          preview: result.preview || false,
          reason: result.reason || null,
        });
      }
      return result;
    }
    if (allowApply || isReadOnly(toolName)) {
      if (telemetry) {
        telemetry.logCustomEvent('permission_decision', {
          tool: toolName,
          allowed: true,
          preview: false,
        });
      }
      return { allowed: true };
    }
    const result = {
      allowed: false,
      preview: true,
      reason: `Preview mode: would execute '${toolName}' if --apply flag is set`,
      wouldDo: { tool: toolName, params },
    };
    if (telemetry) {
      telemetry.logCustomEvent('permission_decision', {
        tool: toolName,
        allowed: false,
        preview: true,
        reason: result.reason,
      });
    }
    return result;
  };

  // ---------------------------------------------------------------------------
  // Treasury helpers
  // ---------------------------------------------------------------------------

  const treasuryAgentId = treasury?.agentId || process.env.TREASURY_AGENT || 'default';
  const treasuryDbPath = treasury?.dbPath || process.env.TREASURY_DB || null;
  const treasuryContextOptions = treasuryDbPath ? { dbPath: treasuryDbPath } : {};
  const treasuryRegistry =
    treasury?.erc8004Registry || process.env.TREASURY_ERC8004_REGISTRY || null;
  const treasuryIdentityDbPath = treasury?.erc8004DbPath || dbPath;
  let treasuryIdentityLoaded = false;
  let treasuryIdentityCache = null;

  const resolveTreasuryIdentity = async () => {
    if (!treasuryRegistry) return null;
    if (treasuryIdentityLoaded) return treasuryIdentityCache;
    treasuryIdentityLoaded = true;
    try {
      const { getIdentity } = await import('./erc8004/index.js');
      treasuryIdentityCache = getIdentity(
        treasuryIdentityDbPath,
        treasuryRegistry,
        treasuryAgentId,
      );
    } catch {
      treasuryIdentityCache = null;
    }
    if (!treasuryIdentityCache) {
      throw new Error(`ERC-8004 identity not found for ${treasuryRegistry}:${treasuryAgentId}`);
    }
    return treasuryIdentityCache;
  };

  const resolveTreasuryAgentId = async () => {
    const identity = await resolveTreasuryIdentity();
    return identity?.agent_id || treasuryAgentId;
  };

  const buildTreasuryIdentityMetadata = async () => {
    const identity = await resolveTreasuryIdentity();
    if (!identity) return {};
    return {
      erc8004: {
        registry: treasuryRegistry,
        agentId: identity.agent_id,
        wallet: identity.agent_wallet,
        owner: identity.owner_address,
      },
    };
  };

  // ---------------------------------------------------------------------------
  // Telemetry & audit helpers
  // ---------------------------------------------------------------------------

  const wrapWithTelemetry = (toolName, fn) => {
    return async (params, extra) => {
      const startTime = Date.now();
      try {
        const result = await fn(params, extra);
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, result, duration);
        }
        return result;
      } catch (error) {
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, { error: error.message }, duration);
        }
        throw error;
      }
    };
  };

  const buildAuditContext = (extra, toolName) => ({
    taskId: extra?.requestId || null,
    requestId: extra?.requestId || null,
    sessionId: extra?.sessionId || null,
    toolName,
  });

  const maybeChargeForTool = async (toolName, extra) => {
    try {
      const { loadTreasuryContext, getToolPricing, resolveToken, recordFee } =
        await import('./treasury/index.js');
      const { toSmallestUnit } = await import('./chains/config.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions);
      const rule = getToolPricing(ctx.pricing, toolName);
      if (!rule) return { charged: false };

      if (!allowApply) {
        return {
          charged: false,
          blocked: true,
          reason: `Tool ${toolName} requires a treasury charge. Re-run with --apply.`,
        };
      }

      const token = resolveToken(rule.chainId, rule.tokenSymbol, ctx.registry);
      if (!token) {
        return {
          charged: false,
          blocked: true,
          reason: `Unknown token ${rule.tokenSymbol} on ${rule.chainId}.`,
        };
      }
      const amount = Number(rule.amount);
      if (!Number.isFinite(amount) || amount <= 0) {
        return {
          charged: false,
          blocked: true,
          reason: `Invalid pricing amount for ${toolName}.`,
        };
      }
      const effectiveAgentId = await resolveTreasuryAgentId();
      const identityMeta = await buildTreasuryIdentityMetadata();
      const balance = ctx.store.getBalance({
        agentId: effectiveAgentId,
        chainId: rule.chainId,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals,
      });

      const required = toSmallestUnit(amount, token.decimals);

      if (balance.balanceSmallest < required) {
        return {
          charged: false,
          blocked: true,
          reason: `Insufficient ${token.symbol} balance for ${toolName}. Required ${rule.amount} ${token.symbol}.`,
        };
      }

      const audit = buildAuditContext(extra, toolName);
      await recordFee(
        {
          agentId: effectiveAgentId,
          chainId: rule.chainId,
          tokenSymbol: token.symbol,
          amount,
          source: 'task',
          metadata: {
            pricingRule: rule,
            ...identityMeta,
          },
          ...audit,
        },
        ctx,
      );

      return { charged: true, rule };
    } catch (error) {
      return { charged: false, blocked: true, reason: error.message };
    }
  };

  // ---------------------------------------------------------------------------
  // Tool wrapper — adds hooks, permission checks, treasury, and telemetry
  // ---------------------------------------------------------------------------

  const wrapTool = (name, description, schema, handler) => {
    return sdkTool(name, description, schema, async (args, extra) => {
      let nextArgs = args;

      if (hookRunner?.hasHooks?.('before_tool_call')) {
        const hookResult = await hookRunner.run('before_tool_call', {
          tool: name,
          params: nextArgs,
          allowApply,
          requestId: extra?.requestId,
          sessionId: extra?.sessionId,
        });
        if (hookResult?.params) nextArgs = hookResult.params;
        if (hookResult?.blocked || hookResult?.allowed === false) {
          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify({
                  error: hookResult?.reason || 'Tool execution blocked by hook',
                  tool: name,
                }),
              },
            ],
            isError: true,
          };
        }
      }

      const permission = await checkPermission(name, nextArgs);
      if (!permission.allowed) {
        const payload = {
          error: permission.reason || 'Permission denied',
          tool: name,
        };
        if (permission.preview) {
          payload.preview = true;
          if (permission.wouldDo) {
            payload.wouldDo = permission.wouldDo;
          }
        }
        return {
          content: [{ type: 'text', text: JSON.stringify(payload) }],
          isError: true,
        };
      }

      const charge = await maybeChargeForTool(name, extra);
      if (charge?.blocked) {
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify({
                error: charge.reason || 'Treasury charge blocked',
                tool: name,
              }),
            },
          ],
          isError: true,
        };
      }

      const wrapped = wrapWithTelemetry(name, handler);
      try {
        const result = await wrapped(nextArgs, extra);
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            result,
            requestId: extra?.requestId,
            sessionId: extra?.sessionId,
          });
        }
        return result;
      } catch (error) {
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            error: error.message,
            requestId: extra?.requestId,
            sessionId: extra?.sessionId,
          });
        }
        throw error;
      }
    });
  };

  // ---------------------------------------------------------------------------
  // Adapt domain tool modules into MCP-formatted tools
  // ---------------------------------------------------------------------------

  /**
   * Context object passed to every domain tool handler.
   */
  const toolContext = {
    commerce: commerceWithA2A,
    allowApply,
    autoIndexEntity,
    resolveTreasuryAgentId,
    treasuryContextOptions,
    buildAuditContext,
    buildTreasuryIdentityMetadata,
    agentConfig,
  };

  /**
   * Convert a domain tool definition into an SDK-wrapped MCP tool.
   * Bridges the module handler signature `({ commerce, params, ... }) => plainObject`
   * to the MCP format `(args, extra) => { content: [{ type: 'text', ... }] }`.
   */
  const adaptTool = (toolDef) => {
    const { name, description, inputSchema, handler } = toolDef;

    return wrapTool(name, description, inputSchema, async (args, extra) => {
      try {
        const result = await handler({
          ...toolContext,
          params: args,
          extra,
        });
        return {
          content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
        };
      } catch (error) {
        return {
          content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }],
        };
      }
    });
  };

  // ---------------------------------------------------------------------------
  // Build and return the MCP server
  // ---------------------------------------------------------------------------

  return createSdkMcpServer({
    name: 'stateset-commerce',
    version: '1.0.0',
    tools: ALL_TOOL_DEFS.map(adaptTool),
  });
}

/**
 * All MCP tool names in the `mcp__<server>__<tool>` format expected by the harness.
 */
export const TOOL_NAMES = ALL_TOOL_DEFS.map((t) => `mcp__stateset-commerce__${t.name}`);
