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

const MAX_RESPONSE_CHARS = 12000;
const SYNC_KEYS_MODULE = ['.', 'sync', 'keys.js'].join('/');
const POLICY_ENGINE_MODULE = ['.', 'policies', 'engine.js'].join('/');
const CHAINS_MODULE = ['.', 'chains', 'index.js'].join('/');

/**
 * @typedef {Record<string, unknown>} JsonRecord
 * @typedef {{ keyId?: number, privateKey?: string | Buffer, publicKey?: string | Buffer }} SigningKeyJson
 * @typedef {{ keyId?: number, privateKey: Buffer, publicKey: Buffer }} SigningKey
 * @typedef {{
 *   filePath: string,
 *   getSpentToday: () => number,
 *   getBalance: () => number | null,
 *   recordSpend: (amount: number, metadata?: Record<string, unknown>) => void,
 *   listHistory: (limit?: number) => unknown[],
 * }} BudgetState
 * @typedef {{
 *   getSigningKey: (agentId: string, keyId: number) => Promise<SigningKey | null>,
 *   getCurrentSigningKey: (agentId: string) => Promise<SigningKey | null>,
 * }} KeyManagerLike
 * @typedef {{ type?: string, transform?: JsonRecord, reason?: string, metadata?: JsonRecord }} PolicyAction
 * @typedef {{ actions?: PolicyAction[], shouldDeny?: boolean }} PolicyEvaluation
 * @typedef {{ requestId?: string | null, sessionId?: string | null }} PolicyExtra
 * @typedef {{ allowed: boolean, params: JsonRecord, reason?: string, actions?: PolicyAction[], evaluation?: PolicyEvaluation | null }} PolicyDecision
 * @typedef {{
 *   load: () => Promise<unknown>,
 *   evaluate: (domain: string, context: JsonRecord) => Promise<PolicyEvaluation>,
 * }} PolicyEngineLike
 * @typedef {{
 *   sequencerClient: X402SequencerClient | null,
 *   tenantId?: string | null,
 *   storeId?: string | null,
 *   agentId: string,
 *   agentKeyId: number,
 *   payerAddress: string,
 *   signingKey: SigningKey,
 *   preferredNetworks: string[],
 *   requireReceipt: boolean,
 *   receiptTimeoutMs?: number,
 *   receiptPollMs?: number,
 *   maxAmount?: number,
 *   maxAmountPerCall?: number,
 *   dailyBudget?: number,
 *   budgetState: BudgetState | null,
 * }} ResolvedX402Config
 * @typedef {{ url: string, method?: string, headers?: Record<string, string>, body?: unknown, maxAmount?: number, requireReceipt?: boolean }} X402CallArgs
 * @typedef {{ limit?: number }} X402HistoryArgs
 * @typedef {{ intentId: string }} X402ReceiptArgs
 * @typedef {{ chain?: string, token?: string, address?: string }} X402BalanceArgs
 * @typedef {{ env?: NodeJS.ProcessEnv, configDir?: string, policyEngine?: PolicyEngineLike | null, policyStorePath?: string | null }} CreateX402McpServerOptions
 * @typedef {{ agentId?: string | null, keyId?: number | null | undefined, configDir?: string, keyJson?: SigningKeyJson | null, keyPath?: string | null }} ResolveSigningKeyOptions
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {unknown} value
 * @returns {string | null}
 */
function asOptionalString(value) {
  if (value === undefined || value === null || value === '') return null;
  return String(value);
}

/**
 * @returns {Promise<{ getKeyManager: (configDir?: string) => KeyManagerLike }>}
 */
async function loadKeyManagerModule() {
  return /** @type {Promise<{ getKeyManager: (configDir?: string) => KeyManagerLike }>} */ (
    import(SYNC_KEYS_MODULE)
  );
}

/**
 * @returns {Promise<{ PolicyEngine: new (options?: { storePath?: string | null, unknownDomainMode?: 'allow' | 'deny' }) => PolicyEngineLike }>}
 */
