import { validateFetchUrl } from '../utils/url-validator.js';
import {
  MPP_PROTOCOL,
  MPP_VERSION,
  MppPaymentPolicyError,
  createPaymentCredential,
  extractPaymentChallenge,
  validatePaymentChallenge,
} from './index.js';

const HTTP_METHODS = new Set(['get', 'post', 'put', 'patch', 'delete', 'head', 'options']);

function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

function headersToObject(headers) {
  if (!headers) return {};
  if (typeof Headers !== 'undefined' && headers instanceof Headers) {
    return Object.fromEntries(headers.entries());
  }
  if (Array.isArray(headers)) {
    return Object.fromEntries(headers.map(([key, value]) => [key, String(value)]));
  }
  if (headers instanceof Map) {
    return Object.fromEntries([...headers.entries()].map(([key, value]) => [key, String(value)]));
  }
  return Object.fromEntries(Object.entries(headers).map(([key, value]) => [key, String(value)]));
}

function readHeader(response, name) {
  if (!response?.headers) return null;
  if (typeof response.headers.get === 'function') {
    return response.headers.get(name) || response.headers.get(name.toLowerCase()) || null;
  }
  if (response.headers instanceof Map) {
    return response.headers.get(name) || response.headers.get(name.toLowerCase()) || null;
  }
  const direct =
    response.headers[name] ??
    response.headers[name.toLowerCase()] ??
    response.headers[String(name).toLowerCase()];
  return direct === undefined || direct === null ? null : String(direct);
}

function decodeHeaderPayload(value) {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  if (!trimmed) return null;

  try {
    return JSON.parse(trimmed);
  } catch (_err) {
    void _err;
  }

  try {
    return JSON.parse(Buffer.from(trimmed, 'base64url').toString('utf8'));
  } catch (_err) {
    void _err;
  }

  try {
    return JSON.parse(Buffer.from(trimmed, 'base64').toString('utf8'));
  } catch (_err) {
    void _err;
  }

  return null;
}

function encodeCredentialHeader(credential) {
  return Buffer.from(JSON.stringify(credential), 'utf8').toString('base64url');
}

function normalizeUrl(url) {
  if (!url || typeof url !== 'string') {
    throw new Error('url is required');
  }
  return new URL(url).toString();
}

function resolveUrl(baseUrl, path = '/') {
  if (path && /^https?:\/\//i.test(String(path))) {
    return normalizeUrl(String(path));
  }
  return new URL(String(path || '/'), normalizeUrl(baseUrl)).toString();
}

function isPlainObject(value) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function prepareRequestOptions(options = {}) {
  const prepared = {
    ...options,
    headers: headersToObject(options.headers),
  };

  if (isPlainObject(prepared.body)) {
    prepared.body = JSON.stringify(prepared.body);
    if (!prepared.headers['content-type'] && !prepared.headers['Content-Type']) {
      prepared.headers['content-type'] = 'application/json';
    }
  }

  return prepared;
}

async function parseJsonSafely(response) {
  if (!response || typeof response !== 'object') return null;
  const target = typeof response.clone === 'function' ? response.clone() : response;

  if (typeof target.json !== 'function') {
    return null;
  }

  try {
    return await target.json();
  } catch (error) {
    console.debug('[mpp-agent] Failed to parse payment payload:', messageFromError(error));
    return null;
  }
}

async function parseJsonResponse(response, label = 'HTTP response') {
  const body = await parseJsonSafely(response);
  if (body === null) {
    throw new Error(`${label} did not return valid JSON`);
  }
  return body;
}

function attachPaymentContext(response, context = {}) {
  if (!response || typeof response !== 'object') {
    return response;
  }

  try {
    response.mpp = {
      ...(response.mpp || {}),
      ...context,
    };
  } catch (_err) {
    void _err;
  }

  return response;
}

function filterPayableRoute(route, filters = {}) {
  const intent = String(filters.intent || '')
    .trim()
    .toLowerCase();
  const asset = String(filters.asset || '')
    .trim()
    .toUpperCase();
  const network = String(filters.network || '')
    .trim()
    .toLowerCase();
  const method = String(filters.method || '')
    .trim()
    .toLowerCase();
  const path = String(filters.path || '').trim();

  if (
    intent &&
    String(route.paymentInfo?.intent || '')
      .trim()
      .toLowerCase() !== intent
  ) {
    return false;
  }
  if (
    asset &&
    String(route.paymentInfo?.amount?.asset || '')
      .trim()
      .toUpperCase() !== asset
  ) {
    return false;
  }
  if (
    network &&
    String(route.paymentInfo?.amount?.network || '')
      .trim()
      .toLowerCase() !== network
  ) {
    return false;
  }
  if (
    method &&
    String(route.method || '')
      .trim()
      .toLowerCase() !== method
  ) {
    return false;
  }
  if (path && String(route.path || '').trim() !== path) {
    return false;
  }

  return true;
}

