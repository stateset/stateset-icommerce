import { Wallet } from 'ethers';
import {
  getExactEvmSupportedKinds,
  isExactEvmRequirement,
  settleExactEvmPaymentPayload,
  verifyExactEvmPaymentPayload,
} from './exact-evm.js';

/**
 * @typedef {Record<string, unknown>} JsonRecord
 * @typedef {{
 *   x402Version?: unknown,
 *   paymentPayload?: unknown,
 *   paymentRequirements?: unknown,
 *   checkOnchain?: boolean,
 * }} VerifyFacilitatedPaymentInput
 * @typedef {{
 *   x402Version?: unknown,
 *   paymentPayload?: unknown,
 *   paymentRequirements?: unknown,
 *   facilitatorPrivateKey?: string | null,
 * }} SettleFacilitatedPaymentInput
 * @typedef {{
 *   kinds?: unknown,
 *   extensions?: unknown[],
 *   signers?: JsonRecord | null,
 *   facilitatorPrivateKey?: string | null,
 * }} FacilitatorSupportedResponseOptions
 * @typedef {{
 *   facilitatorPrivateKey?: string | null,
 *   kinds?: unknown,
 *   extensions?: unknown[],
 *   signers?: JsonRecord | null,
 *   defaultCheckOnchain?: boolean,
 * }} FacilitatorHttpHandlerOptions
 * @typedef {AsyncIterable<Buffer | Uint8Array | string> & { method?: string, url?: string }} RequestLike
 * @typedef {{ statusCode: number, setHeader: (name: string, value: string) => void, end: (body: string) => void }} ResponseLike
 */

/**
 * @param {unknown} value
 * @returns {JsonRecord | null}
 */
function asObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? /** @type {JsonRecord} */ (value)
    : null;
}

/**
 * @param {unknown} value
 * @returns {string}
 */
function networkFrom(value) {
  return String(asObject(value)?.network || '');
}

/**
 * @param {unknown} value
 * @returns {unknown}
 */
function json(value) {
  return JSON.parse(JSON.stringify(value));
}

/**
 * @param {unknown} privateKey
 * @returns {string | null}
 */
function signerAddressFromPrivateKey(privateKey) {
  const normalized = String(privateKey || '').trim();
  if (!/^0x[a-fA-F0-9]{64}$/.test(normalized)) {
    return null;
  }
  return new Wallet(normalized).address;
}

/**
 * @param {unknown} version
 * @returns {string | null}
 */
function ensureV2(version) {
  if (version === undefined || version === null) return null;
  return Number(version) === 2 ? null : 'invalid_x402_version';
}

/**
 * @param {VerifyFacilitatedPaymentInput} input
 */
export async function verifyFacilitatedPayment({
  x402Version = 2,
  paymentPayload,
  paymentRequirements,
  checkOnchain = true,
}) {
  const versionError = ensureV2(x402Version);
  if (versionError) {
    return { isValid: false, invalidReason: versionError };
  }

  const payload = asObject(paymentPayload);
  const requirements = asObject(paymentRequirements) || asObject(payload?.accepted);
  if (!payload || !requirements) {
    return { isValid: false, invalidReason: 'invalid_payload' };
  }
  if (Number(payload.x402Version) !== 2) {
    return { isValid: false, invalidReason: 'invalid_x402_version' };
  }
  if (!isExactEvmRequirement(requirements)) {
    return { isValid: false, invalidReason: 'unsupported_scheme' };
  }

  return verifyExactEvmPaymentPayload({
    paymentPayload: payload,
    paymentRequirements: requirements,
    checkOnchain,
  });
}

/**
 * @param {SettleFacilitatedPaymentInput} input
 */
