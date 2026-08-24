// MPP (Machine Payments Protocol) helpers for the MCP orchestrator.
//
// Three concerns live here:
//   - `attachPaymentMetadataToResponse`: stamp a payment receipt onto an
//     MCP text response whose first content block is JSON.
//   - `resolveMppPaymentContext`: given a tool + params, work out whether
//     the tool is priced and, if so, whether the caller presented a valid
//     payment credential for the generated challenge.
//   - `preparePaymentForTool`: the `prepare_payment` runtime tool body —
//     validates params, builds the challenge, and returns a credential
//     template + retry example the agent can fill in.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import {
  MPP_PROTOCOL,
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  MPP_VERSION,
  attachPaymentMetadata,
  buildPaymentInfoFromPricing,
  buildPaymentRequiredPayload,
  createPaymentChallenge,
  extractPaymentCredential,
  verifyPaymentCredential,
} from '../mpp/index.js';
import {
  formatValidationIssues,
  inputSchemaDefToJsonSchema,
  validateToolInput,
} from '../tool-schema.js';
import { normalizeToolName } from './policy-helpers.js';

/**
 * Attach payment metadata to an MCP tool response whose first content block
 * is a JSON-encoded text payload. Responses that don't match that shape are
 * returned untouched.
 *
 * @param {{content?: Array<{type: string, text?: string}>} | null | undefined} response
 * @param {object} [paymentMetadata]
 * @returns {typeof response}
 */
export const attachPaymentMetadataToResponse = (response, paymentMetadata = {}) => {
  if (!response || !Array.isArray(response.content) || response.content.length === 0) {
    return response;
  }

  const first = response.content[0];
  if (!first || first.type !== 'text' || typeof first.text !== 'string') {
    return response;
  }

  try {
    const parsed = JSON.parse(first.text);
    const nextPayload = attachPaymentMetadata(parsed, paymentMetadata);
    return {
      ...response,
      content: [{ ...first, text: JSON.stringify(nextPayload) }, ...response.content.slice(1)],
    };
  } catch {
    return response;
  }
};

/**
 * Build `resolveMppPaymentContext` for one server instance.
 *
 * @param {{
 *   getAgenticToolPricing: (toolName: string) => Promise<object | null>,
 *   serviceInfo: { id: string, name: string },
 * }} deps
 * @returns {(input?: {
 *   toolName: string,
 *   description?: string,
 *   params?: object,
 *   extra?: object,
 *   requestId?: string | null,
 *   sessionId?: string | null,
 * }) => Promise<{
 *   pricing: object | null,
 *   challenge: object | null,
 *   credential: object | null,
 *   authorized: boolean,
 *   verification?: object,
 *   errorPayload?: object,
 * }>}
 */
export function createResolveMppPaymentContext({
  getAgenticToolPricing,
  serviceInfo: MPP_SERVICE_INFO,
}) {
  const resolveMppPaymentContext = async ({
    toolName,
    description = '',
    params = {},
    extra = {},
    requestId = null,
    sessionId = null,
  } = {}) => {
    const pricing = await getAgenticToolPricing(toolName);
    if (!pricing) {
      return {
        pricing: null,
        challenge: null,
        credential: null,
        authorized: false,
      };
    }

    const challenge = createPaymentChallenge({
      toolName,
      description,
      pricing,
      params,
      requestId,
      sessionId,
      serviceId: MPP_SERVICE_INFO.id,
      serviceName: MPP_SERVICE_INFO.name,
    });
    const credential = extractPaymentCredential(params, extra);
    if (!credential) {
      return {
        pricing,
        challenge,
        credential: null,
        authorized: false,
        errorPayload: buildPaymentRequiredPayload({ challenge }),
      };
    }

    const verification = verifyPaymentCredential(credential, challenge);
    if (!verification.valid) {
      return {
        pricing,
        challenge,
        credential,
        authorized: false,
        verification,
        errorPayload: buildPaymentRequiredPayload({
          challenge,
          reason: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
          validationError: verification.reason,
        }),
      };
    }

    return {
      pricing,
      challenge,
      credential: verification.credential,
      verification,
      authorized: true,
    };
  };

  return resolveMppPaymentContext;
}

