import {
  MPP_PROTOCOL,
  MPP_VERSION,
  attachPaymentMetadata,
  buildPaymentInfoFromPricing,
  buildHttpPaymentRequiredResponse,
  buildMppServiceInfo,
  createPaymentChallenge,
  createPaymentReceipt,
  extractPaymentCredential,
  verifyPaymentCredential,
} from './index.js';

function encodeHeaderPayload(payload) {
  return Buffer.from(JSON.stringify(payload), 'utf8').toString('base64url');
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

function asNonEmptyString(value) {
  if (value === null || value === undefined) return null;
  const normalized = String(value).trim();
  return normalized.length > 0 ? normalized : null;
}

function sanitizeBindingBody(body) {
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return body ?? {};
  }

  const nextBody = { ...body };
  if (nextBody._meta && typeof nextBody._meta === 'object' && !Array.isArray(nextBody._meta)) {
    nextBody._meta = { ...nextBody._meta };
    delete nextBody._meta.payment;
    if (Object.keys(nextBody._meta).length === 0) {
      delete nextBody._meta;
    }
  }
  delete nextBody.paymentCredential;
  return nextBody;
}

function isHttpResponseShape(value) {
  return Boolean(
    value &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    (Object.prototype.hasOwnProperty.call(value, 'status') ||
      Object.prototype.hasOwnProperty.call(value, 'headers') ||
      Object.prototype.hasOwnProperty.call(value, 'body') ||
      Object.prototype.hasOwnProperty.call(value, 'rawBody') ||
      Object.prototype.hasOwnProperty.call(value, '_html') ||
      Object.prototype.hasOwnProperty.call(value, 'contentType')),
  );
}

function normalizeArray(values = []) {
  if (Array.isArray(values)) return values.filter(Boolean).map((value) => String(value));
  if (values === null || values === undefined) return [];
  return [String(values)];
}

function normalizeHttpMethod(method) {
  return String(method || 'get')
    .trim()
    .toLowerCase();
}

function buildHttpOperationId(method, path) {
  return `http_${normalizeHttpMethod(method)}_${String(path || '/').replace(/[^a-zA-Z0-9]+/g, '_')}`.replace(
    /_+/g,
    '_',
  );
}

function resolveHttpRouteMeta(route = {}) {
  const handlerMeta =
    route?.handler?.__mppRouteMeta &&
    typeof route.handler.__mppRouteMeta === 'object' &&
    !Array.isArray(route.handler.__mppRouteMeta)
      ? route.handler.__mppRouteMeta
      : {};
  const pricing = route.pricing || handlerMeta.pricing || null;
  const description = route.description || handlerMeta.description || '';
  const paymentInfo =
    route.paymentInfo ||
    handlerMeta.paymentInfo ||
    (pricing
      ? buildPaymentInfoFromPricing({
          toolName:
            route.routeId ||
            handlerMeta.routeId ||
            `${String(route.method || 'GET').toUpperCase()} ${route.path || '/'}`,
          description,
          pricing,
          intent: route.intent || handlerMeta.intent || 'charge',
        })
      : null);

  return {
    summary:
      route.summary ||
      handlerMeta.summary ||
      description ||
      `${String(route.method || 'GET').toUpperCase()} ${route.path || '/'}`,
    description,
    tags: normalizeArray(route.tags || handlerMeta.tags || route.pluginId || 'gateway'),
    inputSchema: route.inputSchema ||
      handlerMeta.inputSchema || {
        type: 'object',
        additionalProperties: true,
      },
    outputSchema: route.outputSchema ||
      handlerMeta.outputSchema || {
        type: 'object',
        additionalProperties: true,
      },
    paymentInfo,
  };
}

async function resolveMaybeFunction(value, context) {
  if (typeof value === 'function') {
    return value(context);
  }
  return value;
}

export function extractHttpPaymentCredential({ headers = {}, body = {} } = {}) {
  const candidates = [
    extractPaymentCredential(body),
    decodeHeaderPayload(headers.payment),
    decodeHeaderPayload(headers['payment-credential']),
    decodeHeaderPayload(headers['x-payment']),
    decodeHeaderPayload(headers['x-payment-credential']),
  ];

  for (const candidate of candidates) {
    if (candidate && typeof candidate === 'object' && !Array.isArray(candidate)) {
      return candidate;
    }
  }

  return null;
}

