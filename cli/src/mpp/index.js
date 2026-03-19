import { createHash, randomUUID } from 'node:crypto';

export const MPP_PROTOCOL = 'mpp';
export const MPP_VERSION = 'draft-2026-03-18';
export const MPP_HTTP_PAYMENT_REQUIRED_STATUS = 402;
export const MPP_JSONRPC_PAYMENT_REQUIRED_CODE = -32042;
export const MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE = 'Payment required';
export const MPP_SUPPORTED_INTENTS = ['charge', 'session'];

const DEFAULT_CHALLENGE_TTL_SECONDS = 300;

const METHOD_REGISTRY = {
  bitcoin: {
    id: 'bitcoin',
    settlementType: 'utxo',
    assets: ['BTC'],
    networks: ['bitcoin', 'bitcoin_testnet', 'btc'],
    privacy: 'public',
  },
  zcash: {
    id: 'zcash',
    settlementType: 'shielded_utxo',
    assets: ['ZEC'],
    networks: ['zcash', 'zcash_testnet', 'zec'],
    privacy: 'shielded',
  },
  tempo: {
    id: 'tempo',
    settlementType: 'account',
    assets: ['USDC', 'USDT', 'SSUSD', 'WSSUSD'],
    networks: ['tempo'],
    privacy: 'account',
  },
  stablecoin: {
    id: 'stablecoin',
    settlementType: 'account',
    assets: ['USDC', 'USDT', 'SSUSD', 'WSSUSD', 'DAI', 'ETH', 'SOL'],
    networks: ['set_chain', 'base', 'ethereum', 'solana'],
    privacy: 'account',
  },
};

function normalizeJsonValue(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeJsonValue(entry));
  }
  if (value && typeof value === 'object') {
    return Object.keys(value)
      .sort()
      .reduce((acc, key) => {
        acc[key] = normalizeJsonValue(value[key]);
        return acc;
      }, {});
  }
  return value;
}

function plusSeconds(date, seconds) {
  return new Date(date.getTime() + seconds * 1000);
}

function asNonEmptyString(value) {
  if (value === null || value === undefined) return null;
  const normalized = String(value).trim();
  return normalized.length > 0 ? normalized : null;
}

function asUpperString(value) {
  const normalized = asNonEmptyString(value);
  return normalized ? normalized.toUpperCase() : null;
}

function normalizePricingAmount(pricing = {}) {
  return {
    amount: pricing?.amount ?? null,
    amountSmallest:
      pricing?.amountSmallest !== undefined && pricing?.amountSmallest !== null
        ? String(pricing.amountSmallest)
        : null,
    asset: asUpperString(pricing?.tokenSymbol || pricing?.token?.symbol),
    network: asNonEmptyString(pricing?.chainId || pricing?.network),
    decimals: pricing?.token?.decimals ?? null,
    tokenAddress: pricing?.token?.address || null,
  };
}

export function stableStringify(value) {
  return JSON.stringify(normalizeJsonValue(value));
}

export function sha256Hex(value) {
  return createHash('sha256').update(String(value)).digest('hex');
}

export function listPaymentMethodAdapters() {
  return Object.values(METHOD_REGISTRY).map((method) => ({
    ...method,
    assets: [...method.assets],
    networks: [...method.networks],
  }));
}

export function resolvePaymentMethodAdapter({ chainId, tokenSymbol, network } = {}) {
  const normalizedChain = String(chainId || network || '')
    .trim()
    .toLowerCase();
  const normalizedToken = String(tokenSymbol || '')
    .trim()
    .toUpperCase();

  if (
    normalizedChain.includes('bitcoin') ||
    normalizedChain === 'btc' ||
    normalizedToken === 'BTC'
  ) {
    return METHOD_REGISTRY.bitcoin;
  }

  if (normalizedChain.includes('zcash') || normalizedChain === 'zec' || normalizedToken === 'ZEC') {
    return METHOD_REGISTRY.zcash;
  }

  if (normalizedChain.includes('tempo')) {
    return METHOD_REGISTRY.tempo;
  }

  return METHOD_REGISTRY.stablecoin;
}