export function extractPayableHttpRoutes(document = {}, filters = {}) {
  const routes = [];
  const paths = document?.paths && typeof document.paths === 'object' ? document.paths : {};
  const serviceInfo = document?.['x-service-info'] || null;

  for (const [path, operations] of Object.entries(paths)) {
    if (!operations || typeof operations !== 'object' || Array.isArray(operations)) {
      continue;
    }

    for (const [method, operation] of Object.entries(operations)) {
      const normalizedMethod = String(method || '')
        .trim()
        .toLowerCase();
      if (!HTTP_METHODS.has(normalizedMethod)) {
        continue;
      }
      if (!operation || typeof operation !== 'object' || Array.isArray(operation)) {
        continue;
      }
      if (!operation['x-payment-info']) {
        continue;
      }

      const route = {
        path,
        method: normalizedMethod.toUpperCase(),
        operationId: operation.operationId || null,
        summary: operation.summary || null,
        description: operation.description || null,
        tags: Array.isArray(operation.tags) ? [...operation.tags] : [],
        paymentInfo: operation['x-payment-info'],
        pluginId: operation['x-stateset-plugin-id'] || null,
        serviceInfo,
      };

      if (filterPayableRoute(route, filters)) {
        routes.push(route);
      }
    }
  }

  return routes;
}

export async function fetchMppServiceInfo(baseUrl, config = {}) {
  const { fetch: providedFetch, validateUrl = true, serviceInfoPath = null } = config || {};
  const endpoint = resolveUrl(baseUrl, serviceInfoPath || '/.well-known/service-info');

  if (validateUrl !== false) {
    validateFetchUrl(endpoint);
  }

  const fetchImpl = providedFetch || globalThis.fetch;
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetch implementation is required');
  }

  const response = await fetchImpl(endpoint, {
    method: 'GET',
    headers: {
      accept: 'application/json',
    },
  });
  if (!response?.ok) {
    throw new Error(`Failed to fetch MPP service info: HTTP ${response?.status ?? 'unknown'}`);
  }

  const serviceInfo = await parseJsonResponse(response, 'MPP service info');
  return {
    url: endpoint,
    serviceInfo,
  };
}

export async function fetchMppDiscoveryDocument(baseUrl, config = {}) {
  const { fetch: providedFetch, validateUrl = true, openapiPath = null } = config || {};
  const endpoint = resolveUrl(baseUrl, openapiPath || '/openapi.json');

  if (validateUrl !== false) {
    validateFetchUrl(endpoint);
  }

  const fetchImpl = providedFetch || globalThis.fetch;
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetch implementation is required');
  }

  const response = await fetchImpl(endpoint, {
    method: 'GET',
    headers: {
      accept: 'application/json',
    },
  });
  if (!response?.ok) {
    throw new Error(
      `Failed to fetch MPP discovery document: HTTP ${response?.status ?? 'unknown'}`,
    );
  }

  const document = await parseJsonResponse(response, 'MPP discovery document');
  return {
    url: endpoint,
    document,
  };
}

export async function discoverMppHttpService(baseUrl, config = {}) {
  const normalizedBaseUrl = normalizeUrl(baseUrl);
  const serviceInfoResult = await fetchMppServiceInfo(normalizedBaseUrl, config);
  const serviceInfo = serviceInfoResult.serviceInfo || null;
  if (serviceInfo?.protocol && serviceInfo.protocol !== MPP_PROTOCOL) {
    throw new Error(`Unsupported payment protocol: ${serviceInfo.protocol}`);
  }

  const canonicalOpenapiPath =
    config?.openapiPath || serviceInfo?.discovery?.canonicalOpenapiPath || '/openapi.json';
  const discoveryResult = await fetchMppDiscoveryDocument(normalizedBaseUrl, {
    ...config,
    openapiPath: canonicalOpenapiPath,
  });
  const document = discoveryResult.document || {};
  const resolvedServiceInfo = document['x-service-info'] || serviceInfo;

  return {
    baseUrl: normalizedBaseUrl,
    serviceInfoUrl: serviceInfoResult.url,
    discoveryUrl: discoveryResult.url,
    serviceInfo: resolvedServiceInfo || null,
    discoveryDocument: document,
    payableRoutes: extractPayableHttpRoutes(document, config),
  };
}

