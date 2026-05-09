// Saga-style compensation hints + ID extraction helpers.
//
// When an agentic plan step fails after committing earlier steps, we look
// up a "compensation tool" (rollback) for each completed step and try to
// invoke it with whatever IDs the original step produced. This module
// owns the static lookup tables and the pure-function logic that maps
// a (compensationTool, params, result) triple into the inverse-call
// parameter object.
//
// Extracted from mcp-server.js for clarity and isolated testability.
// All exports are pure data or pure functions — no runtime deps.

/**
 * For each "forward" tool, the list of compensation tools that should
 * undo it. Multiple compensations are tried in order; the first one whose
 * resolved params look complete is invoked.
 */
export const AGENTIC_COMPENSATION_HINTS = {
  create_order: ['cancel_order'],
  create_cart: ['cancel_cart'],
  ship_order: ['cancel_order'],
  reserve_inventory: ['release_reservation'],
  confirm_reservation: ['release_reservation'],
  add_cart_item: ['remove_cart_item'],
  create_return: ['reject_return'],
  approve_return: ['reject_return'],
  create_payment: ['create_refund'],
};

/**
 * For each compensation tool, the parameter keys it expects. The resolver
 * walks the original step's params + result, looking for these keys (or
 * an `id` fallback) to construct the inverse-call payload.
 */
export const AGENTIC_COMPENSATION_PARAM_HINTS = {
  cancel_order: ['orderId'],
  cancel_cart: ['cartId'],
  release_reservation: ['reservationId'],
  remove_cart_item: ['itemId'],
  reject_return: ['returnId'],
  create_refund: ['paymentId'],
};

/**
 * Tool names whose calls must be deduplicated by idempotency key. The
 * orchestrator generates a deterministic key for these tools and
 * short-circuits duplicate invocations.
 */
export const AGENTIC_IDEMPOTENCY_HINTS = new Set([
  'create_payment',
  'create_stablecoin_payment',
  'create_refund',
]);

/**
 * Normalize an arbitrary value into a non-empty string ID, or undefined.
 * Numbers stringify; everything else (null/undefined/empty/object) → undefined.
 */
export const coerceReplayIdSource = (value) => {
  if (value === null || value === undefined) return undefined;
  if (typeof value === 'string' && value.length > 0) return value;
  if (typeof value === 'number') return `${value}`;
  return undefined;
};

/**
 * Look at `source[key]` for each `key` in `keyCandidates` (in order) and
 * return the first one that coerces to a non-empty ID string.
 */
export const extractReplayIdFromSource = (source, keyCandidates) => {
  if (!source || typeof source !== 'object') return undefined;
  for (const key of keyCandidates) {
    const candidate = coerceReplayIdSource(source[key]);
    if (candidate) return candidate;
  }
  return undefined;
};

/**
 * Find the first id-shaped value in `source`: prefers `source.id`, then
 * any key ending in `id` (case-insensitive), in iteration order.
 *
 * Currently unused by the orchestrator but kept for replay tooling that
 * needs a "best-effort" id without a candidate list. Underscore prefix
 * preserves the non-public-API hint.
 */
export const _extractFirstIdLikeValue = (source) => {
  if (!source || typeof source !== 'object') return undefined;
  const directId = coerceReplayIdSource(source.id);
  if (directId) return directId;
  for (const [key, value] of Object.entries(source)) {
    if (!key.toLowerCase().endsWith('id')) continue;
    const candidate = coerceReplayIdSource(value);
    if (candidate) return candidate;
  }
  return undefined;
};

/**
 * Build the inverse-call parameter object for a compensation step.
 *
 * Walks `params` and a fan-out of common result-payload locations
 * (`result.order`, `result.cart`, etc.), trying each `keyCandidate` from
 * `AGENTIC_COMPENSATION_PARAM_HINTS[compensationTool]`. Falls back to
 * `{id: <first id-like value>}` if no specific keys match.
 *
 * Returns `null` when nothing could be resolved (caller will skip the
 * compensation rather than fire a malformed call).
 */
export const buildCompensationParams = (compensationTool, params, result) => {
  const sources = [
    params || {},
    result || {},
    result?.order || {},
    result?.cart || {},
    result?.reservation || {},
    result?.item || {},
    result?.payment || {},
    result?.invoice || {},
    result?.customer || {},
    result?.return || {},
    result?.refund || {},
  ];
  const candidates = AGENTIC_COMPENSATION_PARAM_HINTS[compensationTool];
  const output = {};
  if (Array.isArray(candidates) && candidates.length > 0) {
    for (const key of candidates) {
      if (!key || typeof key !== 'string') continue;
      for (const source of sources) {
        const exact = extractReplayIdFromSource(source, [key]);
        if (exact) {
          output[key] = exact;
          break;
        }
        const idLike = extractReplayIdFromSource(source, ['id']);
        if (idLike && key.toLowerCase().endsWith('id')) {
          output[key] = idLike;
          break;
        }
      }
    }
  }

  if (!Object.keys(output).length) {
    const fallback = extractReplayIdFromSource(
      {
        ...params,
        ...(result || {}),
      },
      [
        'id',
        'orderId',
        'paymentId',
        'cartId',
        'reservationId',
        'returnId',
        'invoiceId',
        'customerId',
        'itemId',
      ],
    );
    if (fallback) {
      output.id = fallback;
    }
  }

  if (!Object.keys(output).length) return null;
  return output;
};
