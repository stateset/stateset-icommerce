import { decodeBase64Json, encodeBase64Json } from './crypto.js';
import { isExactEvmRequirement } from './exact-evm.js';
import { settleFacilitatedPayment, verifyFacilitatedPayment } from './facilitator.js';

function asObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function normalizeChallenge(paymentRequired) {
  const payload = asObject(paymentRequired);
  if (!payload || Number(payload.x402Version) !== 2 || !Array.isArray(payload.accepts)) {
    throw new Error('paymentRequired must be an x402 v2 PaymentRequired object');
  }
  if (payload.accepts.length === 0) {
    throw new Error('paymentRequired.accepts must contain at least one payment requirement');
  }
  return cloneJson(payload);
}

function sendBody(res, status, body, headers = {}) {
  res.statusCode = status;
  for (const [key, value] of Object.entries(headers)) {
    res.setHeader(key, value);
  }

  if (body === undefined || body === null) {
    res.end('');
    return;
  }

  if (typeof body === 'string') {
    if (!Object.keys(headers).some((key) => key.toLowerCase() === 'content-type')) {
      res.setHeader('Content-Type', 'text/plain; charset=utf-8');
    }
    res.end(body);
    return;
  }

  if (!Object.keys(headers).some((key) => key.toLowerCase() === 'content-type')) {
    res.setHeader('Content-Type', 'application/json');
  }
  res.end(JSON.stringify(body));
}

function buildChallengeResponse(paymentRequired, error) {
  const challenge = cloneJson(paymentRequired);
  if (error) {
    challenge.error = String(error);
  }
  return challenge;
}

function normalizeHandlerResult(result) {
  if (result && typeof result === 'object' && !Array.isArray(result)) {
    const payload = /** @type {{ status?: number, body?: unknown, headers?: Record<string, string> }} */ (
      result
    );
    if ('status' in payload || 'body' in payload || 'headers' in payload) {
      return {
        status: Number(payload.status || 200),
        body: payload.body,
        headers: payload.headers || {},
      };
    }
  }

  return {
    status: 200,
    body: result ?? { success: true },
    headers: {},
  };
}

function defaultInvalidStatus(errorReason) {
  return String(errorReason || '').startsWith('unexpected_') ? 502 : 402;
}

export function buildExactEvmPaymentRequired({
  url,
  description,
  mimeType,
  amount,
  asset,
  network,
  payTo,
  maxTimeoutSeconds = 60,
  extra = {},
  extensions = {},
  error = 'PAYMENT-SIGNATURE header is required',
}) {
  const accepted = {
    scheme: 'exact',
    network: String(network),
    amount: String(amount),
    asset: String(asset),
    payTo: String(payTo),
    maxTimeoutSeconds: Number(maxTimeoutSeconds),
    extra: {
      assetTransferMethod: String(extra.assetTransferMethod || 'eip3009'),
      name: String(extra.name || 'USDC'),
      version: String(extra.version || '2'),
      ...cloneJson(asObject(extra) || {}),
    },
  };

  if (!isExactEvmRequirement(accepted)) {
    throw new Error('buildExactEvmPaymentRequired requires an exact EVM payment requirement');
  }

  return {
    x402Version: 2,
    error,
    resource: {
      url: String(url),
      ...(description ? { description: String(description) } : {}),
      ...(mimeType ? { mimeType: String(mimeType) } : {}),
    },
    accepts: [accepted],
    extensions: cloneJson(asObject(extensions) || {}),
  };
}

export function createExactEvmResourceServerHandler({
  paymentRequired,
  facilitatorPrivateKey = null,
  checkOnchain = true,
  onRequest = null,
  verifyPayment = verifyFacilitatedPayment,
  settlePayment = settleFacilitatedPayment,
} = {}) {
  if (!paymentRequired) {
    throw new Error('paymentRequired is required');
  }
  if (!facilitatorPrivateKey && settlePayment === settleFacilitatedPayment) {
    throw new Error('facilitatorPrivateKey is required unless settlePayment is overridden');
  }

  return async (req, res) => {
    const resolvedPaymentRequired = normalizeChallenge(
      typeof paymentRequired === 'function' ? await paymentRequired(req) : paymentRequired,
    );
    const paymentRequirements = resolvedPaymentRequired.accepts[0];
    const paymentHeader = req.headers?.['payment-signature'];

    if (!paymentHeader) {
      const challenge = buildChallengeResponse(resolvedPaymentRequired, resolvedPaymentRequired.error);
      return sendBody(res, 402, challenge, {
        'payment-required': encodeBase64Json(challenge),
      });
    }

    let paymentPayload;
    try {
      paymentPayload = decodeBase64Json(String(paymentHeader));
    } catch (_error) {
      return sendBody(res, 400, { error: 'invalid_payload' });
    }

    const verification = await verifyPayment({
      x402Version: 2,
      paymentPayload,
      paymentRequirements,
      checkOnchain,
    });

    if (!verification?.isValid) {
      const challenge = buildChallengeResponse(
        resolvedPaymentRequired,
        verification?.invalidReason || resolvedPaymentRequired.error,
      );
      return sendBody(res, 402, challenge, {
        'payment-required': encodeBase64Json(challenge),
      });
    }

    const settlement = await settlePayment({
      x402Version: 2,
      paymentPayload,
      paymentRequirements,
      facilitatorPrivateKey,
    });

    if (!settlement?.success) {
      const status = defaultInvalidStatus(settlement?.errorReason);
      const headers = {
        'PAYMENT-RESPONSE': encodeBase64Json(settlement),
      };
      if (status === 402) {
        const challenge = buildChallengeResponse(
          resolvedPaymentRequired,
          settlement?.errorReason || resolvedPaymentRequired.error,
        );
        headers['payment-required'] = encodeBase64Json(challenge);
        return sendBody(res, status, settlement, headers);
      }
      return sendBody(res, status, settlement, headers);
    }

    const result = normalizeHandlerResult(
      onRequest
        ? await onRequest({
            req,
            paymentPayload,
            paymentRequirements,
            verification,
            settlement,
          })
        : { body: { success: true }, status: 200 },
    );

    return sendBody(res, result.status, result.body, {
      ...result.headers,
      'PAYMENT-RESPONSE': encodeBase64Json(settlement),
    });
  };
}

export default {
  buildExactEvmPaymentRequired,
  createExactEvmResourceServerHandler,
};