async function loadPolicyEngineModule() {
  return /** @type {Promise<{ PolicyEngine: new (options?: { storePath?: string | null, unknownDomainMode?: 'allow' | 'deny' }) => PolicyEngineLike }>} */ (
    import(POLICY_ENGINE_MODULE)
  );
}

/**
 * @returns {Promise<{ getBalance: (address: string, chain: string, token?: string) => Promise<{ balance: unknown, symbol: unknown }> }>}
 */
async function loadChainsModule() {
  return /** @type {Promise<{ getBalance: (address: string, chain: string, token?: string) => Promise<{ balance: unknown, symbol: unknown }> }>} */ (
    import(CHAINS_MODULE)
  );
}

/**
 * @param {unknown} value
 * @param {boolean} [fallback]
 * @returns {boolean}
 */
function parseBool(value, fallback = false) {
  if (value === undefined || value === null || value === '') return fallback;
  if (typeof value === 'boolean') return value;
  return ['1', 'true', 'yes', 'on'].includes(String(value).toLowerCase());
}

/**
 * @param {unknown} value
 * @returns {number | undefined}
 */
function parseNumber(value) {
  if (value === undefined || value === null || value === '') return undefined;
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : undefined;
  }
  const num = Number(value);
  return Number.isFinite(num) ? num : undefined;
}

/**
 * @param {unknown} value
 * @returns {string[]}
 */
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

/**
 * @param {unknown} value
 * @returns {Buffer | null}
 */
function decodeKeyMaterial(value) {
  if (!value) return null;
  const trimmed = String(value).trim();
  if (/^[0-9a-fA-F]+$/.test(trimmed)) {
    return Buffer.from(trimmed, 'hex');
  }
  return Buffer.from(trimmed, 'base64');
}

/**
 * @param {SigningKeyJson | null | undefined} keyJson
 * @returns {SigningKey}
 */
function loadKeyFromJson(keyJson) {
  if (!keyJson?.privateKey || !keyJson?.publicKey) {
    throw new Error('Signing key JSON must include privateKey and publicKey');
  }
  const privateKey = decodeKeyMaterial(keyJson.privateKey);
  const publicKey = decodeKeyMaterial(keyJson.publicKey);
  if (!privateKey || !publicKey) {
    throw new Error('Signing key JSON must include valid privateKey and publicKey');
  }
  return {
    keyId: keyJson.keyId ?? 1,
    privateKey,
    publicKey,
  };
}

/**
 * @param {ResolveSigningKeyOptions} options
 * @returns {Promise<SigningKey>}
 */
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
  const { getKeyManager } = await loadKeyManagerModule();
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

/**
 * @param {unknown} body
 * @returns {unknown}
 */
function truncateBody(body) {
  if (typeof body !== 'string') return body;
  if (body.length <= MAX_RESPONSE_CHARS) return body;
  return `${body.slice(0, MAX_RESPONSE_CHARS)}\n... truncated ...`;
}

/**
 * @param {unknown} data
 * @returns {{ content: Array<{ type: 'text', text: string }> }}
 */
function result(data) {
  return { content: [{ type: 'text', text: JSON.stringify(data, null, 2) }] };
}

/**
 * @param {string} error
 * @returns {{ content: Array<{ type: 'text', text: string }>, isError: true }}
 */
function errorResult(error) {
  return {
    content: [{ type: 'text', text: JSON.stringify({ success: false, error }, null, 2) }],
    isError: true,
  };
}

/**
 * @param {CreateX402McpServerOptions} [options]
 */