export function buildMppServiceInfo({
  serviceId = 'stateset-commerce-mcp',
  serviceName = 'StateSet Commerce MCP',
  version = '1.0.0',
  serverName = 'stateset-commerce',
  serverUrl = '/mcp',
  transportType = 'mcp-jsonrpc',
} = {}) {
  const transport =
    transportType === 'http'
      ? {
          type: 'http',
          serverName,
          serverUrl,
          paymentRequired: {
            status: MPP_HTTP_PAYMENT_REQUIRED_STATUS,
            header: 'payment-required',
          },
          credentialHeader: 'payment',
          receiptHeader: 'payment-response',
        }
      : {
          type: 'mcp-jsonrpc',
          serverName,
          serverUrl,
          paymentRequired: {
            code: MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
            message: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
          },
          credentialMetaKey: 'payment',
          receiptMetaKey: 'payment',
        };

  return {
    id: serviceId,
    name: serviceName,
    version,
    protocol: MPP_PROTOCOL,
    protocolVersion: MPP_VERSION,
    transport,
    discovery: {
      openapiExtension: 'x-payment-info',
      serviceInfoExtension: 'x-service-info',
      paymentRequiredHttpStatus: MPP_HTTP_PAYMENT_REQUIRED_STATUS,
      canonicalOpenapiPath: '/openapi.json',
      serviceInfoPath: '/.well-known/service-info',
    },
    intents: [...MPP_SUPPORTED_INTENTS],
    methods: listPaymentMethodAdapters().map((method) => ({
      id: method.id,
      assets: [...method.assets],
      networks: [...method.networks],
      settlementType: method.settlementType,
    })),
  };
}

export function buildPaymentInfoFromPricing({
  toolName,
  description = '',
  pricing,
  intent = 'charge',
} = {}) {
  if (!pricing) return null;
  const amount = normalizePricingAmount(pricing);
  const method = resolvePaymentMethodAdapter({
    chainId: amount.network,
    tokenSymbol: amount.asset,
  });
  return {
    protocol: MPP_PROTOCOL,
    protocolVersion: MPP_VERSION,
    intent,
    tool: toolName || null,
    description,
    required: true,
    amount,
    methods: [
      {
        kind: method.id,
        network: amount.network,
        asset: amount.asset,
        settlementType: method.settlementType,
      },
    ],
    jsonrpc: {
      paymentRequiredCode: MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
      credentialMetaKey: 'payment',
      receiptMetaKey: 'payment',
    },
    http: {
      paymentRequiredStatus: MPP_HTTP_PAYMENT_REQUIRED_STATUS,
    },
  };
}

export function createPaymentChallenge({
  toolName,
  description = '',
  pricing,
  params = {},
  requestId = null,
  sessionId = null,
  intent = 'charge',
  ttlSeconds = DEFAULT_CHALLENGE_TTL_SECONDS,
  serviceId = 'stateset-commerce-mcp',
  serviceName = 'StateSet Commerce MCP',
  challengeId = null,
  metadata = null,
} = {}) {
  if (!pricing) {
    throw new Error('pricing is required to create a payment challenge');
  }

  const now = new Date();
  const expiresAt = plusSeconds(now, ttlSeconds);
  const amount = normalizePricingAmount(pricing);
  const method = resolvePaymentMethodAdapter({
    chainId: amount.network,
    tokenSymbol: amount.asset,
  });
  const binding = {
    algorithm: 'sha256',
    requestHash: sha256Hex(
      stableStringify({
        toolName,
        requestId,
        sessionId,
        params,
        amount,
      }),
    ),
    fields: ['toolName', 'requestId', 'sessionId', 'params', 'amount'],
  };
  const resolvedChallengeId =
    challengeId ||
    `mpp_${sha256Hex(
      stableStringify({
        serviceId,
        toolName,
        requestId,
        sessionId,
        intent,
        amount,
        binding,
      }),
    ).slice(0, 32)}`;

  return {
    protocol: MPP_PROTOCOL,
    protocolVersion: MPP_VERSION,
    type: 'challenge',
    challengeId: resolvedChallengeId,
    service: {
      id: serviceId,
      name: serviceName,
    },
    tool: toolName || null,
    description,
    intent,
    createdAt: now.toISOString(),
    expiresAt: expiresAt.toISOString(),
    requestId: requestId || null,
    sessionId: sessionId || null,
    amount,
    paymentMethods: [
      {
        kind: method.id,
        asset: amount.asset,
        network: amount.network,
        settlementType: method.settlementType,
      },
    ],
    binding,
    metadata: metadata || null,
  };
}