export async function extractHttpPaymentChallenge(response) {
  const headerPayload = decodeHeaderPayload(readHeader(response, 'payment-required'));
  const bodyPayload = await parseJsonSafely(response);
  return (
    extractPaymentChallenge(headerPayload) ||
    extractPaymentChallenge(bodyPayload) ||
    (headerPayload?.challengeId ? headerPayload : null) ||
    (bodyPayload?.challengeId ? bodyPayload : null)
  );
}

export async function extractHttpPaymentReceipt(response) {
  const headerPayload = decodeHeaderPayload(readHeader(response, 'payment-response'));
  if (headerPayload?.receipt?.receiptId) {
    return headerPayload.receipt;
  }
  if (headerPayload?.receiptId) {
    return headerPayload;
  }

  const bodyPayload = await parseJsonSafely(response);
  return bodyPayload?._meta?.payment?.receipt || null;
}

async function resolveMaybeFunction(value, context) {
  if (typeof value === 'function') {
    return value(context);
  }
  return value;
}

async function buildCredentialFromPolicy(challenge, payment = {}, context = {}) {
  const providedCredential = await resolveMaybeFunction(payment.credential, context);
  if (providedCredential) {
    return providedCredential;
  }

  const payer = await resolveMaybeFunction(payment.payer, context);
  const authorization = (await resolveMaybeFunction(payment.authorization, context)) || {
    type: 'mpp:http:auto',
  };
  const proof = await resolveMaybeFunction(payment.proof, context);
  const metadata = await resolveMaybeFunction(payment.metadata, context);
  const method = await resolveMaybeFunction(payment.method, context);

  return createPaymentCredential({
    challenge,
    payer,
    authorization,
    proof,
    metadata,
    method,
  });
}

export async function mppFetch(url, options = {}, config = {}) {
  const {
    fetch: providedFetch,
    validateUrl = true,
    requireReceipt = false,
    ...payment
  } = config || {};

  if (validateUrl !== false) {
    validateFetchUrl(url);
  }

  const fetchImpl = providedFetch || globalThis.fetch;
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetch implementation is required');
  }

  const preparedOptions = prepareRequestOptions(options);
  const firstResponse = await fetchImpl(url, preparedOptions);
  if (firstResponse?.status !== 402) {
    return attachPaymentContext(firstResponse, {
      receipt: await extractHttpPaymentReceipt(firstResponse),
    });
  }

  const challenge = await extractHttpPaymentChallenge(firstResponse);
  if (!challenge) {
    throw new Error('Failed to parse MPP payment challenge');
  }

  const validation = validatePaymentChallenge(challenge, payment);
  if (!validation.valid) {
    throw new MppPaymentPolicyError(validation.reason, challenge, payment);
  }

  const onChallengeContext = {
    url,
    options: preparedOptions,
    challenge,
    response: firstResponse,
    payment,
  };
  const decision =
    typeof payment.onChallenge === 'function'
      ? await payment.onChallenge(onChallengeContext)
      : null;
  if (decision === false) {
    return attachPaymentContext(firstResponse, { challenge });
  }

  const resolvedPayment = isPlainObject(decision)
    ? {
        ...payment,
        ...decision,
      }
    : payment;
  const challengeValidation = validatePaymentChallenge(challenge, resolvedPayment);
  if (!challengeValidation.valid) {
    throw new MppPaymentPolicyError(challengeValidation.reason, challenge, resolvedPayment);
  }

  const credential = await buildCredentialFromPolicy(challenge, resolvedPayment, {
    ...onChallengeContext,
    payment: resolvedPayment,
  });
  const retryHeaders = {
    ...preparedOptions.headers,
    payment: encodeCredentialHeader(credential),
    'x-payment-protocol': MPP_PROTOCOL,
    'x-payment-version': MPP_VERSION,
  };
  const finalResponse = await fetchImpl(url, {
    ...preparedOptions,
    headers: retryHeaders,
  });
  const receipt = await extractHttpPaymentReceipt(finalResponse);

  if (requireReceipt && !receipt) {
    throw new Error('MPP payment response is missing a receipt');
  }

  return attachPaymentContext(finalResponse, {
    challenge,
    credential,
    receipt,
  });
}

export function createMppHttpAgent(config = {}) {
  return {
    fetch: (url, options = {}) => mppFetch(url, options, config),
    getServiceInfo: (baseUrl, options = {}) =>
      fetchMppServiceInfo(baseUrl, {
        ...config,
        ...options,
      }),
    getDiscovery: (baseUrl, options = {}) =>
      discoverMppHttpService(baseUrl, {
        ...config,
        ...options,
      }),
    discoverPayableRoutes: async (baseUrl, options = {}) => {
      const discovery = await discoverMppHttpService(baseUrl, {
        ...config,
        ...options,
      });
      return discovery.payableRoutes;
    },
  };
}