export function buildHttpPaymentHeaders({
  challenge = null,
  receipt = null,
  serviceInfo = null,
  validationError = null,
} = {}) {
  const headers = {
    'x-payment-protocol': MPP_PROTOCOL,
    'x-payment-version': MPP_VERSION,
  };

  if (challenge?.challengeId) {
    headers['payment-required'] = encodeHeaderPayload({
      protocol: MPP_PROTOCOL,
      protocolVersion: MPP_VERSION,
      challenge,
      service: serviceInfo || null,
      validationError: validationError || null,
    });
    headers['cache-control'] = 'no-store';
  }

  if (receipt?.receiptId) {
    headers['payment-response'] = encodeHeaderPayload({
      protocol: MPP_PROTOCOL,
      protocolVersion: MPP_VERSION,
      receipt,
      service: serviceInfo || null,
    });
  }

  return headers;
}

export function attachPaymentReceiptToHttpResponse(
  result,
  { receipt, credential = null, serviceInfo = null, attachReceiptToBody = true } = {},
) {
  const headers = buildHttpPaymentHeaders({ receipt, serviceInfo });
  const paymentMetadata = {
    protocol: MPP_PROTOCOL,
    receipt,
    credentialId: credential?.credentialId || null,
  };

  if (isHttpResponseShape(result)) {
    const response = {
      ...result,
      headers: {
        ...(result.headers || {}),
        ...headers,
      },
    };

    if (
      attachReceiptToBody &&
      !response._html &&
      !Object.prototype.hasOwnProperty.call(response, 'rawBody')
    ) {
      response.body = attachPaymentMetadata(
        Object.prototype.hasOwnProperty.call(response, 'body') ? response.body : {},
        paymentMetadata,
      );
    }

    return response;
  }

  return {
    status: 200,
    headers,
    body: attachReceiptToBody ? attachPaymentMetadata(result, paymentMetadata) : result,
  };
}

export function buildHttpRouteDiscoveryDocument({
  routes = [],
  serviceInfo = null,
  serverUrl = '/',
} = {}) {
  const resolvedService =
    serviceInfo ||
    buildMppServiceInfo({
      serviceId: 'stateset-http-gateway',
      serviceName: 'StateSet HTTP Gateway',
      serverName: 'stateset-http-gateway',
      serverUrl,
      transportType: 'http',
    });
  const paths = {};

  for (const route of routes) {
    if (!route?.path || !route?.method) {
      continue;
    }

    const method = normalizeHttpMethod(route.method);
    const openapiPath = route.openapiPath || route.path;
    const meta = resolveHttpRouteMeta(route);
    const responses = {
      200: {
        description: 'Successful HTTP route execution',
        content: {
          'application/json': {
            schema: meta.outputSchema,
          },
        },
      },
    };

    if (meta.paymentInfo) {
      responses[402] = {
        description: 'Payment challenge required before execution',
        content: {
          'application/json': {
            schema: {
              type: 'object',
              properties: {
                error: { type: 'string' },
                paymentChallenge: { type: 'object' },
              },
            },
          },
        },
      };
    }

    const operation = {
      operationId: buildHttpOperationId(method, openapiPath),
      summary: meta.summary,
      description: meta.description,
      tags: meta.tags,
      responses,
      'x-stateset-plugin-id': route.pluginId || null,
      'x-stateset-permission-level': route.level || null,
    };

    if (!['get', 'head', 'delete'].includes(method)) {
      operation.requestBody = {
        required: false,
        content: {
          'application/json': {
            schema: meta.inputSchema,
          },
        },
      };
    }

    if (meta.paymentInfo) {
      operation['x-payment-info'] = meta.paymentInfo;
    }

    if (!paths[openapiPath]) {
      paths[openapiPath] = {};
    }
    paths[openapiPath][method] = operation;
  }

  return {
    openapi: '3.1.0',
    info: {
      title: `${resolvedService.name} HTTP Payment Discovery`,
      version: resolvedService.version,
      description: 'Machine Payments Protocol discovery document for HTTP routes.',
    },
    servers: [{ url: serverUrl }],
    'x-service-info': resolvedService,
    paths,
  };
}