export function createPaymentCredential({
  challenge,
  payer = null,
  method = null,
  authorization = null,
  proof = null,
  metadata = null,
} = {}) {
  if (!challenge?.challengeId) {
    throw new Error('challenge is required to create a payment credential');
  }

  const selectedMethod =
    method || (Array.isArray(challenge.paymentMethods) ? challenge.paymentMethods[0] : null);

  return {
    protocol: MPP_PROTOCOL,
    protocolVersion: MPP_VERSION,
    type: 'credential',
    credentialId: randomUUID(),
    challengeId: challenge.challengeId,
    payer: payer || null,
    method: selectedMethod
      ? {
          kind: selectedMethod.kind || null,
          asset: selectedMethod.asset || null,
          network: selectedMethod.network || null,
        }
      : null,
    amount: challenge.amount || null,
    binding: challenge.binding || null,
    authorization: authorization || null,
    proof: proof || null,
    createdAt: new Date().toISOString(),
    metadata: metadata || null,
  };
}

export function extractPaymentCredential(params = {}, extra = {}) {
  const candidates = [
    extra?._meta?.payment,
    extra?.meta?.payment,
    extra?.payment,
    params?._meta?.payment,
    params?.paymentCredential,
  ];
  for (const candidate of candidates) {
    if (candidate && typeof candidate === 'object' && !Array.isArray(candidate)) {
      return candidate;
    }
  }
  return null;
}

export function verifyPaymentCredential(credential, challenge) {
  if (!credential || typeof credential !== 'object') {
    return { valid: false, reason: 'Missing payment credential' };
  }
  if (!challenge?.challengeId) {
    return { valid: false, reason: 'Missing payment challenge' };
  }
  if (credential.protocol && credential.protocol !== MPP_PROTOCOL) {
    return { valid: false, reason: 'Unsupported payment protocol' };
  }
  if (credential.challengeId !== challenge.challengeId) {
    return { valid: false, reason: 'Credential challenge does not match' };
  }

  const expiresAt = Date.parse(challenge.expiresAt || '');
  if (Number.isFinite(expiresAt) && Date.now() > expiresAt) {
    return { valid: false, reason: 'Payment challenge has expired' };
  }

  const expectedHash = challenge?.binding?.requestHash || null;
  const receivedHash = credential?.binding?.requestHash || expectedHash;
  if (expectedHash && receivedHash !== expectedHash) {
    return { valid: false, reason: 'Credential request binding does not match' };
  }

  const expectedAmount = challenge?.amount?.amountSmallest || null;
  const receivedAmount = credential?.amount?.amountSmallest || expectedAmount;
  if (expectedAmount && receivedAmount !== expectedAmount) {
    return { valid: false, reason: 'Credential amount does not match challenge amount' };
  }

  const supportedKinds = new Set(
    Array.isArray(challenge.paymentMethods)
      ? challenge.paymentMethods.map((entry) => entry?.kind).filter(Boolean)
      : [],
  );
  const methodKind = credential?.method?.kind || null;
  if (methodKind && supportedKinds.size > 0 && !supportedKinds.has(methodKind)) {
    return { valid: false, reason: 'Credential payment method is not accepted' };
  }

  return {
    valid: true,
    credential: {
      ...credential,
      protocol: MPP_PROTOCOL,
      protocolVersion: credential.protocolVersion || MPP_VERSION,
      type: credential.type || 'credential',
    },
  };
}

export function createPaymentReceipt({
  challenge,
  credential = null,
  charge = null,
  toolName = null,
  requestId = null,
  sessionId = null,
  settlement = null,
  metadata = null,
} = {}) {
  if (!challenge?.challengeId) {
    throw new Error('challenge is required to create a payment receipt');
  }

  return {
    protocol: MPP_PROTOCOL,
    protocolVersion: MPP_VERSION,
    type: 'receipt',
    receiptId: randomUUID(),
    challengeId: challenge.challengeId,
    credentialId: credential?.credentialId || null,
    tool: toolName || challenge.tool || null,
    requestId: requestId || challenge.requestId || null,
    sessionId: sessionId || challenge.sessionId || null,
    createdAt: new Date().toISOString(),
    amount: challenge.amount || null,
    settlement:
      settlement ||
      (charge?.rule
        ? {
            network: charge.rule.chainId || null,
            asset: charge.rule.tokenSymbol || null,
            amount: charge.rule.amount ?? null,
          }
        : null),
    payer: credential?.payer || null,
    service: challenge.service || null,
    metadata: {
      ...(metadata || {}),
      chargeRecorded: Boolean(charge?.charged),
    },
  };
}