export async function settleFacilitatedPayment({
  x402Version = 2,
  paymentPayload,
  paymentRequirements,
  facilitatorPrivateKey,
}) {
  const versionError = ensureV2(x402Version);
  if (versionError) {
    return {
      success: false,
      errorReason: versionError,
      payer: '',
      transaction: '',
      network: networkFrom(paymentRequirements) || networkFrom(asObject(paymentPayload)?.accepted),
    };
  }

  const payload = asObject(paymentPayload);
  const requirements = asObject(paymentRequirements) || asObject(payload?.accepted);
  if (!payload || !requirements) {
    return {
      success: false,
      errorReason: 'invalid_payload',
      payer: '',
      transaction: '',
      network: String(requirements?.network || ''),
    };
  }
  if (Number(payload.x402Version) !== 2) {
    return {
      success: false,
      errorReason: 'invalid_x402_version',
      payer: '',
      transaction: '',
      network: String(requirements?.network || ''),
    };
  }
  if (!isExactEvmRequirement(requirements)) {
    return {
      success: false,
      errorReason: 'unsupported_scheme',
      payer: '',
      transaction: '',
      network: String(requirements?.network || ''),
    };
  }

  return settleExactEvmPaymentPayload({
    paymentPayload: payload,
    paymentRequirements: requirements,
    facilitatorPrivateKey,
  });
}

/**
 * @param {FacilitatorSupportedResponseOptions} [options]
 */
export function buildFacilitatorSupportedResponse({
  kinds = getExactEvmSupportedKinds(),
  extensions = [],
  signers = null,
  facilitatorPrivateKey = null,
} = {}) {
  let resolvedSigners = signers ? json(signers) : {};
  if (!signers && facilitatorPrivateKey) {
    const address = signerAddressFromPrivateKey(facilitatorPrivateKey);
    if (address) {
      resolvedSigners = { 'eip155:*': [address] };
    }
  }
  return {
    kinds: json(kinds),
    extensions: Array.isArray(extensions) ? [...extensions] : [],
    signers: resolvedSigners,
  };
}

/**
 * @param {ResponseLike} res
 * @param {number} status
 * @param {unknown} body
 */
function sendJson(res, status, body) {
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify(body));
}

/**
 * @param {RequestLike} req
 * @returns {Promise<JsonRecord>}
 */
async function readJson(req) {
  /** @type {Buffer[]} */
  const chunks = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const body = Buffer.concat(chunks).toString('utf8');
  return body ? /** @type {JsonRecord} */ (JSON.parse(body)) : {};
}

/**
 * @param {FacilitatorHttpHandlerOptions} [options]
 */
export function createFacilitatorHttpHandler({
  facilitatorPrivateKey = null,
  kinds = getExactEvmSupportedKinds(),
  extensions = [],
  signers = null,
  defaultCheckOnchain = true,
} = {}) {
  const supportedResponse = buildFacilitatorSupportedResponse({
    kinds,
    extensions,
    signers,
    facilitatorPrivateKey,
  });

  /** @param {RequestLike} req @param {ResponseLike} res */
  return async (req, res) => {
    try {
      if (req.method === 'GET' && req.url === '/supported') {
        return sendJson(res, 200, supportedResponse);
      }

      if (req.method === 'POST' && req.url === '/verify') {
        const body = asObject(await readJson(req));
        if (!body) {
          return sendJson(res, 400, { isValid: false, invalidReason: 'invalid_payload' });
        }
        const response = await verifyFacilitatedPayment({
          x402Version: body.x402Version,
          paymentPayload: body.paymentPayload,
          paymentRequirements: body.paymentRequirements,
          checkOnchain:
            body.checkOnchain === undefined ? defaultCheckOnchain : Boolean(body.checkOnchain),
        });
        return sendJson(res, 200, response);
      }

      if (req.method === 'POST' && req.url === '/settle') {
        const body = asObject(await readJson(req));
        if (!body) {
          return sendJson(res, 400, {
            success: false,
            errorReason: 'invalid_payload',
            payer: '',
            transaction: '',
            network: '',
          });
        }
        const response = await settleFacilitatedPayment({
          x402Version: body.x402Version,
          paymentPayload: body.paymentPayload,
          paymentRequirements: body.paymentRequirements,
          facilitatorPrivateKey,
        });
        return sendJson(res, 200, response);
      }

      return sendJson(res, 404, { error: 'Not Found' });
    } catch {
      return sendJson(res, 400, { error: 'Invalid request body' });
    }
  };
}

export default {
  verifyFacilitatedPayment,
  settleFacilitatedPayment,
  buildFacilitatorSupportedResponse,
  createFacilitatorHttpHandler,
};
