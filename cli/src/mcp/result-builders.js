/**
 * Tool result envelope builders.
 *
 * Build the `_agentic` envelope (schemaVersion, status, timing, policy,
 * permission, charge, mutation manifest) attached to every MCP tool
 * result when `structuredToolResults` is enabled. Pulled out of
 * mcp-server.js so the wrapping helpers stay testable in isolation.
 */

import { compactReplayValue } from './replay-sanitizer.js';

export const AGENTIC_TOOL_RESULT_SCHEMA_VERSION = '1.0.0';

/**
 * Wrap a base payload with `_agentic` metadata. When structured results
 * are disabled the payload is returned unchanged.
 *
 * @param {*} basePayload
 * @param {string} status
 * @param {number} startedAt - epoch ms
 * @param {Object} [toolMeta]
 * @param {Object} options
 * @param {boolean} options.structured - whether to wrap
 * @param {string} [options.schemaVersion] - override schema version
 */
export function buildToolResultPayload(
  basePayload,
  status,
  startedAt,
  toolMeta = {},
  { structured, schemaVersion = AGENTIC_TOOL_RESULT_SCHEMA_VERSION } = {},
) {
  if (!structured) {
    return basePayload;
  }

  const agenticMeta = {
    schemaVersion,
    status,
    tool: basePayload?.tool || toolMeta.name || null,
    requestId: toolMeta.requestId ?? null,
    sessionId: toolMeta.sessionId ?? null,
    policy: compactReplayValue(toolMeta.policy || null),
    permission: compactReplayValue(toolMeta.permission || null),
    charge: compactReplayValue(toolMeta.charge || null),
    mutation: compactReplayValue(toolMeta.mutationManifest || null),
    timing: {
      startedAt: new Date(startedAt).toISOString(),
      completedAt: new Date().toISOString(),
      elapsedMs: Date.now() - startedAt,
    },
  };

  const withType = {
    ...toolMeta.meta,
    ...agenticMeta,
  };

  if (
    basePayload === null ||
    basePayload === undefined ||
    Array.isArray(basePayload) ||
    typeof basePayload !== 'object'
  ) {
    return {
      result: basePayload,
      _agentic: compactReplayValue(withType),
    };
  }

  if (basePayload._agentic) {
    return basePayload;
  }

  return {
    ...basePayload,
    _agentic: compactReplayValue(withType),
  };
}

/**
 * Build a complete MCP tool response (content array with a single JSON
 * text part). When `isError` is true the response is flagged for the
 * client.
 */
export function buildToolResultResponse(
  result,
  status,
  startedAt,
  toolMeta = {},
  isError = false,
  options = {},
) {
  const payload = buildToolResultPayload(result, status, startedAt, toolMeta, options);
  const response = {
    content: [
      {
        type: 'text',
        text: JSON.stringify(payload),
      },
    ],
  };
  if (isError) response.isError = true;
  return response;
}

/**
 * Re-wrap an already-built MCP response's first JSON text content with
 * the `_agentic` envelope. When structured results are off, or the
 * response shape is unrecognized, the response is returned unchanged.
 */
export function attachStructuredToolMetadataToResponse(
  response,
  status,
  startedAt,
  toolMeta = {},
  options = {},
) {
  const { structured } = options;
  if (!structured || !response || !response.content || !Array.isArray(response.content)) {
    return response;
  }

  const first = response.content[0];
  if (!first || first.type !== 'text' || typeof first.text !== 'string') {
    return response;
  }

  try {
    const parsedPayload = JSON.parse(first.text);
    const payload = buildToolResultPayload(parsedPayload, status, startedAt, toolMeta, options);
    return {
      ...response,
      content: [{ ...first, text: JSON.stringify(payload) }, ...response.content.slice(1)],
    };
  } catch {
    return response;
  }
}