export function buildPaymentRequiredPayload({
  challenge,
  reason = MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  validationError = null,
} = {}) {
  if (!challenge?.challengeId) {
    throw new Error('challenge is required to build a payment-required payload');
  }

  return {
    error: reason,
    code: MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
    paymentRequired: true,
    paymentChallenge: challenge,
    acceptedPaymentMethods: challenge.paymentMethods || [],
    _meta: {
      payment: {
        protocol: MPP_PROTOCOL,
        protocolVersion: MPP_VERSION,
        challenge,
        acceptedPaymentMethods: challenge.paymentMethods || [],
        validationError,
      },
    },
  };
}

export function attachPaymentMetadata(payload, paymentMetadata = {}) {
  const base =
    payload && typeof payload === 'object' && !Array.isArray(payload)
      ? { ...payload }
      : { result: payload };
  return {
    ...base,
    _meta: {
      ...(base._meta || {}),
      payment: {
        ...(base._meta?.payment || {}),
        ...(paymentMetadata || {}),
      },
    },
  };
}

export function buildHttpPaymentRequiredResponse({ challenge, serviceInfo = null } = {}) {
  return {
    status: MPP_HTTP_PAYMENT_REQUIRED_STATUS,
    headers: {
      'content-type': 'application/json',
    },
    body: {
      error: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
      paymentChallenge: challenge,
      acceptedPaymentMethods: challenge?.paymentMethods || [],
      service: serviceInfo || null,
    },
  };
}

function normalizeArray(values = []) {
  const list = Array.isArray(values)
    ? values
    : values === null || values === undefined
      ? []
      : [values];
  return list
    .map((value) => asNonEmptyString(value))
    .filter(Boolean)
    .map((value) => String(value));
}

function normalizeUpperArray(values = []) {
  return normalizeArray(values).map((value) => value.toUpperCase());
}

function toBigIntOrNull(value) {
  if (value === null || value === undefined || value === '') return null;
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return null;
    return BigInt(Math.trunc(value));
  }
  const normalized = String(value).trim();
  if (!/^-?\d+$/.test(normalized)) return null;
  try {
    return BigInt(normalized);
  } catch {
    return null;
  }
}

function selectPaymentMethod(challenge, requestedMethod = null) {
  const paymentMethods = Array.isArray(challenge?.paymentMethods) ? challenge.paymentMethods : [];
  if (!requestedMethod) {
    return paymentMethods[0] || null;
  }

  if (typeof requestedMethod === 'string') {
    const normalized = requestedMethod.trim().toLowerCase();
    return (
      paymentMethods.find(
        (entry) =>
          String(entry?.kind || '')
            .trim()
            .toLowerCase() === normalized ||
          String(entry?.network || '')
            .trim()
            .toLowerCase() === normalized ||
          String(entry?.asset || '')
            .trim()
            .toLowerCase() === normalized,
      ) ||
      paymentMethods[0] || {
        kind: requestedMethod,
      }
    );
  }

  if (requestedMethod && typeof requestedMethod === 'object' && !Array.isArray(requestedMethod)) {
    const requestedKind = String(requestedMethod.kind || '')
      .trim()
      .toLowerCase();
    const requestedNetwork = String(requestedMethod.network || '')
      .trim()
      .toLowerCase();
    const requestedAsset = String(requestedMethod.asset || '')
      .trim()
      .toLowerCase();
    const matchedMethod = paymentMethods.find((entry) => {
      if (
        requestedKind &&
        String(entry?.kind || '')
          .trim()
          .toLowerCase() !== requestedKind
      ) {
        return false;
      }
      if (
        requestedNetwork &&
        String(entry?.network || '')
          .trim()
          .toLowerCase() !== requestedNetwork
      ) {
        return false;
      }
      if (
        requestedAsset &&
        String(entry?.asset || '')
          .trim()
          .toLowerCase() !== requestedAsset
      ) {
        return false;
      }
      return true;
    });
    return matchedMethod || requestedMethod;
  }

  return paymentMethods[0] || null;
}

async function resolveAsyncValue(value, context) {
  if (typeof value === 'function') {
    return value(context);
  }
  return value;
}

export class MppPaymentPolicyError extends Error {
  constructor(message, challenge = null, policy = null) {
    super(message);
    this.name = 'MppPaymentPolicyError';
    this.challenge = challenge || null;
    this.policy = policy || null;
  }
}

