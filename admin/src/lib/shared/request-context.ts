/**
 * Request Context using AsyncLocalStorage
 *
 * Provides per-request context (requestId, orgId, startTime) available
 * throughout the request lifecycle without explicit parameter passing.
 */

import { AsyncLocalStorage } from 'async_hooks';

export interface RequestContext {
  requestId: string;
  orgId?: string;
  startTime: number;
  path?: string;
  method?: string;
}

export const requestStore = new AsyncLocalStorage<RequestContext>();

/**
 * Get the current request context.
 * Returns undefined if called outside a request context.
 */
export function getRequestContext(): RequestContext | undefined {
  return requestStore.getStore();
}

/**
 * Get the current request ID.
 * Returns 'unknown' if called outside a request context.
 */
export function getRequestId(): string {
  return requestStore.getStore()?.requestId ?? 'unknown';
}

/**
 * Generate a unique request ID using a cryptographically secure UUID.
 */
export function generateRequestId(): string {
  return `req_${globalThis.crypto.randomUUID()}`;
}