/**
 * Build `preparePaymentForTool` for one server instance.
 *
 * @param {{
 *   toolDefsByName: Map<string, object>,
 *   getAgenticToolPricing: (toolName: string) => Promise<object | null>,
 *   serviceInfo: { id: string, name: string },
 * }} deps
 * @returns {(input?: {
 *   tool: string,
 *   params?: object,
 *   requestId?: string | null,
 *   sessionId?: string | null,
 *   includeSchema?: boolean,
 * }) => Promise<object>}
 */
export function createPreparePaymentForTool({
  toolDefsByName: TOOL_DEFS_BY_NAME,
  getAgenticToolPricing,
  serviceInfo: MPP_SERVICE_INFO,
}) {
  const preparePaymentForTool = async ({
    tool,
    params = {},
    requestId = null,
    sessionId = null,
    includeSchema = false,
  } = {}) => {
    const resolvedToolName = normalizeToolName(tool || '');
    if (!resolvedToolName) {
      return {
        success: false,
        payable: false,
        error: 'tool is required',
      };
    }

    const toolDef = TOOL_DEFS_BY_NAME.get(resolvedToolName);
    if (!toolDef) {
      return {
        success: false,
        tool: resolvedToolName,
        payable: false,
        error: `Unknown tool '${resolvedToolName}'`,
      };
    }

    const validation = validateToolInput(toolDef.inputSchema || {}, params || {});
    if (!validation.success) {
      return {
        success: false,
        tool: resolvedToolName,
        payable: false,
        error: `Invalid parameters for tool '${resolvedToolName}'`,
        validation: {
          valid: false,
          issues: formatValidationIssues(validation.error),
        },
      };
    }

    const pricing = await getAgenticToolPricing(resolvedToolName);
    const paymentInfo = buildPaymentInfoFromPricing({
      toolName: resolvedToolName,
      description: toolDef.description,
      pricing,
    });

    if (!pricing || !paymentInfo) {
      return {
        success: true,
        tool: resolvedToolName,
        payable: false,
        service: MPP_SERVICE_INFO,
        validation: { valid: true },
        paymentInfo: null,
        challenge: null,
        reason: 'No pricing configured for this tool.',
        ...(includeSchema
          ? { inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}) }
          : {}),
      };
    }

    const challenge = createPaymentChallenge({
      toolName: resolvedToolName,
      description: toolDef.description,
      pricing,
      params: validation.data,
      requestId,
      sessionId,
      serviceId: MPP_SERVICE_INFO.id,
      serviceName: MPP_SERVICE_INFO.name,
    });
    const primaryMethod = Array.isArray(challenge.paymentMethods)
      ? challenge.paymentMethods[0] || null
      : null;
    const credentialTemplate = {
      protocol: MPP_PROTOCOL,
      protocolVersion: MPP_VERSION,
      type: 'credential',
      challengeId: challenge.challengeId,
      payer: '<payer-id>',
      method: primaryMethod
        ? {
            kind: primaryMethod.kind || null,
            asset: primaryMethod.asset || null,
            network: primaryMethod.network || null,
          }
        : null,
      amount: challenge.amount,
      binding: challenge.binding,
      authorization: {
        type: '<signature-or-proof>',
      },
    };

    return {
      success: true,
      tool: resolvedToolName,
      payable: true,
      service: MPP_SERVICE_INFO,
      paymentInfo,
      challenge,
      acceptedPaymentMethods: challenge.paymentMethods || [],
      validation: { valid: true },
      credentialTemplate,
      retryExample: {
        jsonrpc: '2.0',
        id: requestId || '<request-id>',
        method: resolvedToolName,
        params: validation.data,
        _meta: {
          payment: credentialTemplate,
        },
      },
      ...(includeSchema
        ? { inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}) }
        : {}),
    };
  };

  // buildPolicyDecisionBundle's body lives in ./mcp/policy-evaluator.js,
  // and is invoked by `createEvaluatePolicy` below — no orchestrator-level
  // call sites remain after the extraction.

  return preparePaymentForTool;
}