export function extractPaymentChallenge(payload = null) {
  const visited = new Set();
  const queue = [payload];

  while (queue.length > 0) {
    const candidate = queue.shift();
    if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) {
      continue;
    }
    if (visited.has(candidate)) {
      continue;
    }
    visited.add(candidate);

    if (candidate?.paymentChallenge?.challengeId) {
      return candidate.paymentChallenge;
    }
    if (candidate?._meta?.payment?.challenge?.challengeId) {
      return candidate._meta.payment.challenge;
    }
    if (candidate?.challenge?.challengeId) {
      return candidate.challenge;
    }

    if (candidate.result && typeof candidate.result === 'object') {
      queue.push(candidate.result);
    }
    if (candidate.error && typeof candidate.error === 'object') {
      queue.push(candidate.error);
    }
    if (candidate.charge && typeof candidate.charge === 'object') {
      queue.push(candidate.charge);
    }
  }

  return null;
}

export function validatePaymentChallenge(challenge, policy = {}) {
  if (!challenge?.challengeId) {
    return { valid: false, reason: 'Missing payment challenge' };
  }

  const expiresAt = Date.parse(challenge.expiresAt || '');
  if (Number.isFinite(expiresAt) && Date.now() > expiresAt) {
    return { valid: false, reason: 'Payment challenge has expired' };
  }

  const acceptedMethods = normalizeArray(policy.acceptedMethods);
  if (acceptedMethods.length > 0) {
    const availableMethods = normalizeArray(
      (challenge.paymentMethods || []).map((entry) => entry?.kind),
    ).map((value) => value.toLowerCase());
    const accepted = acceptedMethods.map((value) => value.toLowerCase());
    if (!accepted.some((value) => availableMethods.includes(value))) {
      return { valid: false, reason: 'Payment method is not allowed by policy' };
    }
  }

  const acceptedAssets = normalizeUpperArray(policy.acceptedAssets);
  if (acceptedAssets.length > 0) {
    const availableAssets = normalizeUpperArray([
      challenge.amount?.asset,
      ...(challenge.paymentMethods || []).map((entry) => entry?.asset),
    ]);
    if (!acceptedAssets.some((value) => availableAssets.includes(value))) {
      return { valid: false, reason: 'Payment asset is not allowed by policy' };
    }
  }

  const acceptedNetworks = normalizeArray(policy.acceptedNetworks).map((value) =>
    value.toLowerCase(),
  );
  if (acceptedNetworks.length > 0) {
    const availableNetworks = normalizeArray([
      challenge.amount?.network,
      ...(challenge.paymentMethods || []).map((entry) => entry?.network),
    ]).map((value) => value.toLowerCase());
    if (!acceptedNetworks.some((value) => availableNetworks.includes(value))) {
      return { valid: false, reason: 'Payment network is not allowed by policy' };
    }
  }

  const acceptedIntents = normalizeArray(policy.acceptedIntents).map((value) =>
    value.toLowerCase(),
  );
  if (
    acceptedIntents.length > 0 &&
    !acceptedIntents.includes(
      String(challenge.intent || '')
        .trim()
        .toLowerCase(),
    )
  ) {
    return { valid: false, reason: 'Payment intent is not allowed by policy' };
  }

  const maxAmountSmallest = toBigIntOrNull(policy.maxAmountSmallest);
  const challengeAmountSmallest = toBigIntOrNull(challenge.amount?.amountSmallest);
  if (
    maxAmountSmallest !== null &&
    challengeAmountSmallest !== null &&
    challengeAmountSmallest > maxAmountSmallest
  ) {
    return {
      valid: false,
      reason: `Payment amount exceeds maxAmountSmallest ${String(maxAmountSmallest)}`,
    };
  }

  if (policy.maxAmount !== undefined && policy.maxAmount !== null) {
    const maximum = Number(policy.maxAmount);
    const actual = Number(challenge.amount?.amount);
    if (Number.isFinite(maximum) && Number.isFinite(actual) && actual > maximum) {
      return { valid: false, reason: `Payment amount exceeds maxAmount ${maximum}` };
    }
  }

  return { valid: true };
}

export function buildPaymentRetryExtra({
  extra = {},
  challenge,
  payer = null,
  method = null,
  authorization = null,
  proof = null,
  metadata = null,
  credential = null,
} = {}) {
  if (!challenge?.challengeId && !credential?.challengeId) {
    throw new Error('challenge or credential is required to build payment retry metadata');
  }

  const resolvedCredential =
    credential ||
    createPaymentCredential({
      challenge,
      payer,
      method: selectPaymentMethod(challenge, method),
      authorization,
      proof,
      metadata,
    });

  return {
    ...(extra || {}),
    _meta: {
      ...((extra || {})._meta || {}),
      payment: resolvedCredential,
    },
  };
}

