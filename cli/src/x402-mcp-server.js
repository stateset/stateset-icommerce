/**
 * MCP Server for x402 Paid API calls
 */

import fs from 'node:fs';
import path from 'node:path';
import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';
import { X402SequencerClient, createX402Agent, BudgetExceededError } from './x402/index.js';
import { createBudgetState, getDefaultBudgetStateFile } from './x402/budget.js';
import { loadX402Config, resolveX402ConfigPath, pickConfigValue } from './x402/config.js';
import { getKeyManager } from './sync/keys.js';
import { PolicyEngine } from './policies/engine.js';

const MAX_RESPONSE_CHARS = 12000;

function parseBool(value, fallback = false) {
  if (value === undefined || value === null || value === '') return fallback;
  if (typeof value === 'boolean') return value;
  return ['1', 'true', 'yes', 'on'].includes(String(value).toLowerCase());
}

function parseNumber(value) {
  if (value === undefined || value === null || value === '') return undefined;
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : undefined;
  }
  const num = Number(value);
  return Number.isFinite(num) ? num : undefined;
}

function parseList(value) {
  if (!value) return [];
  if (Array.isArray(value)) {
    return value.map((item) => String(item).trim()).filter(Boolean);
  }
  return String(value)
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function decodeKeyMaterial(value) {
  if (!value) return null;
  const trimmed = String(value).trim();
  if (/^[0-9a-fA-F]+$/.test(trimmed)) {
    return Buffer.from(trimmed, 'hex');
  }
  return Buffer.from(trimmed, 'base64');
}

function loadKeyFromJson(keyJson) {
  if (!keyJson?.privateKey || !keyJson?.publicKey) {
    throw new Error('Signing key JSON must include privateKey and publicKey');
  }
  return {
    keyId: keyJson.keyId ?? 1,
    privateKey: decodeKeyMaterial(keyJson.privateKey),
    publicKey: decodeKeyMaterial(keyJson.publicKey),
  };
}

async function resolveSigningKey({ agentId, keyId, configDir, keyJson, keyPath }) {
  if (keyJson) return loadKeyFromJson(keyJson);
  if (keyPath) {
    const resolved = path.resolve(keyPath);
    // Prevent path traversal: keyPath must be under cwd or configDir
    const cwd = process.cwd();
    const cfgBase = configDir ? path.resolve(configDir) : null;
    if (
      !resolved.startsWith(cwd + path.sep) &&
      resolved !== cwd &&
      !(cfgBase && resolved.startsWith(cfgBase + path.sep))
    ) {
      throw new Error('keyPath must be within the current working directory or config directory');
    }
    const raw = fs.readFileSync(resolved, 'utf8');
    return loadKeyFromJson(JSON.parse(raw));
  }
  if (!agentId) {
    throw new Error('X402 agentId is required to load signing keys');
  }
  const manager = getKeyManager(configDir);
  if (keyId) {
    const key = await manager.getSigningKey(agentId, Number(keyId));
    if (key) return key;
  }
  const current = await manager.getCurrentSigningKey(agentId);
  if (!current) {
    throw new Error(`No signing keys found for agent ${agentId}`);
  }
  return current;
}

function truncateBody(body) {
  if (typeof body !== 'string') return body;
  if (body.length <= MAX_RESPONSE_CHARS) return body;
  return `${body.slice(0, MAX_RESPONSE_CHARS)}\n... truncated ...`;
}

function result(data) {
  return { content: [{ type: 'text', text: JSON.stringify(data, null, 2) }] };
}

function errorResult(error) {
  return {
    content: [{ type: 'text', text: JSON.stringify({ success: false, error }, null, 2) }],
    isError: true,
  };
}

export function createX402McpServer({
  env = process.env,
  configDir = '.stateset',
  policyEngine = null,
  policyStorePath = null,
} = {}) {
  const configPath = resolveX402ConfigPath({ env, configDir });
  const fileConfig = loadX402Config(configPath);

  const sequencerUrl =
    pickConfigValue(env, fileConfig, 'X402_SEQUENCER_URL', 'sequencerUrl', 'sequencer_url') ||
    pickConfigValue(env, fileConfig, 'X402_SEQUENCER', 'sequencer');
  const tenantId = pickConfigValue(env, fileConfig, 'X402_TENANT_ID', 'tenantId', 'tenant_id');
  const storeId = pickConfigValue(env, fileConfig, 'X402_STORE_ID', 'storeId', 'store_id');
  const agentId = pickConfigValue(env, fileConfig, 'X402_AGENT_ID', 'agentId', 'agent_id');
  const payerAddress =
    pickConfigValue(env, fileConfig, 'X402_PAYER_ADDRESS', 'payerAddress', 'payer_address') ||
    pickConfigValue(env, fileConfig, 'X402_WALLET_ADDRESS', 'walletAddress', 'wallet_address');
  const agentKeyId = parseNumber(
    pickConfigValue(env, fileConfig, 'X402_AGENT_KEY_ID', 'agentKeyId', 'agent_key_id'),
  );
  const preferredNetworks = parseList(
    pickConfigValue(
      env,
      fileConfig,
      'X402_PREFERRED_NETWORKS',
      'preferredNetworks',
      'preferred_networks',
    ),
  );
  const requireReceipt = parseBool(
    pickConfigValue(env, fileConfig, 'X402_REQUIRE_RECEIPT', 'requireReceipt', 'require_receipt'),
    false,
  );
  const receiptTimeoutMs = parseNumber(
    pickConfigValue(
      env,
      fileConfig,
      'X402_RECEIPT_TIMEOUT_MS',
      'receiptTimeoutMs',
      'receipt_timeout_ms',
    ),
  );
  const receiptPollMs = parseNumber(
    pickConfigValue(env, fileConfig, 'X402_RECEIPT_POLL_MS', 'receiptPollMs', 'receipt_poll_ms'),
  );
  const maxAmount = parseNumber(
    pickConfigValue(env, fileConfig, 'X402_MAX_AMOUNT', 'maxAmount', 'max_amount'),
  );
  const maxAmountPerCall = parseNumber(
    pickConfigValue(
      env,
      fileConfig,
      'X402_BUDGET_PER_CALL',
      'maxAmountPerCall',
      'budgetPerCall',
      'budget_per_call',
    ),
  );
  const dailyBudget = parseNumber(
    pickConfigValue(
      env,
      fileConfig,
      'X402_BUDGET_DAILY',
      'dailyBudget',
      'budgetDaily',
      'budget_daily',
    ),
  );
  const startingBalance = parseNumber(
    pickConfigValue(
      env,
      fileConfig,
      'X402_STARTING_BALANCE',
      'startingBalance',
      'starting_balance',
    ),
  );
  const budgetStateFile =
    pickConfigValue(
      env,
      fileConfig,
      'X402_BUDGET_STATE_FILE',
      'budgetStateFile',
      'budget_state_file',
    ) || getDefaultBudgetStateFile();

  const shouldTrackBudget = Boolean(
    maxAmountPerCall !== undefined ||
    dailyBudget !== undefined ||
    startingBalance !== undefined ||
    budgetStateFile,
  );

  const budgetState = shouldTrackBudget
    ? createBudgetState({ filePath: budgetStateFile, startingBalance })
    : null;

  const sequencerClient = sequencerUrl
    ? new X402SequencerClient({
        sequencerUrl,
        auth: {
          apiKey: pickConfigValue(env, fileConfig, 'X402_API_KEY', 'apiKey', 'api_key'),
          jwt: pickConfigValue(env, fileConfig, 'X402_JWT', 'jwt'),
        },
      })
    : null;

  let cachedSigningKey = null;
  const resolveKeyOnce = async () => {
    if (cachedSigningKey) return cachedSigningKey;
    const keyPath = pickConfigValue(
      env,
      fileConfig,
      'X402_SIGNING_KEY_PATH',
      'signingKeyPath',
      'signing_key_path',
    );
    const rawKey = pickConfigValue(
      env,
      fileConfig,
      'X402_SIGNING_KEY',
      'signingKey',
      'signing_key',
    );
    const keyJson = rawKey ? (typeof rawKey === 'string' ? JSON.parse(rawKey) : rawKey) : null;
    cachedSigningKey = await resolveSigningKey({
      agentId,
      keyId: agentKeyId,
      configDir,
      keyJson,
      keyPath,
    });
    return cachedSigningKey;
  };

  const ensureConfig = async () => {
    if (!sequencerClient) throw new Error('X402_SEQUENCER_URL is required');
    if (!tenantId || !storeId || !agentId) {
      throw new Error('X402_TENANT_ID, X402_STORE_ID, and X402_AGENT_ID are required');
    }
    if (!payerAddress) {
      throw new Error('X402_PAYER_ADDRESS is required');
    }
    const signingKey = await resolveKeyOnce();
    return {
      sequencerClient,
      tenantId,
      storeId,
      agentId,
      agentKeyId: agentKeyId ?? signingKey?.keyId,
      payerAddress,
      signingKey,
      preferredNetworks,
      requireReceipt,
      receiptTimeoutMs,
      receiptPollMs,
      maxAmount,
      maxAmountPerCall,
      dailyBudget,
      budgetState,
    };
  };

  const applyPolicyTransform = (input, transform) => {
    if (!transform || typeof transform !== 'object' || Array.isArray(transform)) {
      return input;
    }

    const output = { ...(input || {}) };
    for (const [key, value] of Object.entries(transform)) {
      if (
        output[key] !== null &&
        output[key] !== undefined &&
        typeof output[key] === 'object' &&
        !Array.isArray(output[key]) &&
        value &&
        typeof value === 'object' &&
        !Array.isArray(value)
      ) {
        output[key] = { ...output[key], ...value };
      } else {
        output[key] = value;
      }
    }

    return output;
  };

  const resolvePolicyStorePath =
    policyStorePath || process.env.STATESET_POLICY_DIR || path.resolve(configDir || '.stateset');
  const policyEngineInstance =
    policyEngine ||
    (resolvePolicyStorePath
      ? new PolicyEngine({
          storePath: resolvePolicyStorePath,
        })
      : null);
  const policyLoad =
    policyEngineInstance && !policyEngine
      ? policyEngineInstance.load().catch(() => null)
      : Promise.resolve();

  const evaluatePolicy = async (toolName, params, extra = {}) => {
    if (!policyEngineInstance) return { allowed: true, params };

    await policyLoad;

    const context = {
      domain: 'x402',
      tool: toolName,
      params,
      requestId: extra?.requestId || null,
      sessionId: extra?.sessionId || null,
    };

    let result;
    try {
      result = await policyEngineInstance.evaluate('x402', context);
    } catch (err) {
      console.debug(
        '[x402-mcp-server] Policy evaluation failed, allowing by default:',
        err.message || err,
      );
      return { allowed: true, params };
    }

    const actions = Array.isArray(result?.actions) ? result.actions : [];
    let transformedParams = params;
    for (const action of actions) {
      if (action?.type === 'transform') {
        transformedParams = applyPolicyTransform(transformedParams, action.transform);
      }
    }

    if (result?.shouldDeny) {
      const reason = actions
        .filter((action) => action?.type === 'deny')
        .map((action) => action?.reason || action?.metadata?.reason || 'Tool denied by policy')
        .filter(Boolean)
        .join('; ');

      return {
        allowed: false,
        params: transformedParams,
        reason: reason || 'Tool denied by policy',
        actions,
        evaluation: result,
      };
    }

    return {
      allowed: true,
      params: transformedParams,
      actions,
      evaluation: result,
    };
  };

  const policyError = (toolName, policy) => ({
    content: [
      {
        type: 'text',
        text: JSON.stringify(
          {
            error: policy.reason || 'Tool execution blocked by policy',
            tool: toolName,
            policy: {
              domain: 'x402',
              actions: policy.actions || [],
              evaluation: policy.evaluation || null,
            },
          },
          null,
          2,
        ),
      },
    ],
    isError: true,
  });

  const withPolicy = (toolName, handler) => async (args, extra) => {
    const policy = await evaluatePolicy(toolName, args, extra);
    if (!policy.allowed) {
      return policyError(toolName, policy);
    }
    return handler(policy.params, extra);
  };

  return createSdkMcpServer({
    name: 'stateset-x402',
    version: '1.0.0',
    tools: [
      tool(
        'x402_call',
        'Pay and call a URL with automatic x402 handling.',
        {
          url: z.string().describe('URL to call (x402-protected endpoint)'),
          method: z.string().optional().describe('HTTP method (default: GET)'),
          headers: z.record(z.string()).optional().describe('Request headers'),
          body: z.any().optional().describe('Request body (object or string)'),
          maxAmount: z.number().optional().describe('Override max amount for this call'),
          requireReceipt: z.boolean().optional().describe('Wait for payment receipt'),
        },
        withPolicy(
          'x402_call',
          async ({
            url,
            method = 'GET',
            headers = {},
            body,
            maxAmount: perCallMax,
            requireReceipt: perCallReceipt,
          }) => {
            try {
              const config = await ensureConfig();
              const agent = createX402Agent({
                ...config,
                maxAmount: perCallMax ?? config.maxAmount,
                requireReceipt: perCallReceipt ?? config.requireReceipt,
              });

              const requestHeaders = { ...headers };
              let requestBody = body;
              if (body && typeof body === 'object' && !(body instanceof Buffer)) {
                requestBody = JSON.stringify(body);
                if (!requestHeaders['content-type']) {
                  requestHeaders['content-type'] = 'application/json';
                }
              }

              const response = await agent.fetch(url, {
                method,
                headers: requestHeaders,
                body: requestBody,
              });

              const contentType = response.headers.get('content-type') || '';
              let parsedBody = null;
              if (contentType.includes('application/json')) {
                parsedBody = await response.json();
              } else {
                parsedBody = await response.text();
              }

              return result({
                success: response.ok,
                status: response.status,
                statusText: response.statusText,
                url: response.url,
                contentType,
                headers: Object.fromEntries(response.headers.entries()),
                body: truncateBody(parsedBody),
                budget: agent.budget
                  ? {
                      spentToday: agent.budget.getSpentToday(),
                      dailyBudget,
                      balance: agent.budget.getBalance(),
                    }
                  : null,
              });
            } catch (error) {
              const message =
                error instanceof BudgetExceededError
                  ? `Budget exceeded: ${error.message}`
                  : error.message;
              return errorResult(message);
            }
          },
        ),
      ),

      tool(
        'x402_budget_status',
        'Show remaining daily/per-call budget and local balance tracking.',
        {},
        withPolicy('x402_budget_status', () => {
          if (!budgetState) {
            return result({
              success: true,
              budget: null,
              message: 'No budget tracking configured.',
            });
          }
          return result({
            success: true,
            budget: {
              spentToday: budgetState.getSpentToday(),
              dailyBudget,
              perCallLimit: maxAmountPerCall,
              balance: budgetState.getBalance(),
              stateFile: budgetState.filePath,
            },
          });
        }),
      ),

      tool(
        'x402_history',
        'List recent x402 payments recorded locally.',
        {
          limit: z.number().optional().describe('Number of entries to return (default: 50)'),
        },
        withPolicy('x402_history', ({ limit = 50 }) => {
          if (!budgetState) {
            return result({
              success: true,
              history: [],
              message: 'No budget tracking configured.',
            });
          }
          return result({
            success: true,
            count: budgetState.listHistory(limit).length,
            history: budgetState.listHistory(limit),
          });
        }),
      ),

      tool(
        'x402_receipt',
        'Fetch a receipt for a payment intent from the sequencer.',
        {
          intentId: z.string().describe('Payment intent ID'),
        },
        withPolicy('x402_receipt', async ({ intentId }) => {
          try {
            if (!sequencerClient) throw new Error('X402_SEQUENCER_URL is required');
            const receipt = await sequencerClient.getPaymentReceipt(intentId);
            return result({ success: true, receipt });
          } catch (error) {
            return errorResult(error.message);
          }
        }),
      ),

      tool(
        'x402_balance',
        'Check wallet balance for the payer address on a chain (optional).',
        {
          chain: z
            .string()
            .optional()
            .describe('Blockchain: solana, set_chain, base, ethereum, arbitrum'),
          token: z.string().optional().describe('Token symbol (optional)'),
          address: z
            .string()
            .optional()
            .describe('Wallet address (defaults to X402_PAYER_ADDRESS)'),
        },
        withPolicy('x402_balance', async ({ chain, token, address }) => {
          if (!chain) {
            return result({
              success: true,
              balance: null,
              message: 'Chain not provided. Use x402_budget_status for local budget tracking.',
            });
          }
          try {
            const { getBalance } = await import('./chains/index.js');
            const walletAddress = address || payerAddress;
            if (!walletAddress) {
              throw new Error(
                'Wallet address is required (set X402_PAYER_ADDRESS or pass address)',
              );
            }
            const balance = await getBalance(walletAddress, chain, token);
            return result({
              success: true,
              chain,
              address: walletAddress,
              balance: balance.balance,
              symbol: balance.symbol,
            });
          } catch (error) {
            return errorResult(error.message);
          }
        }),
      ),
    ],
  });
}

export const X402_MCP_TOOL_NAMES = [
  'x402_call',
  'x402_budget_status',
  'x402_history',
  'x402_receipt',
  'x402_balance',
];

export default createX402McpServer;