export function createMppHttpRouteHandler({
  routeId,
  description = '',
  summary = null,
  tags = [],
  inputSchema = null,
  outputSchema = null,
  pricing = null,
  resolvePricing = null,
  paymentInfo = null,
  serviceInfo = null,
  intent = 'charge',
  ttlSeconds = 300,
  resolvePayer = null,
  resolveAuthorization = null,
  resolveMethod = null,
  resolveProof = null,
  _resolveCredentialMetadata = null,
  resolveChallengeMetadata = null,
  resolveReceiptMetadata = null,
  attachReceiptToBody = true,
  handler,
} = {}) {
  if (typeof handler !== 'function') {
    throw new Error('handler is required');
  }
  const wrappedHandler = async function mppHttpRouteHandler(request = {}) {
    const resolvedService =
      serviceInfo ||
      buildMppServiceInfo({
        serverName: 'stateset-http-gateway',
        serverUrl: request.pathname || '/',
        transportType: 'http',
      });
    const resolvedPricing = resolvePricing ? await resolvePricing(request) : pricing;

    if (!resolvedPricing) {
      return handler(request);
    }

    const requestId =
      asNonEmptyString(request.requestId) ||
      asNonEmptyString(request.headers?.['x-request-id']) ||
      null;
    const sessionId =
      asNonEmptyString(request.sessionId) ||
      asNonEmptyString(request.headers?.['x-session-id']) ||
      null;
    const resolvedRouteId =
      routeId || `${String(request.method || 'GET').toUpperCase()} ${request.pathname || '/'}`;
    const challenge = createPaymentChallenge({
      toolName: resolvedRouteId,
      description,
      pricing: resolvedPricing,
      params: {
        method: String(request.method || 'GET').toUpperCase(),
        pathname: request.pathname || '/',
        params: request.params || {},
        query: request.query || {},
        body: sanitizeBindingBody(request.body || {}),
      },
      requestId,
      sessionId,
      intent,
      ttlSeconds,
      serviceId: resolvedService.id,
      serviceName: resolvedService.name,
      metadata: await resolveMaybeFunction(resolveChallengeMetadata, {
        request,
        pricing: resolvedPricing,
        serviceInfo: resolvedService,
      }),
    });

    const credential = extractHttpPaymentCredential({
      headers: request.headers || {},
      body: request.body || {},
    });
    const verification = verifyPaymentCredential(credential, challenge);
    if (!verification.valid) {
      const response = buildHttpPaymentRequiredResponse({
        challenge,
        serviceInfo: resolvedService,
      });
      return {
        ...response,
        headers: {
          ...(response.headers || {}),
          ...buildHttpPaymentHeaders({
            challenge,
            serviceInfo: resolvedService,
            validationError: credential ? verification.reason : null,
          }),
        },
        body: {
          ...(response.body || {}),
          validationError: credential ? verification.reason : null,
        },
      };
    }

    const verifiedCredential = verification.credential;
    const payer = await resolveMaybeFunction(resolvePayer, {
      request,
      challenge,
      credential: verifiedCredential,
      pricing: resolvedPricing,
      serviceInfo: resolvedService,
    });
    const authorization = await resolveMaybeFunction(resolveAuthorization, {
      request,
      challenge,
      credential: verifiedCredential,
      pricing: resolvedPricing,
      serviceInfo: resolvedService,
    });
    const method = await resolveMaybeFunction(resolveMethod, {
      request,
      challenge,
      credential: verifiedCredential,
      pricing: resolvedPricing,
      serviceInfo: resolvedService,
    });
    const proof = await resolveMaybeFunction(resolveProof, {
      request,
      challenge,
      credential: verifiedCredential,
      pricing: resolvedPricing,
      serviceInfo: resolvedService,
    });
    const payment = {
      challenge,
      credential: verifiedCredential,
      pricing: resolvedPricing,
      service: resolvedService,
      payer: payer || verifiedCredential?.payer || null,
      authorization: authorization || verifiedCredential?.authorization || null,
      method: method || verifiedCredential?.method || null,
      proof: proof || verifiedCredential?.proof || null,
    };
    const receiptCredential =
      payment.payer && !verifiedCredential?.payer
        ? {
            ...verifiedCredential,
            payer: payment.payer,
          }
        : verifiedCredential;

    const result = await handler({
      ...request,
      payment,
    });

    const receipt = createPaymentReceipt({
      challenge,
      credential: receiptCredential,
      toolName: resolvedRouteId,
      requestId,
      sessionId,
      charge: {
        charged: true,
        rule: {
          chainId: resolvedPricing?.chainId || resolvedPricing?.network || null,
          tokenSymbol:
            resolvedPricing?.tokenSymbol ||
            resolvedPricing?.token?.symbol ||
            challenge.amount?.asset,
          amount: resolvedPricing?.amount ?? challenge.amount?.amount ?? null,
        },
      },
      metadata: await resolveMaybeFunction(resolveReceiptMetadata, {
        request,
        result,
        challenge,
        credential: verifiedCredential,
        pricing: resolvedPricing,
        serviceInfo: resolvedService,
      }),
    });

    return attachPaymentReceiptToHttpResponse(result, {
      receipt,
      credential: receiptCredential,
      serviceInfo: resolvedService,
      attachReceiptToBody,
    });
  };

  wrappedHandler.__mppRouteMeta = {
    routeId: routeId || null,
    description,
    summary: summary || description || routeId || null,
    tags: normalizeArray(tags),
    inputSchema,
    outputSchema,
    pricing,
    paymentInfo:
      paymentInfo ||
      (pricing
        ? buildPaymentInfoFromPricing({
            toolName: routeId || null,
            description,
            pricing,
            intent,
          })
        : null),
    intent,
    transportType: 'http',
  };

  return wrappedHandler;
}