export function createX402McpServer({
  env = process.env,
  configDir = '.stateset',
  policyEngine = null,
  policyStorePath = null,
} = {}) {
  const configPath = resolveX402ConfigPath({ env, configDir });
  const fileConfig = loadX402Config(configPath);

  const sequencerUrl =
    asOptionalString(
      pickConfigValue(env, fileConfig, 'X402_SEQUENCER_URL', 'sequencerUrl', 'sequencer_url'),
    ) ?? asOptionalString(pickConfigValue(env, fileConfig, 'X402_SEQUENCER', 'sequencer'));
  const tenantId = asOptionalString(
    pickConfigValue(env, fileConfig, 'X402_TENANT_ID', 'tenantId', 'tenant_id'),
  );
  const storeId = asOptionalString(
    pickConfigValue(env, fileConfig, 'X402_STORE_ID', 'storeId', 'store_id'),
  );
  const agentId = asOptionalString(
    pickConfigValue(env, fileConfig, 'X402_AGENT_ID', 'agentId', 'agent_id'),
  );
  const payerAddress =
    asOptionalString(
      pickConfigValue(env, fileConfig, 'X402_PAYER_ADDRESS', 'payerAddress', 'payer_address'),
    ) ??
    asOptionalString(
      pickConfigValue(env, fileConfig, 'X402_WALLET_ADDRESS', 'walletAddress', 'wallet_address'),
    );
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
    asOptionalString(
      pickConfigValue(
        env,
        fileConfig,
        'X402_BUDGET_STATE_FILE',
        'budgetStateFile',
        'budget_state_file',
      ),
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
          apiKey: asOptionalString(
            pickConfigValue(env, fileConfig, 'X402_API_KEY', 'apiKey', 'api_key'),
          ),
          jwt: asOptionalString(pickConfigValue(env, fileConfig, 'X402_JWT', 'jwt')),
        },
      })
    : null;

  /** @type {SigningKey | null} */
  let cachedSigningKey = null;
  /**
   * @returns {Promise<SigningKey>}
   */
  const resolveKeyOnce = async () => {
    if (cachedSigningKey) return cachedSigningKey;
    const keyPath = asOptionalString(
      pickConfigValue(
        env,
        fileConfig,
        'X402_SIGNING_KEY_PATH',
        'signingKeyPath',
        'signing_key_path',
      ),
    );
    const rawKey = pickConfigValue(
      env,
      fileConfig,
      'X402_SIGNING_KEY',
      'signingKey',
      'signing_key',
    );
    const keyJson = rawKey
      ? typeof rawKey === 'string'
        ? /** @type {SigningKeyJson} */ (JSON.parse(rawKey))
        : /** @type {SigningKeyJson} */ (rawKey)
      : null;
    cachedSigningKey = await resolveSigningKey({
      agentId,
      keyId: agentKeyId,
      configDir,
      keyJson,
      keyPath,
    });
    return cachedSigningKey;
  };

  /**
   * @returns {Promise<ResolvedX402Config>}
   */
  const ensureConfig = async () => {
    if (!agentId) {
      throw new Error('X402_AGENT_ID is required');
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
      agentKeyId: agentKeyId ?? signingKey?.keyId ?? 1,
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

  /**
   * @param {JsonRecord | null | undefined} input
   * @param {unknown} transform
   * @returns {JsonRecord}
   */
  const applyPolicyTransform = (input, transform) => {
    if (!transform || typeof transform !== 'object' || Array.isArray(transform)) {
      return input || {};
    }

    const output = { ...(input || {}) };
    for (const [key, value] of Object.entries(/** @type {JsonRecord} */ (transform))) {
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
    policyStorePath || env.STATESET_POLICY_DIR || path.resolve(configDir || '.stateset');
  /** @type {PolicyEngineLike | null} */
  let defaultPolicyEngine = null;
  /** @type {Promise<PolicyEngineLike | null> | null} */
  let policyEngineLoad = null;

  /**
   * @returns {Promise<PolicyEngineLike | null>}
   */
  const getPolicyEngine = async () => {
    if (policyEngine) return policyEngine;
    if (!resolvePolicyStorePath) return null;
    if (defaultPolicyEngine) return defaultPolicyEngine;
    if (!policyEngineLoad) {
      policyEngineLoad = (async () => {
        try {
          const { PolicyEngine } = await loadPolicyEngineModule();
          const engine = new PolicyEngine({
            storePath: resolvePolicyStorePath,
            unknownDomainMode: 'allow',
          });
          await engine.load();
          defaultPolicyEngine = engine;
          return engine;
        } catch (err) {
          console.debug('x402 policy load failed:', messageFromError(err));
          return null;
        }
      })();
    }
    return policyEngineLoad;
  };

  /**
   * @param {string} toolName
   * @param {JsonRecord} params
   * @param {PolicyExtra} [extra]
   * @returns {Promise<PolicyDecision>}
   */
  const evaluatePolicy = async (toolName, params, extra = {}) => {
    const policyEngineInstance = await getPolicyEngine();
    if (!policyEngineInstance) return { allowed: true, params };

    const context = {
      domain: 'x402',
      tool: toolName,
      params,
      requestId: extra?.requestId || null,
      sessionId: extra?.sessionId || null,
    };

    /** @type {PolicyEvaluation | null} */
    let evaluation = null;
    try {
      evaluation = /** @type {PolicyEvaluation} */ (
        await policyEngineInstance.evaluate('x402', context)
      );
    } catch (err) {
      console.debug(
        '[x402-mcp-server] Policy evaluation failed, allowing by default:',
        messageFromError(err),
      );
      return { allowed: true, params };
    }

    const actions = Array.isArray(evaluation?.actions) ? evaluation.actions : [];
    let transformedParams = params;
    for (const action of actions) {
      if (action?.type === 'transform') {
        transformedParams = applyPolicyTransform(transformedParams, action.transform);
      }
    }

    if (evaluation?.shouldDeny) {
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
        evaluation,
      };
    }

    return {
      allowed: true,
      params: transformedParams,
      actions,
      evaluation,
    };
  };

  /**
   * @param {string} toolName
   * @param {PolicyDecision} policy
   */
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

  /**
   * @param {string} toolName
   * @param {(args: JsonRecord, extra?: PolicyExtra) => Promise<unknown> | unknown} handler
   * @returns {any}
   */
  const withPolicy =
    (toolName, handler) =>
    /**
     * @param {JsonRecord | null | undefined} args
     * @param {PolicyExtra} [extra]
     */
    async (args, extra) => {
      const policy = await evaluatePolicy(toolName, args || {}, extra);
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
          /** @param {JsonRecord} rawArgs */
          async (rawArgs) => {
            const {
              url,
              method = 'GET',
              headers = {},
              body,
              maxAmount: perCallMax,
              requireReceipt: perCallReceipt,
            } = /** @type {X402CallArgs} */ (rawArgs);
            try {
              const config = await ensureConfig();
              const agent = createX402Agent({
                ...config,
                tenantId: config.tenantId ?? undefined,
                storeId: config.storeId ?? undefined,
                maxAmount: perCallMax ?? config.maxAmount,
                requireReceipt: perCallReceipt ?? config.requireReceipt,
              });

              /** @type {Record<string, string>} */
              const requestHeaders = { ...headers };
              /** @type {BodyInit | null | undefined} */
              let requestBody;
              if (
                typeof body === 'string' ||
                body instanceof Buffer ||
                body instanceof URLSearchParams ||
                body instanceof FormData ||
                body instanceof Blob ||
                body instanceof ArrayBuffer ||
                ArrayBuffer.isView(body)
              ) {
                requestBody = /** @type {BodyInit} */ (body);
              } else if (body !== undefined && body !== null) {
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
                  : messageFromError(error);
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
        withPolicy('x402_history', (rawArgs) => {
          const { limit = 50 } = /** @type {X402HistoryArgs} */ (rawArgs);
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
        withPolicy('x402_receipt', async (rawArgs) => {
          const { intentId } = /** @type {X402ReceiptArgs} */ (rawArgs);
          try {
            if (!sequencerClient) throw new Error('X402_SEQUENCER_URL is required');
            const receipt = await sequencerClient.getPaymentReceipt(intentId);
            return result({ success: true, receipt });
          } catch (error) {
            return errorResult(messageFromError(error));
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
        withPolicy('x402_balance', async (rawArgs) => {
          const { chain, token, address } = /** @type {X402BalanceArgs} */ (rawArgs);
          if (!chain) {
            return result({
              success: true,
              balance: null,
              message: 'Chain not provided. Use x402_budget_status for local budget tracking.',
            });
          }
          try {
            const { getBalance } = await loadChainsModule();
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
            return errorResult(messageFromError(error));
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
