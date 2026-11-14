// Kernel decision fields for the agentic replay log.
//
// A governed tool call answers with the kernel's execution receipt. On the MCP
// path that receipt sits at `<response>.structuredContent.result.receipt`, deep
// enough that the replay sanitiser's depth limit turns the receipt's `policy`
// and `economic_context` into "[truncated]" -- so the reason codes saying why a
// command was refused, and the principal it ran as, never reached the log.
//
// This module lifts a small, bounded `kernel` block out of the UNCOMPACTED
// result before sanitisation, so an operator can join a replay event to its
// kernel receipt and see the decision without parsing the result:
//
//   { commandId, correlationId, receiptStatus, errorCode, reasonCodes,
//     principalId, auditHash }
//
// Each field is present only when the receipt has it; a result with no
// governed receipt yields no block at all.
//
// Signature coverage. The replay log signs a fixed payload that includes
// `resultHash` but not arbitrary event fields (see replay-log.js). So the
// block is bound into `resultHash` (`bindKernelToResultHash`): for an event
// WITH a `kernel` block, resultHash = replayEventHash({ kernel, resultHash:
// replayEventHash(result) }); for an event without one it is
// replayEventHash(result), exactly as before. Old events, and every event of
// an ungoverned call, hash and verify unchanged.

import { randomUUID } from 'node:crypto';

import { replayEventHash } from './audit-envelope.js';

/** Most policy reason codes kept on one event; the rest are counted. */
export const MAX_KERNEL_REASON_CODES = 16;
/** Longest string kept in the block (ids and codes are far shorter). */
export const MAX_KERNEL_FIELD_CHARS = 128;

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const isPlainObject = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

const boundedString = (value) => {
  if (typeof value !== 'string' || value === '') return null;
  if (value.length <= MAX_KERNEL_FIELD_CHARS) return value;
  return `${value.slice(0, MAX_KERNEL_FIELD_CHARS - 3)}...`;
};

/**
 * The governed tool result inside a replay `result`: the value
 * `createKernelToolExecutor` returns (`{ kernel: true, receipt, ... }`),
 * either bare (the executeTool path) or inside an MCP CallToolResult's
 * `structuredContent.result` (the MCP path).
 */
const governedResultOf = (result) => {
  const candidates = [result, isPlainObject(result) ? result.structuredContent?.result : null];
  return (
    candidates.find(
      (candidate) =>
        isPlainObject(candidate) &&
        candidate.kernel === true &&
        isPlainObject(candidate.receipt) &&
        typeof candidate.receipt.status === 'string',
    ) ?? null
  );
};

/**
 * Extract the compact kernel block from a tool result, before sanitisation.
 *
 * @param {unknown} result - the tool's raw result (MCP response or bare)
 * @returns {null | {
 *   commandId?: string,
 *   correlationId?: string,
 *   receiptStatus?: string,
 *   errorCode?: string,
 *   reasonCodes?: string[],
 *   reasonCodesOmitted?: number,
 *   principalId?: string,
 *   auditHash?: string,
 * }} `null` when the result carries no governed kernel receipt
 */
export function extractKernelReplayFields(result) {
  const governed = governedResultOf(result);
  if (!governed) return null;
  const { receipt } = governed;
  const context = isPlainObject(receipt.economic_context) ? receipt.economic_context : {};
  const principal = isPlainObject(context.principal) ? context.principal : {};

  const block = {};
  const put = (key, value) => {
    const bounded = boundedString(value);
    if (bounded !== null) block[key] = bounded;
  };
  put('commandId', receipt.command_id);
  put('correlationId', context.correlation_id);
  put('receiptStatus', receipt.status);
  put('errorCode', receipt.error_code);

  const codes = Array.isArray(receipt.policy?.reason_codes)
    ? receipt.policy.reason_codes.map(boundedString).filter((code) => code !== null)
    : [];
  if (codes.length > 0) {
    block.reasonCodes = codes.slice(0, MAX_KERNEL_REASON_CODES);
    if (codes.length > MAX_KERNEL_REASON_CODES) {
      block.reasonCodesOmitted = codes.length - MAX_KERNEL_REASON_CODES;
    }
  }

  put('principalId', principal.id);
  put('auditHash', receipt.audit_hash);
  return block;
}

/**
 * Bind a replay event's kernel block into its `resultHash`, which the event
 * signature covers. Without a block the hash is returned unchanged.
 *
 * @param {string} resultHash - replayEventHash of the event's result
 * @param {object | null | undefined} kernel - the event's kernel block
 * @returns {string}
 */
export function bindKernelToResultHash(resultHash, kernel) {
  if (!kernel) return resultHash;
  return replayEventHash({ kernel, resultHash });
}

/**
 * The correlation id for a governed command issued by an MCP tool call.
 *
 * The kernel envelope types `correlation_id` as a UUID and refuses any other
 * value, while an MCP (JSON-RPC) request id is often a number. A UUID request
 * id is used as-is (lowercased, as the kernel echoes it); otherwise a fresh
 * UUID is minted, which the receipt echoes and the replay event records in
 * `kernel.correlationId`.
 *
 * @param {unknown} requestId
 * @param {() => string} [mint]
 * @returns {string}
 */
export function resolveKernelCorrelationId(requestId, mint = randomUUID) {
  if (typeof requestId === 'string' && UUID_PATTERN.test(requestId)) {
    return requestId.toLowerCase();
  }
  return mint();
}
