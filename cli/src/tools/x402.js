/**
 * x402 Protocol Tools Module
 *
 * MCP tool definitions for x402 AI agent commerce protocol operations.
 * Includes payment intents, signing, settlement, and credit ledger (metered billing).
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { computeX402SigningHash, signX402Hash, hashToHex } from '../x402/crypto.js';
import { resolveCommerceApi } from '../commerce.js';

const CHAINS_MODULE = ['..', 'chains', 'index.js'].join('/');
const SYNC_KEYS_MODULE = ['..', 'sync', 'keys.js'].join('/');
const TREASURY_MODULE = ['..', 'treasury', 'index.js'].join('/');

/**
 * @typedef {Record<string, unknown>} JsonRecord
 * @typedef {{ [key: string]: unknown }} RawEntity
 * @typedef {{
 *   id?: unknown,
 *   status?: unknown,
 *   payerAddress?: unknown,
 *   payer_address?: unknown,
 *   payeeAddress?: unknown,
 *   payee_address?: unknown,
 *   amount?: unknown,
 *   amountDecimal?: unknown,
 *   amount_decimal?: unknown,
 *   asset?: unknown,
 *   network?: unknown,
 *   chainId?: unknown,
 *   chain_id?: unknown,
 *   validUntil?: unknown,
 *   valid_until?: unknown,
 *   nonce?: unknown,
 *   signingHash?: unknown,
 *   signing_hash?: unknown,
 *   idempotencyKey?: unknown,
 *   idempotency_key?: unknown,
 *   resourceUri?: unknown,
 *   resource_uri?: unknown,
 *   resourceMethod?: unknown,
 *   resource_method?: unknown,
 *   description?: unknown,
 *   merchantId?: unknown,
 *   merchant_id?: unknown,
 *   metadata?: unknown,
 *   payerSignature?: unknown,
 *   payer_signature?: unknown,
 *   payerPublicKey?: unknown,
 *   payer_public_key?: unknown,
 *   txHash?: unknown,
 *   tx_hash?: unknown,
 *   blockNumber?: unknown,
 *   block_number?: unknown,
 * }} IntentLike
 * @typedef {{
 *   id: unknown,
 *   status: unknown,
 *   payerAddress: unknown,
 *   payeeAddress: unknown,
 *   amount: unknown,
 *   amountDecimal: unknown,
 *   asset: unknown,
 *   network: unknown,
 *   chainId: unknown,
 *   validUntil: unknown,
 *   nonce: unknown,
 *   signingHash: unknown,
 *   idempotencyKey: unknown,
 *   resourceUri: unknown,
 *   resourceMethod: unknown,
 *   description: unknown,
 *   merchantId: unknown,
 *   metadata: unknown,
 *   payerSignature: unknown,
 *   payerPublicKey: unknown,
 *   txHash: unknown,
 *   blockNumber: unknown,
 * }} NormalizedIntent
 * @typedef {{ keyId?: number | null, privateKey?: Buffer, publicKey?: Buffer }} SigningKey
 * @typedef {{
 *   getSigningKey: (agentId: string, keyId: number) => Promise<SigningKey | null>,
 *   getCurrentSigningKey: (agentId: string) => Promise<SigningKey | null>,
 *   ensureKeys: (agentId: string) => Promise<{ signingKey: SigningKey }>,
 * }} KeyManagerLike
 * @typedef {{ symbol: string, decimals: number }} TokenConfigLike
 * @typedef {{ chainId?: number }} ChainConfigLike
 * @typedef {{
 *   success: boolean,
 *   error?: string,
 *   blockNumber?: number | string | null,
 *   txHash?: string | null,
 *   confirmations?: number | null,
 *   explorerUrl?: string | null,
 * }} PaymentResultLike
 * @typedef {{
 *   getChain: (chainId: string) => ChainConfigLike | null | undefined,
 *   listChains: () => string[],
 *   getWalletAddress: (agentId: string, chainId: string, options?: JsonRecord) => Promise<string>,
 *   isEvmChain: (chainId: string) => boolean,
 *   getToken: (chainId: string, token: string) => TokenConfigLike | null | undefined,
 *   getDefaultStablecoin: (chainId: string) => TokenConfigLike | null | undefined,
 *   fromSmallestUnit: (amount: bigint, decimals: number) => string,
 *   executePayment: (
 *     params: {
 *       agentId: string,
 *       chainId: string,
 *       toAddress: string,
 *       amount: string | number,
 *       tokenSymbol?: string,
 *       metadata?: JsonRecord,
 *     },
 *     options?: JsonRecord,
 *   ) => Promise<PaymentResultLike>,
 * }} ChainsModuleLike
 * @typedef {{ store: { findByTx?: (query: JsonRecord) => unknown } }} TreasuryContextLike
 * @typedef {{
 *   loadTreasuryContext: (options?: JsonRecord) => Promise<TreasuryContextLike>,
 *   recordWithdrawal: (entry: JsonRecord, context: TreasuryContextLike) => Promise<unknown>,
 *   recordDeposit: (entry: JsonRecord, context: TreasuryContextLike) => Promise<JsonRecord>,
 * }} TreasuryModuleLike
 * @typedef {{
 *   createIntent: (payload: JsonRecord) => Promise<JsonRecord>,
 *   signIntent: (intentId: string, payload: JsonRecord) => Promise<JsonRecord>,
 *   getIntent: (intentId: string) => Promise<JsonRecord | null>,
 *   listIntents: (params: JsonRecord) => Promise<JsonRecord[]>,
 *   markSettled: (intentId: string, txHash: string, blockNumber: number) => Promise<JsonRecord>,
 *   getNextNonce: (payerAddress: string) => Promise<unknown>,
 *   getCreditBalance: (params: JsonRecord) => Promise<unknown>,
 *   creditAccount: (params: JsonRecord) => Promise<JsonRecord>,
 *   debitAccount: (params: JsonRecord) => Promise<JsonRecord>,
 *   listCreditTransactions: (params: JsonRecord) => Promise<JsonRecord[]>,
 * }} CommerceX402ApiLike
 * @typedef {{ x402?: CommerceX402ApiLike | (() => CommerceX402ApiLike) }} CommerceLike
 * @typedef {{
 *   commerce: CommerceLike,
 *   params: JsonRecord,
 *   allowApply?: boolean,
 *   resolveTreasuryAgentId?: (() => Promise<string>) | undefined,
 *   treasuryContextOptions?: JsonRecord | undefined,
 *   buildAuditContext?: ((extra: unknown, toolName: string) => JsonRecord) | undefined,
 *   buildTreasuryIdentityMetadata?: (() => Promise<JsonRecord> | JsonRecord) | undefined,
 *   extra?: unknown,
 * }} ToolHandlerInput
 * @typedef {{
 *   commerce: CommerceLike,
 *   intentId: string,
 *   agentId: string,
 *   keyId?: number,
 *   chain?: string,
 *   configDir?: string,
 * }} SignIntentWithLocalAgentInput
 * @typedef {{ payerAddress: string, payeeAddress: string, amount: number, asset?: string, network?: string, cartId?: string, orderId?: string, description?: string, validitySeconds?: number }} CreatePaymentIntentParams
 * @typedef {{ intentId: string, signature?: string, publicKey?: string, agentId?: string, keyId?: number, chain?: string }} SignIntentParams
 * @typedef {{ intentId: string }} GetIntentParams
 * @typedef {{ payerAddress?: string, payeeAddress?: string, status?: string, network?: string, limit?: number }} ListIntentsParams
 * @typedef {{ intentId: string, agentId?: string, payeeAgentId?: string, chain?: string, token?: string }} SettleIntentParams
 * @typedef {{ amount: number, payerAgentId?: string, payeeAgentId?: string, payerAddress?: string, payeeAddress?: string, asset?: string, network?: string, chain?: string, token?: string, keyId?: number, cartId?: string, orderId?: string, description?: string, validitySeconds?: number, recordIncoming?: boolean }} ExecuteAgentPaymentParams
 * @typedef {{ intentId: string, payeeAgentId: string, chain?: string, token?: string }} RecordIncomingSettlementParams
 * @typedef {{ intentId: string, txHash: string, blockNumber: number }} MarkSettledParams
 * @typedef {{ payerAddress: string }} GetNextNonceParams
 * @typedef {{ payerAddress: string, asset?: string, network?: string }} CreditBalanceParams
 * @typedef {{ payerAddress: string, amount: number, asset?: string, network?: string, reason?: string, referenceId?: string, metadata?: string }} CreditMutationParams
 * @typedef {{ payerAddress?: string, asset?: string, network?: string, direction?: string, limit?: number }} CreditTransactionsParams
 * @typedef {ToolHandlerInput & { params: SettleIntentParams }} SettleIntentHandlerInput
 * @typedef {{
 *   success: boolean,
 *   [key: string]: unknown,
 *   error?: string,
 *   message?: string,
 *   settlement?: JsonRecord | null,
 *   incomingSettlement?: JsonRecord | null,
 *   intent?: JsonRecord | null,
 * }} SettleIntentToolResult
 * @typedef {{
 *   name: string,
 *   description: string,
 *   inputSchema: JsonRecord,
 *   permission: 'read' | 'write',
 *   handler: (input: any) => Promise<JsonRecord>,
 * }} X402Tool
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @returns {Promise<ChainsModuleLike>}
 */