export async function executeMppToolWithPayment({
  executor,
  toolName,
  params = {},
  executionOptions = {},
  payment = {},
} = {}) {
  if (typeof executor !== 'function') {
    throw new Error('executor is required');
  }

  const initialResult = await executor(toolName, params, executionOptions);
  const challenge = extractPaymentChallenge(initialResult);
  if (!challenge) {
    return initialResult;
  }

  const validation = validatePaymentChallenge(challenge, payment);
  if (!validation.valid) {
    throw new MppPaymentPolicyError(validation.reason, challenge, payment);
  }

  const baseContext = {
    toolName,
    params,
    executionOptions,
    initialResult,
    challenge,
    payment,
  };

  const decision =
    typeof payment?.onChallenge === 'function' ? await payment.onChallenge(baseContext) : null;
  if (decision === false) {
    return initialResult;
  }

  const resolvedPayment =
    decision && typeof decision === 'object' && !Array.isArray(decision)
      ? { ...payment, ...decision }
      : payment;
  const challengeValidation = validatePaymentChallenge(challenge, resolvedPayment);
  if (!challengeValidation.valid) {
    throw new MppPaymentPolicyError(challengeValidation.reason, challenge, resolvedPayment);
  }

  const resolutionContext = {
    ...baseContext,
    payment: resolvedPayment,
  };
  const resolvedCredential = await resolveAsyncValue(resolvedPayment.credential, resolutionContext);
  const resolvedPayer = await resolveAsyncValue(resolvedPayment.payer, resolutionContext);
  const resolvedAuthorization = (await resolveAsyncValue(
    resolvedPayment.authorization,
    resolutionContext,
  )) || {
    type: 'mpp:auto',
  };
  const resolvedProof = await resolveAsyncValue(resolvedPayment.proof, resolutionContext);
  const resolvedMetadata = await resolveAsyncValue(resolvedPayment.metadata, resolutionContext);
  const retryExtra = buildPaymentRetryExtra({
    extra: executionOptions.extra,
    challenge,
    payer: resolvedPayer,
    method: resolvedPayment.method,
    authorization: resolvedAuthorization,
    proof: resolvedProof,
    metadata: resolvedMetadata,
    credential: resolvedCredential,
  });

  return executor(toolName, params, {
    ...executionOptions,
    requestId: initialResult?.requestId || challenge.requestId || executionOptions.requestId,
    sessionId: initialResult?.sessionId || challenge.sessionId || executionOptions.sessionId,
    extra: retryExtra,
  });
}

export function createPaymentDiscoveryDocument({
  serviceInfo,
  tools = [],
  serverUrl = '/mcp',
} = {}) {
  const resolvedService = serviceInfo || buildMppServiceInfo({ serverUrl });
  const paths = {};

  for (const tool of tools) {
    const pathKey = `/mcp/tools/${tool.name}`;
    const operation = {
      operationId: `mcp_${String(tool.name || '').replace(/[^a-zA-Z0-9_]/g, '_')}`,
      summary: tool.description || tool.name,
      description: tool.description || '',
      tags: [tool.runtime?.policyDomain || 'commerce'],
      requestBody: {
        required: false,
        content: {
          'application/json': {
            schema: {
              type: 'object',
              properties: {
                jsonrpc: { type: 'string', example: '2.0' },
                id: {},
                method: { type: 'string', example: tool.name },
                params: tool.inputSchema || { type: 'object', additionalProperties: true },
              },
              required: ['method'],
            },
          },
        },
      },
      responses: {
        200: {
          description: 'Successful MCP tool execution',
        },
        402: {
          description: 'Payment challenge required before execution',
          content: {
            'application/json': {
              schema: {
                type: 'object',
                properties: {
                  error: { type: 'string' },
                  code: { type: 'integer', example: MPP_JSONRPC_PAYMENT_REQUIRED_CODE },
                  paymentChallenge: { type: 'object' },
                },
              },
            },
          },
        },
      },
    };

    if (tool.paymentInfo) {
      operation['x-payment-info'] = tool.paymentInfo;
    }

    paths[pathKey] = { post: operation };
  }

  return {
    openapi: '3.1.0',
    info: {
      title: `${resolvedService.name} Payment Discovery`,
      version: resolvedService.version,
      description: 'Machine Payments Protocol discovery document for payable MCP tools.',
    },
    servers: [{ url: serverUrl }],
    'x-service-info': resolvedService,
    paths,
  };
}