async function loadChainsModule() {
  return /** @type {Promise<ChainsModuleLike>} */ (import(CHAINS_MODULE));
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
 * @returns {Promise<TreasuryModuleLike>}
 */
async function loadTreasuryModule() {
  return /** @type {Promise<TreasuryModuleLike>} */ (import(TREASURY_MODULE));
}

/**
 * @param {RawEntity | null | undefined} object
 * @param {string[]} keys
 * @returns {unknown}
 */
function firstValue(object, keys) {
  if (!object || typeof object !== 'object') return undefined;
  for (const key of keys) {
    const value = object[key];
    if (value !== undefined && value !== null) {
      return value;
    }
  }
  return undefined;
}

/**
 * @param {IntentLike | JsonRecord | null | undefined} intent
 * @returns {NormalizedIntent}
 */
function normalizeIntent(intent) {
  return {
    id: firstValue(intent, ['id']),
    status: firstValue(intent, ['status']),
    payerAddress: firstValue(intent, ['payerAddress', 'payer_address']),
    payeeAddress: firstValue(intent, ['payeeAddress', 'payee_address']),
    amount: firstValue(intent, ['amount']),
    amountDecimal: firstValue(intent, ['amountDecimal', 'amount_decimal']),
    asset: firstValue(intent, ['asset']),
    network: firstValue(intent, ['network']),
    chainId: firstValue(intent, ['chainId', 'chain_id']),
    validUntil: firstValue(intent, ['validUntil', 'valid_until']),
    nonce: firstValue(intent, ['nonce']),
    signingHash: firstValue(intent, ['signingHash', 'signing_hash']),
    idempotencyKey: firstValue(intent, ['idempotencyKey', 'idempotency_key']),
    resourceUri: firstValue(intent, ['resourceUri', 'resource_uri']),
    resourceMethod: firstValue(intent, ['resourceMethod', 'resource_method']),
    description: firstValue(intent, ['description']),
    merchantId: firstValue(intent, ['merchantId', 'merchant_id']),
    metadata: firstValue(intent, ['metadata']),
    payerSignature: firstValue(intent, ['payerSignature', 'payer_signature']),
    payerPublicKey: firstValue(intent, ['payerPublicKey', 'payer_public_key']),
    txHash: firstValue(intent, ['txHash', 'tx_hash']),
    blockNumber: firstValue(intent, ['blockNumber', 'block_number']),
  };
}

/**
 * @param {CommerceLike} commerce
 * @returns {CommerceX402ApiLike}
 */
function getX402Api(commerce) {
  return /** @type {CommerceX402ApiLike} */ (resolveCommerceApi(commerce, 'x402'));
}

/**
 * @param {CreatePaymentIntentParams | ExecuteAgentPaymentParams} params
 * @returns {JsonRecord}
 */
function buildCreateIntentInput(params) {
  return {
    payerAddress: params.payerAddress,
    payer_address: params.payerAddress,
    payeeAddress: params.payeeAddress,
    payee_address: params.payeeAddress,
    amount: params.amount,
    asset: params.asset || 'usdc',
    network: params.network || 'set_chain',
    cartId: params.cartId,
    cart_id: params.cartId,
    orderId: params.orderId,
    order_id: params.orderId,
    description: params.description,
    validitySeconds: params.validitySeconds,
    validity_seconds: params.validitySeconds,
  };
}

/**
 * @param {{
 *   intentId?: string,
 *   signature: string,
 *   publicKey: string,
 *   signatureScheme?: string,
 *   signatureBundle?: unknown,
 *   publicKeyBundle?: unknown,
 * }} params
 * @returns {JsonRecord}
 */
function buildSignIntentInput({
  intentId,
  signature,
  publicKey,
  signatureScheme,
  signatureBundle,
  publicKeyBundle,
}) {
  return {
    intentId,
    intent_id: intentId,
    signature,
    publicKey,
    public_key: publicKey,
    signatureScheme,
    signature_scheme: signatureScheme,
    signatureBundle,
    signature_bundle: signatureBundle,
    publicKeyBundle,
    public_key_bundle: publicKeyBundle,
  };
}

/**
 * @param {ListIntentsParams} params
 * @returns {JsonRecord}
 */
function buildListIntentsInput(params) {
  return {
    payerAddress: params.payerAddress,
    payer_address: params.payerAddress,
    payeeAddress: params.payeeAddress,
    payee_address: params.payeeAddress,
    status: params.status,
    network: params.network,
    limit: params.limit || 50,
  };
}

/**
 * @param {CreditBalanceParams} params
 * @returns {JsonRecord}
 */
function buildCreditBalanceInput(params) {
  return {
    payerAddress: params.payerAddress,
    payer_address: params.payerAddress,
    asset: params.asset,
    network: params.network,
  };
}

/**
 * @param {CreditMutationParams} params
 * @returns {JsonRecord}
 */
function buildCreditMutationInput(params) {
  return {
    payerAddress: params.payerAddress,
    payer_address: params.payerAddress,
    asset: params.asset,
    network: params.network,
    amount: params.amount,
    reason: params.reason,
    referenceId: params.referenceId,
    reference_id: params.referenceId,
    metadata: params.metadata,
  };
}

/**
 * @param {CreditTransactionsParams} params
 * @returns {JsonRecord}
 */
function buildCreditTransactionFilterInput(params) {
  return {
    payerAddress: params.payerAddress,
    payer_address: params.payerAddress,
    asset: params.asset,
    network: params.network,
    direction: params.direction,
    limit: params.limit || 50,
  };
}

/**
 * @param {IntentLike | JsonRecord | null | undefined} intent
 * @returns {string}
 */
function ensureIntentSigningHash(intent) {
  const normalized = normalizeIntent(intent);
  if (normalized.signingHash) {
    return String(normalized.signingHash);
  }

  if (
    normalized.payerAddress === undefined ||
    normalized.payeeAddress === undefined ||
    normalized.amount === undefined ||
    normalized.asset === undefined ||
    normalized.network === undefined ||
    normalized.validUntil === undefined ||
    normalized.nonce === undefined
  ) {
    throw new Error('Intent is missing signing hash');
  }

  return hashToHex(
    computeX402SigningHash({
      payerAddress: normalized.payerAddress,
      payeeAddress: normalized.payeeAddress,
      amount: normalized.amount,
      asset: normalized.asset,
      network: normalized.network,
      chainId: normalized.chainId,
      validUntil: normalized.validUntil,
      nonce: normalized.nonce,
      resourceUri: normalized.resourceUri,
      resourceMethod: normalized.resourceMethod,
    }),
  );
}

/**
 * @param {JsonRecord | null | undefined} txn
 * @returns {JsonRecord}
 */
function normalizeCreditTransaction(txn) {
  return {
    id: firstValue(txn, ['id']),
    accountId: firstValue(txn, ['accountId', 'account_id']),
    payerAddress: firstValue(txn, ['payerAddress', 'payer_address']),
    direction: firstValue(txn, ['direction']),
    amount: firstValue(txn, ['amount']),
    balanceAfter: firstValue(txn, ['balanceAfter', 'balance_after']),
    createdAt: firstValue(txn, ['createdAt', 'created_at']),
  };
}

/** @type {Record<string, string>} */
const ASSET_TO_TOKEN = {
  usdc: 'USDC',
  usdt: 'USDT',
  ssusd: 'ssUSD',
  ss_usd: 'ssUSD',
  wssusd: 'wssUSD',
  wss_usd: 'wssUSD',
  dai: 'DAI',
  eth: 'ETH',
  ether: 'ETH',
  sol: 'SOL',
  btc: 'BTC',
  zec: 'ZEC',
};

/**
 * @param {unknown} asset
 * @returns {string | null}
 */
function normalizeAssetToToken(asset) {
  if (!asset) return null;
  const key = String(asset).toLowerCase();
  return ASSET_TO_TOKEN[key] || String(asset);
}

/** @type {Record<string, string>} */
const X402_NETWORK_TO_CHAIN = {
  set: 'set_chain',
  setchain: 'set_chain',
  set_chain: 'set_chain',
  set_testnet: 'set_chain_testnet',
  set_chain_testnet: 'set_chain_testnet',
  arc: 'arc',
  arc_testnet: 'arc_testnet',
  base: 'base',
  ethereum: 'ethereum',
  eth: 'ethereum',
  arbitrum: 'arbitrum',
  solana: 'solana',
  solana_devnet: 'solana_devnet',
};

/**
 * @param {unknown} network
 * @returns {string | null}
 */
function normalizeX402NetworkToChain(network) {
  if (!network) return null;

  const value = String(network).trim();
  if (!value) return null;

  const key = value.toLowerCase();
  return X402_NETWORK_TO_CHAIN[key] || key;
}

/** @type {Record<string, string>} */
const CHAIN_TO_X402_NETWORK = {
  set: 'set_chain',
  setchain: 'set_chain',
  set_chain: 'set_chain',
  set_testnet: 'set_chain_testnet',
  set_chain_testnet: 'set_chain_testnet',
  arc: 'arc',
  arc_testnet: 'arc_testnet',
  base: 'base',
  ethereum: 'ethereum',
  eth: 'ethereum',
  arbitrum: 'arbitrum',
};

/**
 * @param {unknown} chain
 * @returns {string | null}
 */
function normalizeChainToX402Network(chain) {
  if (!chain) return null;

  const value = String(chain).trim();
  if (!value) return null;

  const key = value.toLowerCase();
  return CHAIN_TO_X402_NETWORK[key] || null;
}

/**
 * @param {unknown} value
 * @returns {Buffer}
 */
function decodeHashBytes(value) {
  if (!value) {
    throw new Error('Missing signing hash');
  }

  const raw = String(value).trim();
  if (!raw) {
    throw new Error('Signing hash is empty');
  }

  const hex = raw.startsWith('0x') ? raw.slice(2) : raw;
  if (/^[0-9a-fA-F]+$/.test(hex) && hex.length % 2 === 0) {
    return Buffer.from(hex, 'hex');
  }

  return Buffer.from(raw, 'base64');
}

/**
 * @param {unknown} value
 * @returns {string | null}
 */
function normalizeMaybeString(value) {
  if (value === undefined || value === null) return null;
  const raw = String(value).trim();
  return raw || null;
}

/**
 * @param {string} chainId
 * @param {(chainId: string) => boolean} isEvmChain
 * @returns {(address: unknown) => string}
 */
function createAddressNormalizer(chainId, isEvmChain) {
  return (address) => (isEvmChain(chainId) ? String(address).toLowerCase() : String(address));
}

/**
 * @param {NormalizedIntent} intent
 * @param {unknown} chainOverride
 * @returns {Promise<string | null>}
 */
async function resolveIntentChainId(intent, chainOverride) {
  const { getChain, listChains } = await loadChainsModule();
  let chainId = normalizeX402NetworkToChain(chainOverride || intent.network);

  if ((!chainId || !getChain(chainId)) && intent.chainId !== undefined && intent.chainId !== null) {
    const numericChainId = Number(intent.chainId);
    if (Number.isFinite(numericChainId)) {
      chainId =
        listChains().find((candidate) => {
          const config = getChain(candidate);
          return config?.chainId === numericChainId;
        }) || chainId;
    }
  }

  return chainId && getChain(chainId) ? chainId : null;
}

/**
 * @param {SignIntentWithLocalAgentInput} input
 * @returns {Promise<{
 *   signed: JsonRecord,
 *   intent: NormalizedIntent,
 *   chainId: string | null,
 *   signature: string,
 *   publicKey: string,
 *   agentId: string,
 *   keyId: number | null,
 * }>}
 */
async function signIntentWithLocalAgent({
  commerce,
  intentId,
  agentId,
  keyId,
  chain,
  configDir = '.stateset',
}) {
  if (!agentId) {
    throw new Error('agentId is required for local signing');
  }

  const x402 = getX402Api(commerce);
  const rawIntent = await x402.getIntent(intentId);
  if (!rawIntent) {
    throw new Error('Payment intent not found');
  }

  const intent = normalizeIntent(rawIntent);
  intent.signingHash = ensureIntentSigningHash(intent);

  const chainId = await resolveIntentChainId(intent, chain);
  if (chainId && intent.payerAddress) {
    const { getWalletAddress, isEvmChain } = await loadChainsModule();
    const walletAddress = await getWalletAddress(agentId, chainId, { configDir });
    const normalizeAddress = createAddressNormalizer(chainId, isEvmChain);
    if (normalizeAddress(walletAddress) !== normalizeAddress(intent.payerAddress)) {
      throw new Error(
        `Agent wallet does not match intent payer address (expected: ${intent.payerAddress}, agent wallet: ${walletAddress})`,
      );
    }
  }

  const { getKeyManager } = await loadKeyManagerModule();
  const manager = getKeyManager(configDir);
  const parsedKeyId =
    keyId === undefined || keyId === null ? null : Number.parseInt(String(keyId), 10);
  if (parsedKeyId !== null && (!Number.isInteger(parsedKeyId) || parsedKeyId <= 0)) {
    throw new Error('keyId must be a positive integer');
  }

  let signingKey =
    parsedKeyId !== null
      ? await manager.getSigningKey(agentId, parsedKeyId)
      : await manager.getCurrentSigningKey(agentId);

  if (!signingKey) {
    if (parsedKeyId !== null) {
      throw new Error(`Signing key ${parsedKeyId} not found for agent ${agentId}`);
    }
    const ensured = await manager.ensureKeys(agentId);
    signingKey = ensured.signingKey;
  }

  if (!signingKey?.privateKey || !signingKey?.publicKey) {
    throw new Error(`Signing key material is invalid for agent ${agentId}`);
  }

  const signingHashBytes = decodeHashBytes(intent.signingHash);
  const signatureBytes = signX402Hash(signingHashBytes, signingKey.privateKey);
  const signature = hashToHex(signatureBytes);
  const publicKey = hashToHex(signingKey.publicKey);

  const signed = await x402.signIntent(
    intentId,
    buildSignIntentInput({
      intentId,
      signature,
      publicKey,
    }),
  );

  return {
    signed,
    intent,
    chainId,
    signature,
    publicKey,
    agentId,
    keyId: signingKey.keyId ?? parsedKeyId ?? null,
  };
}

/**
 * @param {SettleIntentHandlerInput} input
 * @returns {Promise<SettleIntentToolResult>}
 */
async function handleSettleIntentOnchain({
  commerce,
  params,
  allowApply,
  resolveTreasuryAgentId,
  treasuryContextOptions,
  buildAuditContext,
  buildTreasuryIdentityMetadata,
  extra,
}) {
  const { intentId, agentId, payeeAgentId, chain, token } = params;
  if (!allowApply) {
    return {
      success: false,
      error: 'Settling an x402 intent on-chain requires --apply flag.',
      wouldSettle: {
        intentId,
        agentId,
        payeeAgentId: payeeAgentId || null,
        chain: chain || null,
        token: token || null,
      },
      instruction: 'Run with --apply to execute and settle this intent',
    };
  }

  const x402 = getX402Api(commerce);
  const rawIntent = await x402.getIntent(intentId);
  if (!rawIntent) {
    return { success: false, error: 'Payment intent not found' };
  }

  const intent = normalizeIntent(rawIntent);
  if (!intent.payerAddress || !intent.payeeAddress) {
    return { success: false, error: 'Intent is missing payer/payee addresses' };
  }
  if (!intent.status) {
    return { success: false, error: 'Intent status is missing' };
  }
  if (!['signed', 'sequenced', 'batched'].includes(String(intent.status).toLowerCase())) {
    return {
      success: false,
      error: `Intent must be signed, sequenced, or batched before settlement (current: ${intent.status})`,
    };
  }
  if (intent.txHash) {
    return {
      success: false,
      error: 'Intent already has a transaction hash and appears settled or in-progress',
      intent: {
        id: intent.id,
        status: intent.status,
        txHash: intent.txHash,
        blockNumber: intent.blockNumber ?? null,
      },
    };
  }

  const {
    executePayment,
    getToken,
    getDefaultStablecoin,
    getWalletAddress,
    getChain,
    listChains,
    fromSmallestUnit,
    isEvmChain,
  } = await loadChainsModule();

  let chainId = normalizeX402NetworkToChain(chain || intent.network);
  if ((!chainId || !getChain(chainId)) && intent.chainId !== undefined && intent.chainId !== null) {
    const numericChainId = Number(intent.chainId);
    if (Number.isFinite(numericChainId)) {
      chainId =
        listChains().find((candidate) => {
          const config = getChain(candidate);
          return config?.chainId === numericChainId;
        }) || chainId;
    }
  }

  if (!chainId || !getChain(chainId)) {
    return { success: false, error: 'Unable to determine target chain (provide --chain)' };
  }

  const inferredToken = token || normalizeAssetToToken(intent.asset);
  const tokenConfig = inferredToken
    ? getToken(chainId, inferredToken)
    : getDefaultStablecoin(chainId);
  if (!tokenConfig) {
    return {
      success: false,
      error: `Unable to resolve token for intent asset=${intent.asset || 'unknown'} on chain=${chainId}`,
    };
  }

  const effectiveAgentId =
    agentId || (resolveTreasuryAgentId ? await resolveTreasuryAgentId() : 'default');
  const payerWallet = await getWalletAddress(effectiveAgentId, chainId, {
    configDir: '.stateset',
  });
  const normalizeAddress = createAddressNormalizer(chainId, isEvmChain);
  const payerAddress = String(intent.payerAddress);
  const payeeAddress = String(intent.payeeAddress);
  if (normalizeAddress(payerWallet) !== normalizeAddress(payerAddress)) {
    return {
      success: false,
      error: 'Agent wallet does not match intent payer address',
      expectedPayer: intent.payerAddress,
      agentWallet: payerWallet,
      chain: chainId,
      agentId: effectiveAgentId,
    };
  }
  if (payeeAgentId) {
    const payeeWallet = await getWalletAddress(payeeAgentId, chainId, {
      configDir: '.stateset',
    });
    if (normalizeAddress(payeeWallet) !== normalizeAddress(payeeAddress)) {
      return {
        success: false,
        error: 'Payee agent wallet does not match intent payee address',
        expectedPayee: intent.payeeAddress,
        payeeAgentWallet: payeeWallet,
        chain: chainId,
        payeeAgentId,
      };
    }
  }

  const amount =
    intent.amountDecimal !== undefined && intent.amountDecimal !== null
      ? String(intent.amountDecimal)
      : intent.amount !== undefined && intent.amount !== null
        ? fromSmallestUnit(BigInt(String(intent.amount)), tokenConfig.decimals)
        : null;
  if (!amount) {
    return { success: false, error: 'Intent amount is missing' };
  }
  const numericAmount = Number.parseFloat(String(amount));
  if (!Number.isFinite(numericAmount) || numericAmount <= 0) {
    return {
      success: false,
      error: `Intent amount must be greater than zero (resolved: ${amount})`,
    };
  }

  const paymentResult = await executePayment(
    {
      agentId: effectiveAgentId,
      chainId,
      toAddress: payeeAddress,
      amount,
      tokenSymbol: tokenConfig.symbol,
      metadata: {
        x402_intent_id: intentId,
        x402_nonce: intent.nonce ?? null,
        x402_asset: intent.asset ?? null,
        x402_network: intent.network ?? null,
      },
    },
    {
      configDir: '.stateset',
      simulate: false,
    },
  );

  if (!paymentResult.success) {
    return {
      success: false,
      error: paymentResult.error || 'On-chain settlement failed',
      intentId,
      chain: chainId,
      token: tokenConfig.symbol,
    };
  }

  const settledBlockNumber = Number(paymentResult.blockNumber ?? 0);
  if (!Number.isFinite(settledBlockNumber) || settledBlockNumber < 0) {
    return {
      success: false,
      error: `Invalid block number from payment confirmation: ${paymentResult.blockNumber}`,
      intentId,
      chain: chainId,
      token: tokenConfig.symbol,
      txHash: paymentResult.txHash || null,
    };
  }
  if (!paymentResult.txHash) {
    return {
      success: false,
      error: 'Payment confirmation did not include a transaction hash',
      intentId,
      chain: chainId,
      token: tokenConfig.symbol,
    };
  }

  const settledIntent = await x402.markSettled(intentId, paymentResult.txHash, settledBlockNumber);

  /** @type {JsonRecord | null} */
  let incomingSettlement = null;
  try {
    const { loadTreasuryContext, recordWithdrawal, recordDeposit } = await loadTreasuryModule();
    const ctx = await loadTreasuryContext(treasuryContextOptions || {});
    const audit = buildAuditContext ? buildAuditContext(extra, 'x402_settle_intent_onchain') : {};
    const identityMeta = buildTreasuryIdentityMetadata ? await buildTreasuryIdentityMetadata() : {};
    await recordWithdrawal(
      {
        agentId: effectiveAgentId,
        chainId,
        tokenSymbol: tokenConfig.symbol,
        amount,
        txId: paymentResult.txHash || null,
        toAddress: payeeAddress,
        source: 'x402_settlement',
        metadata: {
          x402IntentId: intentId,
          x402Nonce: intent.nonce ?? null,
          x402Asset: intent.asset ?? null,
          x402Network: intent.network ?? null,
          ...identityMeta,
        },
        ...audit,
      },
      ctx,
    );

    if (payeeAgentId) {
      const duplicate =
        typeof ctx.store.findByTx === 'function'
          ? ctx.store.findByTx({
              agentId: payeeAgentId,
              chainId,
              tokenSymbol: tokenConfig.symbol,
              direction: 'deposit',
              source: 'x402_settlement_incoming',
              txId: paymentResult.txHash,
            })
          : null;

      if (duplicate) {
        incomingSettlement = {
          recorded: false,
          reason: 'already_recorded',
          payeeAgentId,
          chain: chainId,
          token: tokenConfig.symbol,
          txHash: paymentResult.txHash,
        };
      } else {
        const payeeAudit = buildAuditContext
          ? buildAuditContext(extra, 'x402_settle_intent_onchain')
          : {};
        const payeeEntry = await recordDeposit(
          {
            agentId: payeeAgentId,
            chainId,
            tokenSymbol: tokenConfig.symbol,
            amount,
            txId: paymentResult.txHash || null,
            fromAddress: payerAddress,
            source: 'x402_settlement_incoming',
            metadata: {
              x402IntentId: intentId,
              x402Nonce: intent.nonce ?? null,
              x402Asset: intent.asset ?? null,
              x402Network: intent.network ?? null,
            },
            ...payeeAudit,
          },
          ctx,
        );
        incomingSettlement = {
          recorded: true,
          payeeAgentId,
          chain: chainId,
          token: tokenConfig.symbol,
          txHash: paymentResult.txHash,
          entryEventId: payeeEntry.event_id ?? null,
        };
      }
    }
  } catch (auditError) {
    console.warn(
      `[Treasury] Failed to record x402 settlement withdrawal: ${messageFromError(auditError)}`,
    );
  }

  return {
    success: true,
    message: 'x402 intent settled on-chain and marked settled.',
    settlement: {
      intentId,
      agentId: effectiveAgentId,
      payeeAgentId: payeeAgentId || null,
      chain: chainId,
      token: tokenConfig.symbol,
      amount,
      txHash: paymentResult.txHash,
      blockNumber: paymentResult.blockNumber ?? null,
      confirmations: paymentResult.confirmations ?? null,
      explorerUrl: paymentResult.explorerUrl ?? null,
    },
    incomingSettlement,
    intent: {
      id: firstValue(settledIntent, ['id']),
      status: firstValue(settledIntent, ['status']),
      txHash: firstValue(settledIntent, ['txHash', 'tx_hash']),
      blockNumber: firstValue(settledIntent, ['blockNumber', 'block_number']),
      settledAt: firstValue(settledIntent, ['settledAt', 'settled_at']),
    },
  };
}

/**
 * x402 tool definitions
 */
/** @type {X402Tool[]} */
export const x402Tools = [
  {
    name: 'x402_create_payment_intent',
    description:
      'Create an x402 payment intent for AI agent commerce. Returns a signing hash that the payer agent must sign with Ed25519.',
    inputSchema: {
      payerAddress: z.string().min(1).describe('Payer wallet address (sender)'),
      payeeAddress: z.string().min(1).describe('Payee wallet address (recipient)'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      cartId: z.string().optional().describe('Cart ID to link this payment to'),
      orderId: z.string().optional().describe('Order ID for reference'),
      description: z.string().optional().describe('Description of what this payment is for'),
      validitySeconds: z
        .number()
        .optional()
        .describe('How long the intent is valid (default: 3600)'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      const x402 = getX402Api(commerce);
      const intent = normalizeIntent(await x402.createIntent(buildCreateIntentInput(params)));
      return {
        success: true,
        message: 'x402 payment intent created. Payer must sign the signing_hash.',
        intent: {
          id: intent.id,
          status: intent.status,
          payerAddress: intent.payerAddress,
          payeeAddress: intent.payeeAddress,
          amount: intent.amount,
          amountDecimal: intent.amountDecimal,
          asset: intent.asset,
          network: intent.network,
          chainId: intent.chainId,
          signingHash: intent.signingHash,
          validUntil: intent.validUntil,
          nonce: intent.nonce,
        },
      };
    },
  },

  {
    name: 'x402_sign_intent',
    description:
      'Sign an x402 payment intent with an Ed25519 signature. Supports manual signature/public key or local agent-key signing.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID to sign'),
      signature: z
        .string()
        .optional()
        .describe('Ed25519 signature over the signing_hash (hex or base64 encoded)'),
      publicKey: z.string().optional().describe('Payer Ed25519 public key (hex or base64 encoded)'),
      agentId: z
        .string()
        .optional()
        .describe(
          'Local payer agent ID to sign with local key material (default: treasury/default agent)',
        ),
      keyId: z.number().optional().describe('Optional signing key ID for local signing'),
      chain: z
        .string()
        .optional()
        .describe('Optional chain override used for local wallet-address verification'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, resolveTreasuryAgentId }) => {
      const { intentId } = params;
      const signature = normalizeMaybeString(params.signature);
      const publicKey = normalizeMaybeString(params.publicKey);
      const requestedAgentId = normalizeMaybeString(params.agentId);
      const wantsLocalSigning =
        requestedAgentId !== null || (signature === null && publicKey === null);

      if (!allowApply) {
        return {
          success: false,
          error: 'Signing x402 intent requires --apply flag.',
          wouldSign: {
            intentId,
            mode: wantsLocalSigning ? 'local_agent_key' : 'manual_signature',
            hasSignature: !!signature,
            hasPublicKey: !!publicKey,
            agentId: requestedAgentId || null,
            keyId: params.keyId ?? null,
            chain: params.chain || null,
          },
          instruction: 'Run with --apply to sign this payment intent',
        };
      }

      if (wantsLocalSigning && (signature || publicKey)) {
        return {
          success: false,
          error:
            'Provide either local signing params (agentId/keyId) or manual signature/publicKey, not both',
        };
      }

      if (!wantsLocalSigning && (!signature || !publicKey)) {
        return {
          success: false,
          error:
            'Manual signing requires both signature and publicKey. Omit both to use local agent-key signing.',
        };
      }

      if (wantsLocalSigning) {
        const effectiveAgentId =
          requestedAgentId || (resolveTreasuryAgentId ? await resolveTreasuryAgentId() : 'default');

        let locallySigned;
        try {
          locallySigned = await signIntentWithLocalAgent({
            commerce,
            intentId,
            agentId: effectiveAgentId,
            keyId: params.keyId,
            chain: params.chain,
            configDir: '.stateset',
          });
        } catch (err) {
          return { success: false, error: `Local signing failed: ${messageFromError(err)}` };
        }

        return {
          success: true,
          message: 'Payment intent signed with local agent key. Ready for settlement.',
          signing: {
            mode: 'local_agent_key',
            agentId: locallySigned.agentId,
            keyId: locallySigned.keyId,
            publicKey: locallySigned.publicKey,
            chain: locallySigned.chainId || null,
          },
          intent: {
            id: firstValue(locallySigned.signed, ['id']),
            status: firstValue(locallySigned.signed, ['status']),
            payerSignature: firstValue(locallySigned.signed, ['payerSignature', 'payer_signature']),
            payerPublicKey: firstValue(locallySigned.signed, [
              'payerPublicKey',
              'payer_public_key',
            ]),
          },
        };
      }

      const x402 = getX402Api(commerce);
      const signed = normalizeIntent(
        await x402.signIntent(
          intentId,
          buildSignIntentInput({
            intentId,
            signature: /** @type {string} */ (signature),
            publicKey: /** @type {string} */ (publicKey),
          }),
        ),
      );
      return {
        success: true,
        message: 'Payment intent signed. Ready for settlement.',
        signing: {
          mode: 'manual_signature',
        },
        intent: {
          id: signed.id,
          status: signed.status,
          payerSignature: signed.payerSignature,
          payerPublicKey: signed.payerPublicKey,
        },
      };
    },
  },

  {
    name: 'x402_get_intent',
    description: 'Get details of an x402 payment intent.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { intentId } = params;
      const x402 = getX402Api(commerce);
      const rawIntent = await x402.getIntent(intentId);
      if (!rawIntent) {
        return { success: false, error: 'Payment intent not found' };
      }
      const intent = normalizeIntent(rawIntent);
      return {
        success: true,
        intent: {
          id: intent.id,
          status: intent.status,
          payerAddress: intent.payerAddress,
          payeeAddress: intent.payeeAddress,
          amount: intent.amount,
          amountDecimal: intent.amountDecimal,
          asset: intent.asset,
          network: intent.network,
          chainId: intent.chainId,
          signingHash: intent.signingHash,
          payerSignature: intent.payerSignature,
          validUntil: intent.validUntil,
          nonce: intent.nonce,
          txHash: intent.txHash,
          blockNumber: intent.blockNumber,
        },
      };
    },
  },

  {
    name: 'x402_list_intents',
    description: 'List x402 payment intents with optional filtering.',
    inputSchema: {
      payerAddress: z.string().optional().describe('Filter by payer address'),
      payeeAddress: z.string().optional().describe('Filter by payee address'),
      status: z
        .string()
        .optional()
        .describe(
          'Filter by status: created, signed, sequenced, batched, settled, expired, failed, cancelled',
        ),
      network: z.string().optional().describe('Filter by network'),
      limit: z.number().optional().describe('Maximum results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = getX402Api(commerce);
      const intents = /** @type {JsonRecord[]} */ (await x402.listIntents(buildListIntentsInput(params)));
      return {
        success: true,
        count: intents.length,
        intents: intents.map((intent) => {
          const normalized = normalizeIntent(intent);
          return {
            id: normalized.id,
            status: normalized.status,
            payerAddress: normalized.payerAddress,
            payeeAddress: normalized.payeeAddress,
            amount: normalized.amount,
            asset: normalized.asset,
            network: normalized.network,
          };
        }),
      };
    },
  },

  {
    name: 'x402_settle_intent_onchain',
    description:
      'Execute a signed x402 intent on-chain using an agent wallet, then mark the intent as settled.',
    inputSchema: {
      intentId: z.string().min(1).describe('x402 payment intent ID'),
      agentId: z
        .string()
        .optional()
        .describe(
          'Local agent ID used to sign/broadcast the on-chain payment (default: treasury/default agent)',
        ),
      payeeAgentId: z
        .string()
        .optional()
        .describe(
          'Optional local payee agent ID to credit as an incoming settlement after successful execution',
        ),
      chain: z.string().optional().describe('Optional chain override (default: intent network)'),
      token: z
        .string()
        .optional()
        .describe('Optional token override (default: inferred from intent asset)'),
    },
    permission: 'write',
    handler: handleSettleIntentOnchain,
  },

  {
    name: 'x402_execute_agent_payment',
    description:
      'Execute end-to-end agentic payment: create intent, locally sign with payer agent key, settle on-chain, and optionally record incoming settlement for payee agent.',
    inputSchema: {
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      payerAgentId: z
        .string()
        .optional()
        .describe(
          'Local payer agent ID used for local signing and on-chain settlement (default: treasury/default agent)',
        ),
      payeeAgentId: z
        .string()
        .optional()
        .describe(
          'Optional local payee agent ID; if provided, payee wallet is derived automatically',
        ),
      payerAddress: z
        .string()
        .optional()
        .describe(
          'Optional explicit payer wallet address (defaults to derived payer agent wallet)',
        ),
      payeeAddress: z
        .string()
        .optional()
        .describe(
          'Optional explicit payee wallet address (required if payeeAgentId is not provided)',
        ),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('x402 network (default: inferred from chain or set_chain)'),
      chain: z
        .string()
        .optional()
        .describe('Settlement chain override (default: inferred from network)'),
      token: z
        .string()
        .optional()
        .describe('Settlement token override (default: inferred from intent asset)'),
      keyId: z.number().optional().describe('Optional signer key ID for payer local signing'),
      cartId: z.string().optional().describe('Cart ID to link this payment to'),
      orderId: z.string().optional().describe('Order ID to link this payment to'),
      description: z.string().optional().describe('Payment description'),
      validitySeconds: z
        .number()
        .optional()
        .describe('How long the intent is valid (default: 3600)'),
      recordIncoming: z
        .boolean()
        .optional()
        .describe(
          'When payeeAgentId is set, also record incoming settlement in payee treasury (default: true)',
        ),
    },
    permission: 'write',
    handler: async ({
      commerce,
      params,
      allowApply,
      resolveTreasuryAgentId,
      treasuryContextOptions,
      buildAuditContext,
      buildTreasuryIdentityMetadata,
      extra,
    }) => {
      const {
        amount,
        payerAgentId,
        payeeAgentId,
        payerAddress,
        payeeAddress,
        asset,
        network,
        chain,
        token,
        keyId,
        cartId,
        orderId,
        description,
        validitySeconds,
        recordIncoming,
      } = params;

      const effectivePayerAgentId =
        normalizeMaybeString(payerAgentId) ||
        (resolveTreasuryAgentId ? await resolveTreasuryAgentId() : 'default');
      const requestedPayeeAgentId = normalizeMaybeString(payeeAgentId);
      const shouldRecordIncoming = recordIncoming !== false;

      if (!allowApply) {
        return {
          success: false,
          error: 'Executing an end-to-end x402 payment requires --apply flag.',
          wouldExecute: {
            amount,
            payerAgentId: effectivePayerAgentId,
            payeeAgentId: requestedPayeeAgentId || null,
            payerAddress: normalizeMaybeString(payerAddress),
            payeeAddress: normalizeMaybeString(payeeAddress),
            asset: asset || 'usdc',
            network: network || normalizeChainToX402Network(chain) || 'set_chain',
            chain: chain || null,
            token: token || null,
            keyId: keyId ?? null,
            cartId: cartId || null,
            orderId: orderId || null,
            recordIncoming: shouldRecordIncoming,
          },
          instruction: 'Run with --apply to create, sign, and settle this x402 payment',
        };
      }

      if (!Number.isFinite(Number(amount)) || Number(amount) <= 0) {
        return { success: false, error: 'amount must be a positive number in smallest unit' };
      }

      const normalizedNetwork =
        normalizeChainToX402Network(network) ||
        normalizeMaybeString(network) ||
        normalizeChainToX402Network(chain) ||
        'set_chain';
      const chainId = normalizeX402NetworkToChain(chain || normalizedNetwork);
      if (!chainId) {
        return {
          success: false,
          error: 'Unable to determine settlement chain from network/chain inputs',
        };
      }

      if (network && chain) {
        const networkChain = normalizeX402NetworkToChain(network);
        if (networkChain && networkChain !== chainId) {
          return {
            success: false,
            error: `network=${network} resolves to ${networkChain}, which conflicts with chain=${chain}`,
          };
        }
      }

      const { getChain, getWalletAddress, isEvmChain } = await loadChainsModule();
      if (!getChain(chainId)) {
        return {
          success: false,
          error: `Unsupported settlement chain: ${chainId}`,
        };
      }

      const derivedPayerWallet = await getWalletAddress(effectivePayerAgentId, chainId, {
        configDir: '.stateset',
      });
      const requestedPayerAddress = normalizeMaybeString(payerAddress);
      const resolvedPayerAddress = requestedPayerAddress || derivedPayerWallet;

      const normalizeAddress = createAddressNormalizer(chainId, isEvmChain);
      if (
        requestedPayerAddress &&
        normalizeAddress(requestedPayerAddress) !== normalizeAddress(derivedPayerWallet)
      ) {
        return {
          success: false,
          error: 'Provided payerAddress does not match derived payer agent wallet',
          payerAgentId: effectivePayerAgentId,
          providedPayerAddress: requestedPayerAddress,
          derivedPayerWallet,
          chain: chainId,
        };
      }

      let resolvedPayeeAddress = normalizeMaybeString(payeeAddress);
      if (requestedPayeeAgentId) {
        const derivedPayeeWallet = await getWalletAddress(requestedPayeeAgentId, chainId, {
          configDir: '.stateset',
        });
        if (
          resolvedPayeeAddress &&
          normalizeAddress(resolvedPayeeAddress) !== normalizeAddress(derivedPayeeWallet)
        ) {
          return {
            success: false,
            error: 'Provided payeeAddress does not match derived payee agent wallet',
            payeeAgentId: requestedPayeeAgentId,
            providedPayeeAddress: resolvedPayeeAddress,
            derivedPayeeWallet,
            chain: chainId,
          };
        }
        resolvedPayeeAddress = resolvedPayeeAddress || derivedPayeeWallet;
      }

      if (!resolvedPayeeAddress) {
        return {
          success: false,
          error: 'payeeAddress or payeeAgentId is required',
        };
      }

      const x402 = getX402Api(commerce);
      const createdIntent = await x402.createIntent(
        buildCreateIntentInput({
          payerAddress: resolvedPayerAddress,
          payeeAddress: resolvedPayeeAddress,
          amount,
          asset,
          network: normalizedNetwork,
          cartId,
          orderId,
          description,
          validitySeconds,
        }),
      );
      const normalizedIntent = normalizeIntent(createdIntent);
      if (!normalizedIntent.id) {
        return { success: false, error: 'x402 createIntent did not return an intent id' };
      }
      const createdIntentId = String(normalizedIntent.id);

      let localSigning;
      try {
        localSigning = await signIntentWithLocalAgent({
          commerce,
          intentId: createdIntentId,
          agentId: effectivePayerAgentId,
          keyId,
          chain: chainId,
          configDir: '.stateset',
        });
      } catch (err) {
        return { success: false, error: `Local signing failed: ${messageFromError(err)}` };
      }

      const settlement = await handleSettleIntentOnchain({
        commerce,
        params: {
          intentId: createdIntentId,
          agentId: effectivePayerAgentId,
          payeeAgentId:
            shouldRecordIncoming && requestedPayeeAgentId ? requestedPayeeAgentId : undefined,
          chain: chainId,
          token,
        },
        allowApply,
        resolveTreasuryAgentId,
        treasuryContextOptions,
        buildAuditContext,
        buildTreasuryIdentityMetadata,
        extra,
      });

      if (!settlement?.success) {
        return {
          success: false,
          error: settlement?.error || 'Failed to settle x402 intent',
          intent: {
            id: createdIntentId,
            status: firstValue(localSigning.signed, ['status']),
          },
          signing: {
            mode: 'local_agent_key',
            agentId: localSigning.agentId,
            keyId: localSigning.keyId,
            publicKey: localSigning.publicKey,
          },
          settlement,
        };
      }

      return {
        success: true,
        message: 'x402 agentic payment executed end-to-end.',
        payment: {
          intentId: createdIntentId,
          payerAgentId: effectivePayerAgentId,
          payeeAgentId: requestedPayeeAgentId || null,
          payerAddress: resolvedPayerAddress,
          payeeAddress: resolvedPayeeAddress,
          amount,
          asset: asset || 'usdc',
          network: normalizedNetwork,
        },
        signing: {
          mode: 'local_agent_key',
          agentId: localSigning.agentId,
          keyId: localSigning.keyId,
          publicKey: localSigning.publicKey,
        },
        settlement: settlement.settlement || null,
        incomingSettlement: settlement.incomingSettlement || null,
        intent: settlement.intent || {
          id: createdIntentId,
          status: firstValue(localSigning.signed, ['status']),
        },
      };
    },
  },

  {
    name: 'x402_record_incoming_settlement',
    description:
      'Record a settled x402 intent as an incoming treasury deposit for a local payee agent.',
    inputSchema: {
      intentId: z.string().min(1).describe('Settled x402 payment intent ID'),
      payeeAgentId: z.string().min(1).describe('Local payee agent ID to credit'),
      chain: z.string().optional().describe('Optional chain override (default: intent network)'),
      token: z
        .string()
        .optional()
        .describe('Optional token override (default: inferred from intent asset)'),
    },
    permission: 'write',
    handler: async ({
      commerce,
      params,
      allowApply,
      treasuryContextOptions,
      buildAuditContext,
      extra,
    }) => {
      const { intentId, payeeAgentId, chain, token } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Recording incoming x402 settlement requires --apply flag.',
          wouldRecord: {
            intentId,
            payeeAgentId,
            chain: chain || null,
            token: token || null,
          },
          instruction: 'Run with --apply to record this incoming settlement',
        };
      }

      const x402 = getX402Api(commerce);
      const rawIntent = await x402.getIntent(intentId);
      if (!rawIntent) {
        return { success: false, error: 'Payment intent not found' };
      }

      const intent = normalizeIntent(rawIntent);
      if (!intent.payeeAddress) {
        return { success: false, error: 'Intent is missing payee address' };
      }

      const status = String(intent.status || '').toLowerCase();
      if (status !== 'settled' && !intent.txHash) {
        return {
          success: false,
          error: `Intent must be settled or include a transaction hash (current: ${intent.status || 'unknown'})`,
        };
      }
      if (!intent.txHash) {
        return {
          success: false,
          error: 'Intent is missing tx hash, cannot record incoming settlement',
        };
      }

      const {
        getToken,
        getDefaultStablecoin,
        getWalletAddress,
        getChain,
        listChains,
        fromSmallestUnit,
        isEvmChain,
      } = await loadChainsModule();
      const { loadTreasuryContext, recordDeposit } = await loadTreasuryModule();

      let chainId = normalizeX402NetworkToChain(chain || intent.network);
      if (
        (!chainId || !getChain(chainId)) &&
        intent.chainId !== undefined &&
        intent.chainId !== null
      ) {
        const numericChainId = Number(intent.chainId);
        if (Number.isFinite(numericChainId)) {
          chainId =
            listChains().find((id) => {
              const config = getChain(id);
              return config?.chainId === numericChainId;
            }) || chainId;
        }
      }

      if (!chainId || !getChain(chainId)) {
        return { success: false, error: 'Unable to determine target chain (provide --chain)' };
      }

      const inferredToken = token || normalizeAssetToToken(intent.asset);
      const tokenConfig = inferredToken
        ? getToken(chainId, inferredToken)
        : getDefaultStablecoin(chainId);
      if (!tokenConfig) {
        return {
          success: false,
          error: `Unable to resolve token for intent asset=${intent.asset || 'unknown'} on chain=${chainId}`,
        };
      }

      const normalizeAddress = createAddressNormalizer(chainId, isEvmChain);
      const payeeWallet = await getWalletAddress(payeeAgentId, chainId, {
        configDir: '.stateset',
      });
      if (normalizeAddress(payeeWallet) !== normalizeAddress(intent.payeeAddress)) {
        return {
          success: false,
          error: 'Payee agent wallet does not match intent payee address',
          expectedPayee: intent.payeeAddress,
          payeeAgentWallet: payeeWallet,
          chain: chainId,
          payeeAgentId,
        };
      }

      const amount =
        intent.amountDecimal !== undefined && intent.amountDecimal !== null
          ? String(intent.amountDecimal)
          : intent.amount !== undefined && intent.amount !== null
            ? fromSmallestUnit(BigInt(String(intent.amount)), tokenConfig.decimals)
            : null;
      if (!amount) {
        return { success: false, error: 'Intent amount is missing' };
      }
      const numericAmount = Number.parseFloat(String(amount));
      if (!Number.isFinite(numericAmount) || numericAmount <= 0) {
        return {
          success: false,
          error: `Intent amount must be greater than zero (resolved: ${amount})`,
        };
      }

      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const duplicate =
        typeof ctx.store.findByTx === 'function'
          ? ctx.store.findByTx({
              agentId: payeeAgentId,
              chainId,
              tokenSymbol: tokenConfig.symbol,
              direction: 'deposit',
              source: 'x402_settlement_incoming',
              txId: intent.txHash,
            })
          : null;

      if (duplicate) {
        return {
          success: true,
          recorded: false,
          message: 'Incoming settlement already recorded in treasury.',
          settlement: {
            intentId,
            payeeAgentId,
            chain: chainId,
            token: tokenConfig.symbol,
            amount,
            txHash: intent.txHash,
            blockNumber: intent.blockNumber ?? null,
          },
        };
      }

      const audit = buildAuditContext
        ? buildAuditContext(extra, 'x402_record_incoming_settlement')
        : {};
      const entry = await recordDeposit(
        {
          agentId: payeeAgentId,
          chainId,
          tokenSymbol: tokenConfig.symbol,
          amount,
          txId: intent.txHash,
          fromAddress: intent.payerAddress || null,
          source: 'x402_settlement_incoming',
          metadata: {
            x402IntentId: intentId,
            x402Nonce: intent.nonce ?? null,
            x402Asset: intent.asset ?? null,
            x402Network: intent.network ?? null,
          },
          ...audit,
        },
        ctx,
      );

      return {
        success: true,
        recorded: true,
        message: 'Incoming settlement recorded in treasury.',
        settlement: {
          intentId,
          payeeAgentId,
          chain: chainId,
          token: tokenConfig.symbol,
          amount,
          txHash: intent.txHash,
          blockNumber: intent.blockNumber ?? null,
        },
        treasuryEntry: {
          eventId: entry.event_id ?? null,
          direction: entry.direction ?? null,
          source: entry.source ?? null,
        },
      };
    },
  },

  {
    name: 'x402_mark_settled',
    description:
      'Mark an x402 payment intent as settled on-chain. Called after blockchain confirmation.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
      txHash: z.string().min(1).describe('On-chain transaction hash'),
      blockNumber: z.number().describe('Block number where settled'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { intentId, txHash, blockNumber } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Marking settled requires --apply flag.',
          wouldSettle: { intentId, txHash, blockNumber },
        };
      }

      const x402 = getX402Api(commerce);
      const settled = await x402.markSettled(intentId, txHash, blockNumber);
      return {
        success: true,
        message: 'Payment intent marked as settled.',
        intent: {
          id: settled.id,
          status: settled.status,
          txHash: firstValue(settled, ['txHash', 'tx_hash']),
          blockNumber: firstValue(settled, ['blockNumber', 'block_number']),
          settledAt: firstValue(settled, ['settledAt', 'settled_at']),
        },
      };
    },
  },

  {
    name: 'x402_get_next_nonce',
    description: 'Get the next nonce for a payer address. Used for replay protection.',
    inputSchema: {
      payerAddress: z.string().min(1).describe('Payer wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { payerAddress } = params;
      const x402 = getX402Api(commerce);
      const nonce = await x402.getNextNonce(payerAddress);
      return {
        success: true,
        payerAddress,
        nextNonce: nonce,
      };
    },
  },

  // x402 Credit Ledger Tools (Metered Billing)
  {
    name: 'x402_credit_balance',
    description: 'Get x402 credit balance for a payer (prepaid meter for streaming usage).',
    inputSchema: {
      payerAddress: z.string().min(1).describe('Payer wallet address'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { payerAddress, asset, network } = params;
      const x402 = getX402Api(commerce);
      const balance = await x402.getCreditBalance(
        buildCreditBalanceInput({ payerAddress, asset, network }),
      );
      return {
        success: true,
        payerAddress,
        asset: asset || 'usdc',
        network: network || 'set_chain',
        balance,
      };
    },
  },

  {
    name: 'x402_credit_deposit',
    description: 'Credit (deposit) x402 balance for metered usage. Requires --apply.',
    inputSchema: {
      payerAddress: z.string().min(1).describe('Payer wallet address'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      reason: z.string().optional().describe('Reason for deposit'),
      referenceId: z.string().optional().describe('Reference ID for audit'),
      metadata: z.string().optional().describe('Metadata (JSON string)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { payerAddress, amount, asset, network, reason, referenceId, metadata } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Depositing credit requires --apply flag.',
          wouldDeposit: { payerAddress, amount, asset, network },
        };
      }

      const x402 = getX402Api(commerce);
      const txn = normalizeCreditTransaction(
        await x402.creditAccount(
          buildCreditMutationInput({ payerAddress, amount, asset, network, reason, referenceId, metadata }),
        ),
      );
      return {
        success: true,
        message: 'Credit deposited.',
        transaction: {
          id: txn.id,
          accountId: txn.accountId,
          direction: txn.direction,
          amount: txn.amount,
          balanceAfter: txn.balanceAfter,
          createdAt: txn.createdAt,
        },
      };
    },
  },

  {
    name: 'x402_credit_debit',
    description: 'Debit x402 balance for metered usage. Requires --apply.',
    inputSchema: {
      payerAddress: z.string().min(1).describe('Payer wallet address'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      reason: z.string().optional().describe('Reason for debit'),
      referenceId: z.string().optional().describe('Reference ID for audit'),
      metadata: z.string().optional().describe('Metadata (JSON string)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { payerAddress, amount, asset, network, reason, referenceId, metadata } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Debiting credit requires --apply flag.',
          wouldDebit: { payerAddress, amount, asset, network },
        };
      }

      const x402 = getX402Api(commerce);
      const txn = normalizeCreditTransaction(
        await x402.debitAccount(
          buildCreditMutationInput({ payerAddress, amount, asset, network, reason, referenceId, metadata }),
        ),
      );
      return {
        success: true,
        message: 'Credit debited.',
        transaction: {
          id: txn.id,
          accountId: txn.accountId,
          direction: txn.direction,
          amount: txn.amount,
          balanceAfter: txn.balanceAfter,
          createdAt: txn.createdAt,
        },
      };
    },
  },

  {
    name: 'x402_credit_transactions',
    description: 'List x402 credit ledger transactions.',
    inputSchema: {
      payerAddress: z.string().optional().describe('Filter by payer address'),
      asset: z.string().optional().describe('Filter by asset'),
      network: z.string().optional().describe('Filter by network'),
      direction: z.string().optional().describe('Filter by direction: credit, debit'),
      limit: z.number().optional().describe('Maximum results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = getX402Api(commerce);
      const txns = /** @type {JsonRecord[]} */ (
        await x402.listCreditTransactions(buildCreditTransactionFilterInput(params))
      );
      return {
        success: true,
        count: txns.length,
        transactions: txns.map((txn) => normalizeCreditTransaction(txn)),
      };
    },
  },
];

export default x402Tools;
